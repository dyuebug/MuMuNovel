import asyncio
import json
from types import SimpleNamespace
from typing import Any
from datetime import datetime, timedelta

import pytest
import pytest_asyncio
from fastapi import FastAPI
from httpx import ASGITransport, AsyncClient
from sqlalchemy import select
from sqlalchemy.ext.asyncio import async_sessionmaker, create_async_engine
from sqlalchemy.pool import StaticPool

from app.api import chapters as chapters_api
from app.api import chapter_analysis_routes as chapter_analysis_routes_api
from app.api import chapter_analysis_task_routes as chapter_analysis_task_routes_api
from app.api import chapter_annotation_routes as chapter_annotation_routes_api
from app.api import chapter_crud_routes as chapter_crud_routes_api
from app.api import chapter_draft_routes as chapter_draft_routes_api
from app.api import chapter_expansion_plan_routes as chapter_expansion_plan_routes_api
from app.api import chapter_partial_regeneration_routes as chapter_partial_regeneration_routes_api
from app.api import chapter_regeneration_routes as chapter_regeneration_routes_api
import app.database as app_database
from app.database import Base, get_db as app_get_db
from app.models.analysis_task import AnalysisTask
from app.models.batch_generation_snapshot import BatchGenerationSnapshot
from app.models.batch_generation_task import BatchGenerationTask
from app.models.chapter import Chapter
from app.models.chapter_draft_attempt import ChapterDraftAttempt
from app.models.generation_history import GenerationHistory
from app.models.memory import PlotAnalysis, StoryMemory
from app.models.outline import Outline
from app.models.project import Project
from app.models.regeneration_task import RegenerationTask
from app.services import batch_generation_run_wiring_service
from app.services import batch_generation_single_chapter_entry_service
from app.services.chapter_quality_context_service import StoryPacket

from tests.test_api.chapters_test_support import (
    _build_quality_history_payload,
    chapters_client,
    chapters_session_factory,
    create_chapter,
    create_outline,
    create_project,
    fake_ai_service,
    mock_side_effect_services,
    parse_sse_data,
    reset_chapters_runtime_caches,
)

pytestmark = pytest.mark.asyncio



async def test_should_handle_chapter_crud_and_project_word_count(
    chapters_client,
    chapters_session_factory,
    mock_user,
):
    project = await create_project(
        chapters_session_factory,
        user_id=mock_user.user_id,
        default_creative_mode="hook",
        default_story_focus="advance_plot",
    )
    outline = await create_outline(
        chapters_session_factory,
        project_id=project.id,
        order_index=3,
        title="第一章总纲",
    )

    create_response = await chapters_client.post(
        "/api/chapters",
        json={
            "project_id": project.id,
            "chapter_number": 1,
            "title": "第一章",
            "content": "abc",
            "outline_id": outline.id,
        },
    )
    assert create_response.status_code == 200
    created = create_response.json()
    chapter_id = created["id"]
    assert created["word_count"] == 3
    assert created["status"] == "draft"

    list_response = await chapters_client.get(f"/api/chapters/project/{project.id}")
    assert list_response.status_code == 200
    list_body = list_response.json()
    assert list_body["total"] == 1
    assert list_body["items"][0]["outline_title"] == "第一章总纲"
    assert list_body["items"][0]["outline_order"] == 3

    detail_response = await chapters_client.get(f"/api/chapters/{chapter_id}")
    assert detail_response.status_code == 200
    assert detail_response.json()["title"] == "第一章"

    update_response = await chapters_client.put(
        f"/api/chapters/{chapter_id}",
        json={"title": "第一章（修订）", "content": "abcdef"},
    )
    assert update_response.status_code == 200
    updated = update_response.json()
    assert updated["title"] == "第一章（修订）"
    assert updated["word_count"] == 6

    async with chapters_session_factory() as session:
        words_result = await session.execute(
            select(Project.current_words).where(Project.id == project.id)
        )
        assert words_result.scalar_one() == 6

    delete_response = await chapters_client.delete(f"/api/chapters/{chapter_id}")
    assert delete_response.status_code == 200

    async with chapters_session_factory() as session:
        deleted_chapter = await session.get(Chapter, chapter_id)
        assert deleted_chapter is None

        words_result = await session.execute(
            select(Project.current_words).where(Project.id == project.id)
        )
        assert words_result.scalar_one() == 0




async def test_should_delegate_project_chapter_list_query(
    chapters_client,
    chapters_session_factory,
    mock_user,
    monkeypatch,
):
    project = await create_project(chapters_session_factory, user_id=mock_user.user_id)
    captured = {}
    now = datetime.utcnow()

    async def fake_load_project_chapter_list_payload(**kwargs):
        captured.update(kwargs)
        return {
            'total': 1,
            'items': [{
                'id': 'crud-list-route',
                'project_id': kwargs['project_id'],
                'title': 'delegated list item',
                'chapter_number': 1,
                'content': 'abc',
                'summary': None,
                'word_count': 3,
                'status': 'draft',
                'outline_id': None,
                'sub_index': 1,
                'expansion_plan': None,
                'outline_title': None,
                'outline_order': None,
                'created_at': now,
                'updated_at': now,
            }],
        }

    monkeypatch.setattr(
        chapter_crud_routes_api,
        'load_project_chapter_list_payload',
        fake_load_project_chapter_list_payload,
    )

    response = await chapters_client.get(f'/api/chapters/project/{project.id}')

    assert response.status_code == 200
    assert response.json()['items'][0]['id'] == 'crud-list-route'
    assert captured['project_id'] == project.id
    assert captured['db_session'] is not None


async def test_should_delegate_create_chapter_workflow(
    chapters_client,
    chapters_session_factory,
    mock_user,
    monkeypatch,
):
    project = await create_project(chapters_session_factory, user_id=mock_user.user_id)
    captured = {}
    now = datetime.utcnow()

    async def fake_create_chapter_record(**kwargs):
        captured.update(kwargs)
        payload = kwargs['chapter_create']
        return {
            'id': 'crud-create-route',
            'project_id': kwargs['project'].id,
            'title': payload.title,
            'chapter_number': payload.chapter_number,
            'content': payload.content,
            'summary': payload.summary,
            'word_count': len(payload.content or ''),
            'status': payload.status,
            'outline_id': payload.outline_id,
            'sub_index': payload.sub_index,
            'expansion_plan': payload.expansion_plan,
            'outline_title': None,
            'outline_order': None,
            'created_at': now,
            'updated_at': now,
        }

    monkeypatch.setattr(
        chapter_crud_routes_api,
        'create_chapter_record',
        fake_create_chapter_record,
    )

    response = await chapters_client.post(
        '/api/chapters',
        json={
            'project_id': project.id,
            'chapter_number': 1,
            'title': 'delegated create',
            'content': 'abc',
        },
    )

    assert response.status_code == 200
    assert response.json()['id'] == 'crud-create-route'
    assert captured['project'].id == project.id
    assert captured['db_session'] is not None
    assert captured['chapter_create'].title == 'delegated create'


async def test_should_delegate_update_chapter_workflow(
    chapters_client,
    chapters_session_factory,
    mock_user,
    monkeypatch,
):
    project = await create_project(chapters_session_factory, user_id=mock_user.user_id)
    chapter = await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=1,
        title='before update',
        content='abc',
        status='draft',
    )
    captured = {}
    now = datetime.utcnow()

    async def fake_update_chapter_record(**kwargs):
        captured.update(kwargs)
        return {
            'id': kwargs['chapter'].id,
            'project_id': kwargs['chapter'].project_id,
            'title': 'delegated update',
            'chapter_number': kwargs['chapter'].chapter_number,
            'content': 'abcdef',
            'summary': kwargs['chapter'].summary,
            'word_count': 6,
            'status': kwargs['chapter'].status,
            'outline_id': kwargs['chapter'].outline_id,
            'sub_index': kwargs['chapter'].sub_index,
            'expansion_plan': kwargs['chapter'].expansion_plan,
            'outline_title': None,
            'outline_order': None,
            'created_at': now,
            'updated_at': now,
        }

    monkeypatch.setattr(
        chapter_crud_routes_api,
        'update_chapter_record',
        fake_update_chapter_record,
    )

    response = await chapters_client.put(
        f'/api/chapters/{chapter.id}',
        json={'title': 'delegated update', 'content': 'abcdef'},
    )

    assert response.status_code == 200
    assert response.json()['title'] == 'delegated update'
    assert captured['chapter'].id == chapter.id
    assert captured['db_session'] is not None
    assert captured['chapter_update'].title == 'delegated update'


async def test_should_delegate_delete_chapter_workflow(
    chapters_client,
    chapters_session_factory,
    mock_user,
    monkeypatch,
):
    project = await create_project(chapters_session_factory, user_id=mock_user.user_id)
    chapter = await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=1,
        title='before delete',
        content='abc',
        status='draft',
    )
    captured = {}

    async def fake_delete_chapter_record(**kwargs):
        captured.update(kwargs)
        return {'success': True}

    monkeypatch.setattr(
        chapter_crud_routes_api,
        'delete_chapter_record',
        fake_delete_chapter_record,
    )

    response = await chapters_client.delete(f'/api/chapters/{chapter.id}')

    assert response.status_code == 200
    assert response.json()['success'] is True
    assert captured['chapter'].id == chapter.id
    assert captured['user_id'] == mock_user.user_id
    assert captured['db_session'] is not None



async def test_should_delegate_chapter_navigation_query(
    chapters_client,
    chapters_session_factory,
    mock_user,
    monkeypatch,
):
    project = await create_project(chapters_session_factory, user_id=mock_user.user_id)
    chapter = await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=2,
        title='delegated nav current',
        content='B',
        status='completed',
    )
    captured = {}

    async def fake_load_chapter_navigation_payload(**kwargs):
        captured.update(kwargs)
        return {
            'current': {'id': chapter.id, 'chapter_number': 2, 'title': 'delegated nav current'},
            'previous': None,
            'next': None,
        }

    monkeypatch.setattr(
        chapter_crud_routes_api,
        'load_chapter_navigation_payload',
        fake_load_chapter_navigation_payload,
    )

    response = await chapters_client.get(f'/api/chapters/{chapter.id}/navigation')

    assert response.status_code == 200
    assert response.json()['current']['id'] == chapter.id
    assert captured['current_chapter'].id == chapter.id
    assert captured['db_session'] is not None


async def test_should_return_chapter_navigation(
    chapters_client,
    chapters_session_factory,
    mock_user,
):
    project = await create_project(chapters_session_factory, user_id=mock_user.user_id)
    chapter_one = await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=1,
        title="第一章",
        content="A",
        status="completed",
    )
    chapter_two = await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=2,
        title="第二章",
        content="B",
        status="completed",
    )
    chapter_three = await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=3,
        title="第三章",
        content="C",
        status="completed",
    )

    response = await chapters_client.get(f"/api/chapters/{chapter_two.id}/navigation")
    assert response.status_code == 200

    body = response.json()
    assert body["current"] == {
        "id": chapter_two.id,
        "chapter_number": 2,
        "title": "第二章",
    }
    assert body["previous"] == {
        "id": chapter_one.id,
        "chapter_number": 1,
        "title": "第一章",
    }
    assert body["next"] == {
        "id": chapter_three.id,
        "chapter_number": 3,
        "title": "第三章",
    }


async def test_should_update_chapter_expansion_plan(
    chapters_client,
    chapters_session_factory,
    mock_user,
):
    project = await create_project(chapters_session_factory, user_id=mock_user.user_id)
    chapter = await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=1,
        title="规划章节",
        content="正文",
        status="draft",
    )

    response = await chapters_client.put(
        f"/api/chapters/{chapter.id}/expansion-plan",
        json={
            "summary": "新的章节概要",
            "key_events": ["主角与守门人冲突", "钥匙暴露"],
            "emotional_tone": "紧张",
        },
    )
    assert response.status_code == 200

    body = response.json()
    assert body["id"] == chapter.id
    assert body["summary"] == "新的章节概要"
    assert body["expansion_plan"]["key_events"] == ["主角与守门人冲突", "钥匙暴露"]
    assert body["expansion_plan"]["emotional_tone"] == "紧张"
    assert body["message"] == "规划信息更新成功"


async def test_should_delegate_expansion_plan_route_to_compat_service(
    chapters_client,
    monkeypatch,
):
    captured: dict[str, Any] = {}

    async def fake_update_chapter_expansion_plan_with_default_route_wiring(**kwargs):
        captured.update(kwargs)
        return {
            "id": kwargs["chapter_id"],
            "summary": "delegated summary",
            "expansion_plan": {"key_events": ["delegated"]},
            "message": "ok",
        }

    monkeypatch.setattr(
        chapter_expansion_plan_routes_api,
        "update_chapter_expansion_plan_with_default_route_wiring",
        fake_update_chapter_expansion_plan_with_default_route_wiring,
    )

    response = await chapters_client.put(
        '/api/chapters/expansion-route/expansion-plan',
        json={"summary": "delegated summary", "key_events": ["delegated"]},
    )

    assert response.status_code == 200
    assert response.json()["id"] == "expansion-route"
    assert captured["chapter_id"] == "expansion-route"
    assert captured["request"] is not None
    assert captured["db_session"] is not None
    assert captured["expansion_plan"].summary == "delegated summary"
    assert captured["expansion_plan"].key_events == ["delegated"]



async def test_should_delegate_regeneration_tasks_query(
    chapters_client,
    chapters_session_factory,
    mock_user,
    monkeypatch,
):
    project = await create_project(chapters_session_factory, user_id=mock_user.user_id)
    chapter = await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=1,
        title='delegated regeneration route',
        content='abc',
        status='completed',
    )
    captured: dict[str, Any] = {}

    async def fake_load_regeneration_tasks_payload(**kwargs):
        captured.update(kwargs)
        return {
            'chapter_id': kwargs['chapter_id'],
            'total': 1,
            'tasks': [{
                'task_id': 'regen-task-delegated',
                'status': 'completed',
                'version_number': 2,
                'version_note': 'delegated',
                'original_word_count': 3,
                'regenerated_word_count': 5,
                'created_at': None,
                'completed_at': None,
            }],
        }

    monkeypatch.setattr(
        chapter_regeneration_routes_api,
        'load_regeneration_tasks_payload',
        fake_load_regeneration_tasks_payload,
    )

    response = await chapters_client.get(
        f'/api/chapters/{chapter.id}/regeneration/tasks',
        params={'limit': 7},
    )

    assert response.status_code == 200
    assert response.json()['tasks'][0]['task_id'] == 'regen-task-delegated'
    assert captured['chapter_id'] == chapter.id
    assert captured['limit'] == 7
    assert captured['db_session'] is not None


async def test_should_return_regeneration_tasks_history(
    chapters_client,
    chapters_session_factory,
    mock_user,
):
    project = await create_project(chapters_session_factory, user_id=mock_user.user_id)
    chapter = await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=1,
        title="重写历史章节",
        content="原始正文",
        status="completed",
    )

    async with chapters_session_factory() as session:
        session.add(
            RegenerationTask(
                chapter_id=chapter.id,
                user_id=mock_user.user_id,
                project_id=project.id,
                modification_instructions="增强冲突",
                original_content="原始正文",
                original_word_count=4,
                regenerated_content="新的正文版本",
                regenerated_word_count=6,
                version_number=2,
                version_note="加强结尾冲突",
                status="completed",
                progress=100,
            )
        )
        await session.commit()

    response = await chapters_client.get(f"/api/chapters/{chapter.id}/regeneration/tasks")
    assert response.status_code == 200

    body = response.json()
    assert body["chapter_id"] == chapter.id
    assert body["total"] == 1
    assert len(body["tasks"]) == 1
    assert body["tasks"][0]["status"] == "completed"
    assert body["tasks"][0]["version_number"] == 2
    assert body["tasks"][0]["version_note"] == "加强结尾冲突"
    assert body["tasks"][0]["regenerated_word_count"] == 6


async def test_should_return_401_when_create_chapter_without_user_id(
    chapters_client,
    chapters_session_factory,
    mock_user,
):
    project = await create_project(chapters_session_factory, user_id=mock_user.user_id)

    response = await chapters_client.post(
        "/api/chapters",
        headers={"x-test-user-id": "__none__"},
        json={
            "project_id": project.id,
            "chapter_number": 1,
            "title": "未登录创建",
            "content": "abc",
        },
    )
    assert response.status_code == 401


async def test_should_return_401_when_get_chapter_without_user_id(
    chapters_client,
    chapters_session_factory,
    mock_user,
):
    project = await create_project(chapters_session_factory, user_id=mock_user.user_id)
    chapter = await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=1,
        title="未登录章节详情",
        content="abc",
        status="draft",
    )

    response = await chapters_client.get(
        f"/api/chapters/{chapter.id}",
        headers={"x-test-user-id": "__none__"},
    )
    assert response.status_code == 401


async def test_should_return_404_when_accessing_project_owned_by_other_user(
    chapters_client,
    chapters_session_factory,
):
    foreign_project = await create_project(
        chapters_session_factory,
        user_id="another-user",
        title="他人项目",
    )

    response = await chapters_client.get(f"/api/chapters/project/{foreign_project.id}")
    assert response.status_code == 404


async def test_should_return_404_when_chapter_not_found(chapters_client):
    response = await chapters_client.get("/api/chapters/not-exists")
    assert response.status_code == 404




async def test_generate_single_chapter_for_batch_should_build_runtime_context_without_dirty_writes(
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
        title="batch-runtime-context",
    )

    quality_metric_calls: list[dict[str, Any]] = []

    class FakeContext:
        chapter_outline = "批量生成大纲"
        continuation_point = None
        previous_chapter_summary = ""
        chapter_characters = (
            "角色状态\n- 角色A\n"
            "人物动态\n- 角色A正在追查线索\n"
            "关系动态\n- 角色A/角色B互相试探"
        )
        chapter_careers = "角色A：巡夜人"
        foreshadow_reminders = (
            "伏笔提醒\n- 失踪账册\n"
            "回收线索\n- 下一章需要兑现账册来源"
        )
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
        quality_metric_calls.append(kwargs)
        return {
            "overall_score": 88.0,
            "conflict_chain_hit_rate": 82.0,
            "rule_grounding_hit_rate": 84.0,
            "outline_alignment_rate": 86.0,
            "dialogue_naturalness_rate": 80.0,
            "opening_hook_rate": 87.0,
            "payoff_chain_rate": 81.0,
            "cliffhanger_rate": 85.0,
            "quality_runtime_context": kwargs.get("quality_runtime_context"),
        }

    monkeypatch.setattr(
        batch_generation_single_chapter_entry_service.chapter_web_research_service,
        "is_enabled",
        lambda *_args, **_kwargs: False,
    )
    monkeypatch.setattr(
        batch_generation_single_chapter_entry_service,
        "OneToManyContextBuilder",
        FakeOneToManyBuilder,
    )
    monkeypatch.setattr(
        batch_generation_single_chapter_entry_service,
        "resolve_chapter_quality_profile",
        fake_resolve_chapter_quality_profile,
    )
    monkeypatch.setattr(
        batch_generation_single_chapter_entry_service.PromptService,
        "get_template",
        fake_get_template,
    )
    monkeypatch.setattr(
        batch_generation_single_chapter_entry_service.PromptService,
        "format_prompt",
        fake_format_prompt,
    )
    monkeypatch.setattr(
        batch_generation_single_chapter_entry_service,
        "build_chapter_runtime_system_prompt",
        fake_build_chapter_runtime_system_prompt,
    )
    monkeypatch.setattr(
        batch_generation_single_chapter_entry_service,
        "compute_story_quality_metrics",
        fake_compute_story_quality_metrics,
    )

    fake_ai_service.calls.clear()
    fake_ai_service.chunks = ["批量", "正文"]

    async with chapters_session_factory() as session:
        db_chapter = await session.get(Chapter, chapter.id)
        db_project = await session.get(Project, project.id)
        assert db_chapter is not None
        assert db_project is not None

        result = await batch_generation_single_chapter_entry_service.generate_single_chapter_for_batch(
            db_session=session,
            chapter=db_chapter,
            user_id=mock_user.user_id,
            style_id=None,
            target_word_count=600,
            ai_service=fake_ai_service,
            write_lock=chapters_api.Lock(),
        )

        assert result["full_content"] == "批量正文"
        assert result["word_count"] == len("批量正文")
        assert db_chapter.content is None
        assert db_chapter.word_count == 0
        assert db_project.current_words == 0

        await session.refresh(db_chapter)
        await session.refresh(db_project)
        assert db_chapter.content is None
        assert db_chapter.word_count == 0
        assert db_project.current_words == 0

    assert quality_metric_calls
    runtime_context = quality_metric_calls[0]["quality_runtime_context"]
    assert runtime_context["current_chapter_number"] == 1
    assert runtime_context["target_word_count"] == 600
    assert "角色A正在追查线索" in runtime_context["character_state_ledger"]
    assert "角色A/角色B互相试探" in runtime_context["relationship_state_ledger"]
    assert "下一章需要兑现账册来源" in runtime_context["foreshadow_state_ledger"]
    assert result["quality_metrics"]["quality_runtime_context"] == runtime_context








async def test_generate_single_chapter_for_batch_should_inject_web_research_grounding_block(
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
        title="batch-web-research",
    )

    captured_runtime_prompt_kwargs: dict[str, Any] = {}
    replace_memory_calls: list[dict[str, Any]] = []

    class FakeContext:
        chapter_outline = "batch outline"
        continuation_point = None
        previous_chapter_summary = ""
        chapter_characters = "character-a"
        chapter_careers = "career-a"
        foreshadow_reminders = "foreshadow-a"
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
        captured_runtime_prompt_kwargs.update(kwargs)
        return "mock-batch-system-prompt"

    def fake_compute_story_quality_metrics(**kwargs):
        return {
            "overall_score": 88.0,
            "conflict_chain_hit_rate": 82.0,
            "rule_grounding_hit_rate": 84.0,
            "outline_alignment_rate": 86.0,
            "dialogue_naturalness_rate": 80.0,
            "opening_hook_rate": 87.0,
            "payoff_chain_rate": 81.0,
            "cliffhanger_rate": 85.0,
            "quality_runtime_context": kwargs.get("quality_runtime_context"),
        }

    async def fake_collect_for_chapter(**kwargs):
        return {
            "query": "night market customs",
            "archive_path": "tmp/research.json",
            "assets": [
                {
                    "title": "night market",
                    "source": "mock-source",
                    "summary": "used for scene atmosphere and crowd texture.",
                    "usage_hint": "improve environment details",
                }
            ],
        }

    async def fake_replace_chapter_memories(**kwargs):
        replace_memory_calls.append(kwargs)
        return ["memory-1"]

    monkeypatch.setattr(
        batch_generation_single_chapter_entry_service.chapter_web_research_service,
        "is_enabled",
        lambda *_args, **_kwargs: True,
    )
    monkeypatch.setattr(
        batch_generation_single_chapter_entry_service.chapter_web_research_service,
        "collect_for_chapter",
        fake_collect_for_chapter,
    )
    monkeypatch.setattr(
        batch_generation_single_chapter_entry_service.chapter_web_research_service,
        "replace_chapter_memories",
        fake_replace_chapter_memories,
    )
    monkeypatch.setattr(
        batch_generation_single_chapter_entry_service,
        "OneToManyContextBuilder",
        FakeOneToManyBuilder,
    )
    monkeypatch.setattr(
        batch_generation_single_chapter_entry_service,
        "resolve_chapter_quality_profile",
        fake_resolve_chapter_quality_profile,
    )
    monkeypatch.setattr(
        batch_generation_single_chapter_entry_service.PromptService,
        "get_template",
        fake_get_template,
    )
    monkeypatch.setattr(
        batch_generation_single_chapter_entry_service.PromptService,
        "format_prompt",
        fake_format_prompt,
    )
    monkeypatch.setattr(
        batch_generation_single_chapter_entry_service,
        "build_chapter_runtime_system_prompt",
        fake_build_chapter_runtime_system_prompt,
    )
    monkeypatch.setattr(
        batch_generation_single_chapter_entry_service,
        "compute_story_quality_metrics",
        fake_compute_story_quality_metrics,
    )

    fake_ai_service.calls.clear()
    fake_ai_service.chunks = ["draft ", "text"]

    async with chapters_session_factory() as session:
        db_chapter = await session.get(Chapter, chapter.id)
        assert db_chapter is not None

        result = await batch_generation_single_chapter_entry_service.generate_single_chapter_for_batch(
            db_session=session,
            chapter=db_chapter,
            user_id=mock_user.user_id,
            style_id=None,
            target_word_count=600,
            ai_service=fake_ai_service,
            write_lock=chapters_api.Lock(),
            enable_web_research=True,
            web_research_query="night market customs",
        )

    assert result["full_content"] == "draft text"
    assert fake_ai_service.calls
    assert fake_ai_service.calls[0]["system_prompt"] == "mock-batch-system-prompt"
    assert captured_runtime_prompt_kwargs["web_research_grounding_block"]
    assert "night market" in captured_runtime_prompt_kwargs["web_research_grounding_block"]
    assert "scene atmosphere" in captured_runtime_prompt_kwargs["web_research_grounding_block"]
    assert replace_memory_calls
    assert replace_memory_calls[0]["query"] == "night market customs"
    assert len(replace_memory_calls[0]["assets"]) == 1


async def test_should_sync_project_words_when_partial_apply_chapter_word_count_missing(
    chapters_client,
    chapters_session_factory,
    mock_user,
):
    project = await create_project(chapters_session_factory, user_id=mock_user.user_id)
    chapter = await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=1,
        title="Word Count Sync",
        content="ABCDEFG",
        status="completed",
    )

    async with chapters_session_factory() as session:
        saved_chapter = await session.get(Chapter, chapter.id)
        saved_project = await session.get(Project, project.id)
        assert saved_chapter is not None and saved_project is not None
        saved_chapter.word_count = None
        saved_project.current_words = len(saved_chapter.content or "")
        await session.commit()

    response = await chapters_client.post(
        f"/api/chapters/{chapter.id}/apply-partial-regenerate",
        json={
            "new_text": "XYZ",
            "start_position": 1,
            "end_position": 4,
        },
    )
    assert response.status_code == 200

    async with chapters_session_factory() as session:
        saved_chapter = await session.get(Chapter, chapter.id)
        saved_project = await session.get(Project, project.id)
        assert saved_chapter is not None and saved_project is not None
        assert saved_chapter.content == "AXYZEFG"
        assert saved_chapter.word_count == len("AXYZEFG")
        assert saved_project.current_words == len("AXYZEFG")






async def test_should_forward_creative_mode_to_batch_background_generation(
    chapters_client,
    chapters_session_factory,
    mock_user,
    monkeypatch,
):
    captured: dict[str, Any] = {}

    async def fake_execute_batch_generation(*args, **kwargs):
        captured.update(kwargs)
        return None

    monkeypatch.setattr(
        batch_generation_run_wiring_service,
        "execute_batch_generation_in_order_with_entry_service_seams",
        fake_execute_batch_generation,
    )

    project = await create_project(chapters_session_factory, user_id=mock_user.user_id)

    await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=1,
        title="第一章",
        content="前置章节已完成",
        status="completed",
    )
    await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=2,
        title="第二章",
        content=None,
    )
    await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=3,
        title="第三章",
        content=None,
    )

    response = await chapters_client.post(
        f"/api/chapters/project/{project.id}/batch-generate",
        json={
            "start_chapter_number": 2,
            "count": 2,
            "target_word_count": 500,
            "enable_analysis": False,
            "enable_mcp": False,
            "max_retries": 1,
            "creative_mode": "payoff",
            "story_focus": "foreshadow_payoff",
            "plot_stage": "ending",
        },
    )

    assert response.status_code == 200
    assert isinstance(captured["story_packet"], StoryPacket)
    assert captured["story_packet"].guidance.creative_mode == "payoff"
    assert captured["story_packet"].guidance.story_focus == "foreshadow_payoff"
    assert captured["story_packet"].guidance.plot_stage == "ending"


async def test_should_forward_web_research_settings_for_batch_background_generation(
    chapters_client,
    chapters_session_factory,
    mock_user,
    monkeypatch,
):
    captured: dict[str, Any] = {}

    async def fake_execute_batch_generation(*args, **kwargs):
        captured.update(kwargs)
        return None

    monkeypatch.setattr(
        batch_generation_run_wiring_service,
        "execute_batch_generation_in_order_with_entry_service_seams",
        fake_execute_batch_generation,
    )

    project = await create_project(chapters_session_factory, user_id=mock_user.user_id)

    await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=1,
        title="Chapter One",
        content="opening paragraph",
        status="completed",
    )
    await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=2,
        title="Chapter Two",
        content=None,
    )

    response = await chapters_client.post(
        f"/api/chapters/project/{project.id}/batch-generate",
        json={
            "start_chapter_number": 2,
            "count": 1,
            "target_word_count": 600,
            "enable_analysis": False,
            "enable_mcp": False,
            "max_retries": 1,
            "enable_web_research": True,
            "web_research_query": "late qing trade customs",
        },
    )

    assert response.status_code == 200
    assert captured["enable_web_research"] is True
    assert captured["web_research_query"] == "late qing trade customs"


async def test_should_fallback_to_project_generation_defaults_for_batch_background_generation(
    chapters_client,
    chapters_session_factory,
    mock_user,
    monkeypatch,
):
    captured: dict[str, Any] = {}

    async def fake_execute_batch_generation(*args, **kwargs):
        captured.update(kwargs)
        return None

    monkeypatch.setattr(
        batch_generation_run_wiring_service,
        "execute_batch_generation_in_order_with_entry_service_seams",
        fake_execute_batch_generation,
    )

    project = await create_project(
        chapters_session_factory,
        user_id=mock_user.user_id,
        default_creative_mode="hook",
        default_story_focus="advance_plot",
        default_plot_stage="development",
        default_story_creation_brief="默认要求：保持连载感和推进效率。",
    )

    await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=1,
        title="第一章",
        content="前置章节已完成",
        status="completed",
    )
    await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=2,
        title="第二章",
        content=None,
    )

    response = await chapters_client.post(
        f"/api/chapters/project/{project.id}/batch-generate",
        json={
            "start_chapter_number": 2,
            "count": 1,
            "target_word_count": 500,
            "enable_analysis": False,
            "enable_mcp": False,
            "max_retries": 1,
        },
    )

    assert response.status_code == 200
    assert isinstance(captured["story_packet"], StoryPacket)
    assert captured["story_packet"].guidance.creative_mode == "hook"
    assert captured["story_packet"].guidance.story_focus == "advance_plot"
    assert captured["story_packet"].guidance.plot_stage == "development"
    assert captured["story_packet"].guidance.story_creation_brief == "默认要求：保持连载感和推进效率。"




async def test_should_skip_snapshot_commit_when_payload_is_unchanged(
    chapters_session_factory,
    mock_user,
    monkeypatch,
):
    project = await create_project(chapters_session_factory, user_id=mock_user.user_id)

    async with chapters_session_factory() as session:
        task = BatchGenerationTask(
            project_id=project.id,
            user_id=mock_user.user_id,
            start_chapter_number=1,
            chapter_count=1,
            chapter_ids=["chapter-1"],
            status="running",
            total_chapters=1,
            completed_chapters=0,
        )
        session.add(task)
        await session.commit()
        await session.refresh(task)

        commit_count = 0
        original_commit = session.commit

        async def counted_commit():
            nonlocal commit_count
            commit_count += 1
            return await original_commit()

        monkeypatch.setattr(session, "commit", counted_commit)

        await chapters_api._upsert_batch_generation_snapshot(
            session,
            task.id,
            workflow_runtime_state={"phase": "loading", "progress": 5},
        )
        await chapters_api._upsert_batch_generation_snapshot(
            session,
            task.id,
            workflow_runtime_state={"phase": "loading", "progress": 5},
        )
        await chapters_api._upsert_batch_generation_snapshot(
            session,
            task.id,
            workflow_runtime_state={"phase": "generating", "progress": 35},
        )

        snapshot_result = await session.execute(
            select(BatchGenerationSnapshot).where(BatchGenerationSnapshot.batch_task_id == task.id)
        )
        snapshot = snapshot_result.scalar_one_or_none()

    assert commit_count == 2
    assert snapshot is not None
    assert snapshot.latest_quality_metrics is None
    assert snapshot.quality_metrics_history is None
    assert snapshot.quality_metrics_summary is None
    assert snapshot.workflow_runtime_state["phase"] == "generating"
    assert snapshot.workflow_runtime_state["progress"] == 35


















def test_should_build_runtime_prompt_with_serial_style_guard():
    project = SimpleNamespace(
        world_time_period="近未来",
        world_location="临海三环",
        world_atmosphere="潮湿压迫",
        world_rules="门影会在镜面附近折返",
    )

    runtime_prompt = chapters_api._build_chapter_runtime_system_prompt(
        project=project,
        style_content="写作风格建议：低AI连载感",
        chapter_outline="【关键事件】\n- 主角带队撤离\n- 出现镜面门",
        previous_summary="上一章队伍已进入高风险走廊",
        style_name="低AI连载感",
        style_preset_id="low_ai_serial",
    )

    assert "连载感优先" in runtime_prompt
    assert "情绪要有层次" in runtime_prompt
    assert "慎用“像……/仿佛/像……一样”" in runtime_prompt
    assert "少用“下一秒/那一瞬/忽然/不是……而是……”" in runtime_prompt
    assert "台词长度控制：单句以6-18字为主" not in runtime_prompt


def test_should_resolve_generation_temperature_by_style_profile():
    serial_profile = chapters_api._detect_style_profile(
        style_name="低AI连载感",
        style_preset_id="low_ai_serial",
        style_content="",
    )
    life_profile = chapters_api._detect_style_profile(
        style_name="低AI生活化",
        style_preset_id="low_ai_life",
        style_content="",
    )
    default_profile = chapters_api._detect_style_profile(
        style_name="默认风格",
        style_preset_id="",
        style_content="",
    )

    assert chapters_api._resolve_generation_temperature(serial_profile) == pytest.approx(0.82)
    assert chapters_api._resolve_generation_temperature(life_profile) == pytest.approx(0.78)
    assert chapters_api._resolve_generation_temperature(default_profile) == pytest.approx(0.72)


def test_should_append_serial_guard_when_apply_style_to_prompt():
    merged_prompt = chapters_api.WritingStyleManager.apply_style_to_prompt(
        base_prompt="基础提示词",
        style_content="写作风格建议：低AI连载感，强调现场感",
    )

    assert "连载强化要点" in merged_prompt
    assert "人物情绪要有层次" in merged_prompt
    assert "比喻要克制" in merged_prompt
    assert "慎用高频定式句法" in merged_prompt




def test_should_dedupe_overlapping_continuity_missing_items_in_candidate_highlights():
    highlights = chapters_api._build_candidate_draft_quality_highlights(
        content="The chapter focuses on the quiet before the alarm but never pays it off.",
        quality_metrics={
            "quality_runtime_context": {
                "character_state_ledger": [
                    "Watchtower alarm: Harbor bells stay one strike away from a citywide lockdown.",
                    "Alliance fracture: The dockworkers no longer trust the magistrate.",
                ],
            },
            "continuity_preflight": {
                "status": "warning",
                "warnings": [
                    {"item": "Watchtower alarm"},
                ],
            },
        },
    )

    continuity = highlights["continuity"]
    assert continuity["missing_items"] == [
        "Watchtower alarm: Harbor bells stay one strike away from a citywide lockdown.",
        "Alliance fracture: The dockworkers no longer trust the magistrate.",
    ]


def test_should_attach_continuity_preflight_to_story_quality_metrics():
    runtime_context = {
        "plot_stage": "development",
        "chapter_count": 12,
        "current_chapter_number": 5,
        "character_state_ledger": ["Lin: injured hand still limits movement"],
        "relationship_state_ledger": ["Lin/Su: uneasy alliance under tension"],
        "foreshadow_state_ledger": ["RoyalKey: still missing from the archive"],
    }

    metrics = chapters_api.compute_story_quality_metrics(
        content="Lin slipped into the archive alone, his injured hand slowing every move.",
        chapter_outline="Lin and Su hold a fragile alliance while the RoyalKey clue advances.",
        world_rules="An injured hand should reduce action efficiency.",
        quality_runtime_context=runtime_context,
    )

    assert metrics["continuity_preflight"]["status"] == "warning"
    assert metrics["continuity_preflight"]["warning_count"] == 2
    assert metrics["quality_gate"]["continuity_warning_count"] == 2
    assert "relationship_continuity" in metrics["repair_guidance"]["focus_areas"]
    assert metrics["repair_guidance"]["repair_targets"]


def test_should_detect_workflow_meta_line_in_generated_content():
    text = "\n".join([
        "以下是章节正文：",
        "步骤1：先输出冲突",
        "他推门而入，雨水沿着衣角滴到地砖上。",
    ])

    assert chapters_api._contains_chapter_workflow_meta_text(text) is True


def test_should_not_misclassify_story_text_with_plan_ab():
    text = "他把方案A塞进口袋，转头对同伴说先按旧路撤。"

    assert chapters_api._contains_chapter_workflow_meta_text(text) is False


def test_should_sanitize_generated_narrative_text_keep_story_lines():
    raw_text = "\n".join(
        [
            "以下是章节正文：",
            "执行1.1：先描述冲突",
            "门外的风越刮越急，他还是把灯点亮了。",
            "调用Agent补全设定",
            "她没有回答，只把手里的信纸折成更小的一块。",
        ]
    )

    cleaned, removed_count = chapters_api._sanitize_generated_narrative_text(raw_text)

    assert removed_count == 3
    assert "执行1.1" not in cleaned
    assert "调用Agent" not in cleaned
    assert "门外的风越刮越急" in cleaned
    assert "她没有回答" in cleaned


def test_should_lightly_polish_high_frequency_template_phrases():
    first_next = "下一秒，门外有人敲了两下玻璃。"
    second_next = "下一秒，收银台下的灯灭了。"
    first_moment = "那一瞬，他听见冰柜里咯地一声。"
    second_moment = "那一瞬，她已经把刀收回袖口。"
    first_simile = "雨丝像细针一样扎在玻璃上。"
    second_simile = "风声像砂纸一样刮过卷帘门。"
    third_simile = "裂纹像旧瓷一样往手背上爬。"
    vague_simile = "地上的水痕像有什么东西拖过去。"

    raw_text = "\n".join(
        [
            first_next,
            second_next,
            first_moment,
            second_moment,
            first_simile,
            second_simile,
            third_simile,
            vague_simile,
        ]
    )

    cleaned, removed_count = chapters_api._sanitize_generated_narrative_text(raw_text)

    assert removed_count == 0
    assert first_next in cleaned
    assert second_next not in cleaned
    assert "收银台下的灯灭了。" in cleaned
    assert first_moment in cleaned
    assert second_moment not in cleaned
    assert "她已经把刀收回袖口。" in cleaned
    assert first_simile in cleaned
    assert second_simile in cleaned
    assert "裂纹像旧瓷那样往手背上爬。" in cleaned
    assert "像有什么东西" not in cleaned
    assert "像有东西拖过去。" in cleaned


def test_should_mark_ai_identity_disclaimer_as_meta_text():
    text = "作为AI助手，我将先给出执行计划再输出正文。"

    assert chapters_api._contains_chapter_workflow_meta_text(text) is True


async def test_should_create_single_chapter_background_generation_task(
    chapters_client,
    chapters_session_factory,
    mock_user,
):
    project = await create_project(chapters_session_factory, user_id=mock_user.user_id)
    outline = await create_outline(
        chapters_session_factory,
        project_id=project.id,
        order_index=1,
        title="单章后台生成大纲",
    )
    chapter = await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=1,
        title="待后台生成章节",
        content=None,
        outline_id=outline.id,
    )

    response = await chapters_client.post(
        f"/api/chapters/{chapter.id}/generate-background",
        json={"target_word_count": 1200},
    )
    assert response.status_code == 200
    body = response.json()
    assert body["chapter_id"] == chapter.id
    assert body["status"] == "pending"
    assert body["task_id"]

    async with chapters_session_factory() as session:
        task = await session.get(BatchGenerationTask, body["task_id"])
        assert task is not None
        assert task.chapter_count == 1
        assert task.chapter_ids == [chapter.id]
        assert task.target_word_count == 1200
        assert task.enable_analysis is True


async def test_should_allow_disabling_analysis_for_single_chapter_background_generation(
    chapters_client,
    chapters_session_factory,
    mock_user,
):
    project = await create_project(chapters_session_factory, user_id=mock_user.user_id)
    outline = await create_outline(
        chapters_session_factory,
        project_id=project.id,
        order_index=1,
        title="单章关闭分析大纲",
    )
    chapter = await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=1,
        title="待关闭分析章节",
        content=None,
        outline_id=outline.id,
    )

    response = await chapters_client.post(
        f"/api/chapters/{chapter.id}/generate-background",
        json={"target_word_count": 1200, "enable_analysis": False},
    )
    assert response.status_code == 200
    body = response.json()

    async with chapters_session_factory() as session:
        task = await session.get(BatchGenerationTask, body["task_id"])
        assert task is not None
        assert task.enable_analysis is False


async def test_should_forward_creative_mode_to_single_background_generation(
    chapters_client,
    chapters_session_factory,
    mock_user,
    monkeypatch,
):
    captured: dict[str, Any] = {}

    async def fake_execute_batch_generation(*args, **kwargs):
        captured.update(kwargs)
        return None

    monkeypatch.setattr(
        batch_generation_run_wiring_service,
        "execute_batch_generation_in_order_with_entry_service_seams",
        fake_execute_batch_generation,
    )

    project = await create_project(chapters_session_factory, user_id=mock_user.user_id)
    outline = await create_outline(
        chapters_session_factory,
        project_id=project.id,
        order_index=1,
        title="单章后台创作模式大纲",
    )
    chapter = await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=1,
        title="待创作模式后台生成章节",
        content=None,
        outline_id=outline.id,
    )

    response = await chapters_client.post(
        f"/api/chapters/{chapter.id}/generate-background",
        json={
            "target_word_count": 1200,
            "creative_mode": "suspense",
            "story_focus": "reveal_mystery",
            "plot_stage": "climax",
            "story_creation_brief": "本轮先把正面对撞和章尾牵引写实",
            "story_repair_summary": "优先补强冲突抬压",
            "story_repair_targets": ["写实受阻", "升级代价"],
            "story_preserve_strengths": ["保留对白辨识度"],
        },
    )

    assert response.status_code == 200
    assert isinstance(captured["story_packet"], StoryPacket)
    assert captured["story_packet"].guidance.creative_mode == "suspense"
    assert captured["story_packet"].guidance.story_focus == "reveal_mystery"
    assert captured["story_packet"].guidance.plot_stage == "climax"
    assert captured["story_packet"].guidance.story_creation_brief == "本轮先把正面对撞和章尾牵引写实"
    assert captured["story_repair_summary"] == "优先补强冲突抬压"
    assert captured["story_repair_targets"] == ["写实受阻", "升级代价"]
    assert captured["story_preserve_strengths"] == ["保留对白辨识度"]




async def test_should_forward_web_research_settings_to_single_background_generation(
    chapters_client,
    chapters_session_factory,
    mock_user,
    monkeypatch,
):
    captured: dict[str, Any] = {}

    async def fake_execute_batch_generation(*args, **kwargs):
        captured.update(kwargs)
        return None

    monkeypatch.setattr(
        batch_generation_run_wiring_service,
        "execute_batch_generation_in_order_with_entry_service_seams",
        fake_execute_batch_generation,
    )

    project = await create_project(chapters_session_factory, user_id=mock_user.user_id)
    outline = await create_outline(
        chapters_session_factory,
        project_id=project.id,
        order_index=1,
        title="单章后台联网生成大纲",
    )
    chapter = await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=1,
        title="待联网后台生成章节",
        content=None,
        outline_id=outline.id,
    )

    response = await chapters_client.post(
        f"/api/chapters/{chapter.id}/generate-background",
        json={
            "target_word_count": 1200,
            "enable_web_research": True,
            "web_research_query": "late qing harbor guild rules",
            "enable_analysis": False,
        },
    )

    assert response.status_code == 200
    assert captured["enable_web_research"] is True
    assert captured["web_research_query"] == "late qing harbor guild rules"
async def test_should_auto_fill_story_repair_payload_from_chapter_quality_history_for_single_background_generation(
    chapters_client,
    chapters_session_factory,
    mock_user,
    monkeypatch,
):
    captured: dict[str, Any] = {}

    async def fake_execute_batch_generation(*args, **kwargs):
        captured.update(kwargs)
        return None

    monkeypatch.setattr(
        batch_generation_run_wiring_service,
        "execute_batch_generation_in_order_with_entry_service_seams",
        fake_execute_batch_generation,
    )

    project = await create_project(chapters_session_factory, user_id=mock_user.user_id)
    outline = await create_outline(
        chapters_session_factory,
        project_id=project.id,
        order_index=1,
        title="preview only chapter",
    )
    chapter = await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=1,
        title="待补修章节预览",
        content=None,
        outline_id=outline.id,
    )
    quality_metrics = {
        "overall_score": 71.2,
        "conflict_chain_hit_rate": 58.0,
        "rule_grounding_hit_rate": 80.0,
        "outline_alignment_rate": 60.0,
        "dialogue_naturalness_rate": 77.0,
        "opening_hook_rate": 74.0,
        "payoff_chain_rate": 55.0,
        "cliffhanger_rate": 78.0,
        "pacing_score": 6.8,
    }

    async with chapters_session_factory() as session:
        session.add(
            GenerationHistory(
                project_id=project.id,
                chapter_id=chapter.id,
                prompt="chapter_generation",
                generated_content=chapters_api._build_generation_history_payload("generated body", quality_metrics),
                model="chapter_generator_v1",
                created_at=datetime.utcnow() - timedelta(minutes=3),
            )
        )
        await session.commit()

    response = await chapters_client.post(
        f"/api/chapters/{chapter.id}/generate-background",
        json={"target_word_count": 1200},
    )

    assert response.status_code == 200
    body = response.json()
    assert body["active_story_repair_payload"]["source"] == "current_chapter_quality"
    assert body["active_story_repair_payload"]["scope"] == "chapter"
    assert captured["story_repair_summary"]
    assert captured["story_repair_summary"].count(" / ") >= 1
    assert captured["story_repair_targets"]
    assert captured["story_preserve_strengths"]

    status_response = await chapters_client.get(
        f"/api/chapters/batch-generate/{body['task_id']}/status",
    )
    assert status_response.status_code == 200
    status_body = status_response.json()
    assert status_body["active_story_repair_payload"]["source"] == "current_chapter_quality"
    assert status_body["active_story_repair_payload"]["scope"] == "chapter"


async def test_should_merge_manual_story_repair_summary_with_history_fallback_for_single_background_generation(
    chapters_client,
    chapters_session_factory,
    mock_user,
    monkeypatch,
):
    captured: dict[str, Any] = {}

    async def fake_execute_batch_generation(*args, **kwargs):
        captured.update(kwargs)
        return None

    monkeypatch.setattr(
        batch_generation_run_wiring_service,
        "execute_batch_generation_in_order_with_entry_service_seams",
        fake_execute_batch_generation,
    )

    project = await create_project(chapters_session_factory, user_id=mock_user.user_id)
    outline = await create_outline(
        chapters_session_factory,
        project_id=project.id,
        order_index=1,
        title="当前大纲",
    )
    chapter = await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=1,
        title="待补修章节",
        content=None,
        outline_id=outline.id,
    )
    quality_metrics = {
        "overall_score": 73.0,
        "conflict_chain_hit_rate": 61.0,
        "rule_grounding_hit_rate": 82.0,
        "outline_alignment_rate": 62.0,
        "dialogue_naturalness_rate": 79.0,
        "opening_hook_rate": 75.0,
        "payoff_chain_rate": 57.0,
        "cliffhanger_rate": 80.0,
        "pacing_score": 7.0,
    }

    async with chapters_session_factory() as session:
        session.add(
            GenerationHistory(
                project_id=project.id,
                chapter_id=chapter.id,
                prompt="chapter_generation",
                generated_content=chapters_api._build_generation_history_payload("generated body", quality_metrics),
                model="chapter_generator_v1",
                created_at=datetime.utcnow() - timedelta(minutes=2),
            )
        )
        await session.commit()

    response = await chapters_client.post(
        f"/api/chapters/{chapter.id}/generate-background",
        json={
            "target_word_count": 1200,
            "story_repair_summary": "MANUAL: focus on concrete pressure and emotional reactions",
        },
    )

    assert response.status_code == 200
    body = response.json()
    assert body["active_story_repair_payload"]["source"] == "manual_plus_current_chapter_quality"
    assert body["active_story_repair_payload"]["scope"] == "chapter"
    assert captured["story_repair_summary"] == "MANUAL: focus on concrete pressure and emotional reactions"
    assert captured["story_repair_targets"]
    assert captured["story_preserve_strengths"]


async def test_should_auto_fill_story_repair_payload_from_previous_chapter_history_for_batch_background_generation(
    chapters_client,
    chapters_session_factory,
    mock_user,
    monkeypatch,
):
    captured: dict[str, Any] = {}

    async def fake_execute_batch_generation(*args, **kwargs):
        captured.update(kwargs)
        return None

    monkeypatch.setattr(
        batch_generation_run_wiring_service,
        "execute_batch_generation_in_order_with_entry_service_seams",
        fake_execute_batch_generation,
    )

    project = await create_project(chapters_session_factory, user_id=mock_user.user_id)
    outline1 = await create_outline(
        chapters_session_factory,
        project_id=project.id,
        order_index=1,
        title="开篇",
    )
    outline2 = await create_outline(
        chapters_session_factory,
        project_id=project.id,
        order_index=2,
        title="追查线索",
    )
    outline3 = await create_outline(
        chapters_session_factory,
        project_id=project.id,
        order_index=3,
        title="收束回响",
    )
    previous_chapter = await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=1,
        title="上章",
        content="上一章正文",
        outline_id=outline1.id,
    )
    await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=2,
        title="次章",
        content=None,
        outline_id=outline2.id,
    )
    await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=3,
        title="终章",
        content=None,
        outline_id=outline3.id,
    )
    quality_metrics = {
        "overall_score": 74.5,
        "conflict_chain_hit_rate": 63.0,
        "rule_grounding_hit_rate": 79.0,
        "outline_alignment_rate": 59.0,
        "dialogue_naturalness_rate": 81.0,
        "opening_hook_rate": 77.0,
        "payoff_chain_rate": 54.0,
        "cliffhanger_rate": 83.0,
        "pacing_score": 7.1,
    }

    async with chapters_session_factory() as session:
        session.add(
            GenerationHistory(
                project_id=project.id,
                chapter_id=previous_chapter.id,
                prompt="chapter_generation",
                generated_content=chapters_api._build_generation_history_payload("generated body", quality_metrics),
                model="chapter_generator_v1",
                created_at=datetime.utcnow() - timedelta(minutes=4),
            )
        )
        await session.commit()

    response = await chapters_client.post(
        f"/api/chapters/project/{project.id}/batch-generate",
        json={
            "start_chapter_number": 2,
            "count": 2,
            "target_word_count": 800,
            "enable_analysis": False,
            "enable_mcp": False,
            "max_retries": 1,
        },
    )

    assert response.status_code == 200
    assert captured["story_repair_summary"]
    assert captured["story_repair_summary"].count(" / ") >= 1
    assert captured["story_repair_targets"]
    assert captured["story_preserve_strengths"]


async def test_should_fallback_to_project_generation_defaults_for_single_background_generation(
    chapters_client,
    chapters_session_factory,
    mock_user,
    monkeypatch,
):
    captured: dict[str, Any] = {}

    async def fake_execute_batch_generation(*args, **kwargs):
        captured.update(kwargs)
        return None

    monkeypatch.setattr(
        batch_generation_run_wiring_service,
        "execute_batch_generation_in_order_with_entry_service_seams",
        fake_execute_batch_generation,
    )

    project = await create_project(
        chapters_session_factory,
        user_id=mock_user.user_id,
        default_creative_mode="suspense",
        default_story_focus="reveal_mystery",
        default_plot_stage="climax",
        default_story_creation_brief="默认要求：重点写实对撞与悬念收束。",
    )
    outline = await create_outline(
        chapters_session_factory,
        project_id=project.id,
        order_index=1,
        title="单章后台默认值大纲",
    )
    chapter = await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=1,
        title="待生成章节",
        content=None,
        outline_id=outline.id,
    )

    response = await chapters_client.post(
        f"/api/chapters/{chapter.id}/generate-background",
        json={
            "target_word_count": 1200,
        },
    )

    assert response.status_code == 200
    assert isinstance(captured["story_packet"], StoryPacket)
    assert captured["story_packet"].guidance.creative_mode == "suspense"
    assert captured["story_packet"].guidance.story_focus == "reveal_mystery"
    assert captured["story_packet"].guidance.plot_stage == "climax"
    assert captured["story_packet"].guidance.story_creation_brief == "默认要求：重点写实对撞与悬念收束。"


async def test_should_reuse_active_background_task_for_same_chapter(
    chapters_client,
    chapters_session_factory,
    mock_user,
):
    project = await create_project(chapters_session_factory, user_id=mock_user.user_id)
    outline = await create_outline(
        chapters_session_factory,
        project_id=project.id,
        order_index=1,
        title="复用任务大纲",
    )
    chapter = await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=1,
        title="待复用任务章节",
        content=None,
        outline_id=outline.id,
    )

    first = await chapters_client.post(
        f"/api/chapters/{chapter.id}/generate-background",
        json={"target_word_count": 900},
    )
    assert first.status_code == 200
    first_task_id = first.json()["task_id"]

    second = await chapters_client.post(
        f"/api/chapters/{chapter.id}/generate-background",
        json={"target_word_count": 900},
    )
    assert second.status_code == 200
    second_body = second.json()
    assert second_body["task_id"] == first_task_id
    assert "已有后台生成任务" in second_body["message"]













async def test_should_apply_project_story_packet_defaults_in_regeneration_prompt_context(
    chapters_client,
    chapters_session_factory,
    mock_user,
    monkeypatch,
):
    captured: dict[str, Any] = {}
    project = await create_project(
        chapters_session_factory,
        user_id=mock_user.user_id,
        default_creative_mode="hook",
        default_story_focus="advance_plot",
        default_plot_stage="development",
        default_story_creation_brief="Default brief: keep the pace moving.",
        default_quality_preset="tight_prose",
        default_quality_notes="Reduce exposition.",
    )
    chapter = await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=1,
        title="chapter-to-regenerate",
        content="legacy content",
        status="completed",
    )

    class FakeRegenerator:
        def __init__(self, ai_service):
            self.ai_service = ai_service

        async def regenerate_with_feedback(self, **kwargs):
            captured.update(kwargs)
            yield {"type": "progress", "progress": 30, "message": "preparing"}
            yield {"type": "chunk", "content": "new content"}

        def calculate_content_diff(self, original_content, new_content):
            return {"similarity": 8.0, "difference": 92.0}

    monkeypatch.setattr(
        chapter_regeneration_routes_api,
        "REGENERATOR_FACTORY",
        FakeRegenerator,
    )

    response = await chapters_client.post(
        f"/api/chapters/{chapter.id}/regenerate-stream",
        json={
            "modification_source": "custom",
            "custom_instructions": "improve pacing",
            "target_word_count": 500,
            "focus_areas": ["pacing"],
            "auto_apply": False,
        },
    )
    assert response.status_code == 200

    events = parse_sse_data(response.text)
    assert any(event.get("type") == "result" for event in events)

    prompt_kwargs = captured["project_context"]["prompt_quality_kwargs"]
    assert prompt_kwargs["creative_mode"] == "hook"
    assert prompt_kwargs["story_focus"] == "advance_plot"
    assert prompt_kwargs["plot_stage"] == "development"
    assert prompt_kwargs["story_creation_brief"] == "Default brief: keep the pace moving."
    assert prompt_kwargs["quality_preset"] == "tight_prose"
    assert prompt_kwargs["quality_notes"] == "Reduce exposition."
    assert "【长线目标锚点】" in prompt_kwargs["story_long_term_goal_block"]
    assert "【章节节奏预算】" in prompt_kwargs["story_pacing_budget_block"]




async def test_should_include_web_research_assets_in_regeneration_prompt_context(
    chapters_client,
    chapters_session_factory,
    mock_user,
    monkeypatch,
):
    captured: dict[str, Any] = {}
    captured_research: dict[str, Any] = {}
    project = await create_project(chapters_session_factory, user_id=mock_user.user_id)
    chapter = await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=1,
        title="chapter-to-regenerate-with-research",
        content="legacy content",
        status="completed",
    )

    fake_assets = [
        {
            "title": "Harbor Guild Rules",
            "url": "https://example.com/harbor-guild-rules",
            "snippet": "Guild protocol and tariff etiquette.",
        }
    ]

    class FakeRegenerator:
        def __init__(self, ai_service):
            self.ai_service = ai_service

        async def regenerate_with_feedback(self, **kwargs):
            captured.update(kwargs)
            yield {"type": "progress", "progress": 30, "message": "preparing"}
            yield {"type": "chunk", "content": "new content"}

        def calculate_content_diff(self, original_content, new_content):
            return {"similarity": 8.0, "difference": 92.0}

    async def fake_collect_for_chapter(**kwargs):
        captured_research.update(kwargs)
        return {
            "query": "late qing harbor guild rules",
            "archive_path": "tmp/regeneration-research.json",
            "assets": fake_assets,
        }

    from app.services import chapter_regeneration_context_service as chapter_regeneration_context_service

    monkeypatch.setattr(
        chapter_regeneration_routes_api,
        "REGENERATOR_FACTORY",
        FakeRegenerator,
    )
    monkeypatch.setattr(
        chapter_regeneration_context_service.chapter_web_research_service,
        "collect_for_chapter",
        fake_collect_for_chapter,
    )

    response = await chapters_client.post(
        f"/api/chapters/{chapter.id}/regenerate-stream",
        json={
            "modification_source": "custom",
            "custom_instructions": "improve grounding and continuity",
            "target_word_count": 500,
            "focus_areas": ["rule_grounding"],
            "auto_apply": False,
            "enable_web_research": True,
            "web_research_query": "late qing harbor guild rules",
        },
    )
    assert response.status_code == 200

    events = parse_sse_data(response.text)
    assert any(event.get("type") == "result" for event in events)
    assert captured_research["enable_web_research"] is True
    assert captured_research["web_research_query"] == "late qing harbor guild rules"
    assert captured["project_context"]["external_assets"] == fake_assets
    assert captured["project_context"]["reference_assets"] == fake_assets

async def test_should_merge_quality_gate_snapshot_into_regeneration_prompt_context(
    chapters_client,
    chapters_session_factory,
    mock_user,
    monkeypatch,
):
    captured: dict[str, Any] = {}
    project = await create_project(chapters_session_factory, user_id=mock_user.user_id)
    chapter = await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=1,
        title="quality-gate-regeneration",
        content="legacy content",
        status="completed",
    )
    quality_metrics = {
        "overall_score": 74.5,
        "conflict_chain_hit_rate": 63.0,
        "rule_grounding_hit_rate": 79.0,
        "outline_alignment_rate": 59.0,
        "dialogue_naturalness_rate": 81.0,
        "opening_hook_rate": 77.0,
        "payoff_chain_rate": 54.0,
        "cliffhanger_rate": 83.0,
        "pacing_score": 7.1,
    }

    async with chapters_session_factory() as session:
        session.add(
            GenerationHistory(
                project_id=project.id,
                chapter_id=chapter.id,
                prompt="chapter_generation",
                generated_content=chapters_api._build_generation_history_payload("generated body", quality_metrics),
                model="chapter_generator_v1",
                created_at=datetime.utcnow() - timedelta(minutes=2),
            )
        )
        await session.commit()

    class FakeRegenerator:
        def __init__(self, ai_service):
            self.ai_service = ai_service

        async def regenerate_with_feedback(self, **kwargs):
            captured.update(kwargs)
            yield {"type": "progress", "progress": 30, "message": "preparing"}
            yield {"type": "chunk", "content": "new content"}

        def calculate_content_diff(self, original_content, new_content):
            return {"similarity": 8.0, "difference": 92.0}

    monkeypatch.setattr(
        chapter_regeneration_routes_api,
        "REGENERATOR_FACTORY",
        FakeRegenerator,
    )

    response = await chapters_client.post(
        f"/api/chapters/{chapter.id}/regenerate-stream",
        json={
            "modification_source": "custom",
            "custom_instructions": "tighten the chapter",
            "target_word_count": 500,
            "focus_areas": [],
            "auto_apply": False,
        },
    )
    assert response.status_code == 200

    events = parse_sse_data(response.text)
    assert any(event.get("type") == "result" for event in events)

    prompt_kwargs = captured["project_context"]["prompt_quality_kwargs"]
    assert prompt_kwargs["story_repair_diagnostic_block"]
    assert prompt_kwargs["story_repair_target_block"]
    assert captured["regenerate_request"].story_repair_summary
    assert captured["regenerate_request"].story_repair_targets
    assert captured["regenerate_request"].story_preserve_strengths


async def test_should_reuse_quality_history_context_in_regeneration_prompt(
    chapters_client,
    chapters_session_factory,
    mock_user,
    monkeypatch,
):
    captured: dict[str, Any] = {}
    project = await create_project(chapters_session_factory, user_id=mock_user.user_id)
    chapter = await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=1,
        title="history-aware-regeneration",
        content="legacy content",
        status="completed",
    )

    quality_metrics = {
        "overall_score": 76.0,
        "conflict_chain_hit_rate": 68.0,
        "rule_grounding_hit_rate": 79.0,
        "outline_alignment_rate": 72.0,
        "dialogue_naturalness_rate": 80.0,
        "opening_hook_rate": 74.0,
        "payoff_chain_rate": 66.0,
        "cliffhanger_rate": 71.0,
        "pacing_score": 7.8,
        "repair_guidance": {"focus_areas": ["payoff", "continuity"]},
        "continuity_preflight": {
            "status": "warning",
            "warning_count": 1,
            "repair_targets": ["Carry forward the hidden-key pressure."],
            "summary": "Current chapter misses explicit handoff for 1 continuity ledger items.",
        },
        "quality_runtime_context": {
            "plot_stage": "development",
            "chapter_count": 12,
            "current_chapter_number": 1,
            "foreshadow_payoff_plan": ["recover the hidden key"],
            "organization_state_ledger": ["ShadowGuild: control tightened around the docks"],
            "career_state_ledger": ["Lin/Strategist: stage 3 with supply-chain pressure"],
        },
    }

    async with chapters_session_factory() as session:
        session.add(
            GenerationHistory(
                project_id=project.id,
                chapter_id=chapter.id,
                prompt="history prompt",
                generated_content=chapters_api._build_generation_history_payload("generated body", quality_metrics),
                model="default",
            )
        )
        await session.commit()

    class FakeRegenerator:
        def __init__(self, ai_service):
            self.ai_service = ai_service

        async def regenerate_with_feedback(self, **kwargs):
            captured.update(kwargs)
            yield {"type": "progress", "progress": 30, "message": "preparing"}
            yield {"type": "chunk", "content": "new content"}

        def calculate_content_diff(self, original_content, new_content):
            return {"similarity": 8.0, "difference": 92.0}

    monkeypatch.setattr(
        chapter_regeneration_routes_api,
        "REGENERATOR_FACTORY",
        FakeRegenerator,
    )

    response = await chapters_client.post(
        f"/api/chapters/{chapter.id}/regenerate-stream",
        json={
            "modification_source": "custom",
            "custom_instructions": "improve payoff and continuity",
            "target_word_count": 500,
            "focus_areas": ["payoff"],
            "auto_apply": False,
        },
    )
    assert response.status_code == 200

    events = parse_sse_data(response.text)
    assert any(event.get("type") == "result" for event in events)

    prompt_kwargs = captured["project_context"]["prompt_quality_kwargs"]
    assert "recover the hidden key" in prompt_kwargs["story_foreshadow_payoff_plan_block"]
    assert "ShadowGuild: control tightened around the docks" in prompt_kwargs["story_organization_state_ledger_block"]
    assert "Lin/Strategist: stage 3 with supply-chain pressure" in prompt_kwargs["story_career_state_ledger_block"]
    assert "【章节近期质量趋势】" in prompt_kwargs["story_quality_trend_block"]
    assert "最近节奏稳定度均值：7.8/10" in prompt_kwargs["story_quality_trend_block"]
    assert "Carry forward the hidden-key pressure" in prompt_kwargs["story_quality_trend_block"]


async def test_should_sanitize_regenerated_content_before_persisting_task(
    chapters_client,
    chapters_session_factory,
    mock_user,
    monkeypatch,
):
    project = await create_project(chapters_session_factory, user_id=mock_user.user_id)
    chapter = await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=1,
        title="\u6574\u7ae0\u91cd\u5199\u6e05\u6d17\u6d4b\u8bd5",
        content="\u539f\u6587",
        status="completed",
    )

    class FakeRegenerator:
        def __init__(self, ai_service):
            self.ai_service = ai_service

        async def regenerate_with_feedback(self, **kwargs):
            yield {
                "type": "chunk",
                "content": "\u4e0b\u4e00\u79d2\uff0c\u95e8\u5916\u6709\u4eba\u6572\u4e86\u4e24\u4e0b\u73bb\u7483\u3002\n",
            }
            yield {
                "type": "chunk",
                "content": "\u4e0b\u4e00\u79d2\uff0c\u6536\u94f6\u53f0\u4e0b\u7684\u706f\u706d\u4e86\u3002\n",
            }
            yield {
                "type": "chunk",
                "content": "\u5730\u4e0a\u7684\u6c34\u75d5\u50cf\u6709\u4ec0\u4e48\u4e1c\u897f\u62d6\u8fc7\u53bb\u3002",
            }

        def calculate_content_diff(self, original_content, new_content):
            return {"similarity": 10.0, "difference": 90.0}

    monkeypatch.setattr(
        chapter_regeneration_routes_api,
        "REGENERATOR_FACTORY",
        FakeRegenerator,
    )

    response = await chapters_client.post(
        f"/api/chapters/{chapter.id}/regenerate-stream",
        json={
            "modification_source": "custom",
            "custom_instructions": "\u4f18\u5316\u8282\u594f",
            "target_word_count": 500,
            "focus_areas": ["pacing"],
            "auto_apply": False,
        },
    )
    assert response.status_code == 200

    events = parse_sse_data(response.text)
    result_event = next(event for event in events if event.get("type") == "result")
    task_id = result_event["data"]["task_id"]

    async with chapters_session_factory() as session:
        task = await session.get(RegenerationTask, task_id)
        assert task is not None
        assert task.regenerated_content == (
            "\u4e0b\u4e00\u79d2\uff0c\u95e8\u5916\u6709\u4eba\u6572\u4e86\u4e24\u4e0b\u73bb\u7483\u3002\n"
            "\u6536\u94f6\u53f0\u4e0b\u7684\u706f\u706d\u4e86\u3002\n"
            "\u5730\u4e0a\u7684\u6c34\u75d5\u50cf\u6709\u4e1c\u897f\u62d6\u8fc7\u53bb\u3002"
        )
async def test_should_return_400_when_regenerate_chapter_content_is_empty(
    chapters_client,
    chapters_session_factory,
    mock_user,
):
    project = await create_project(chapters_session_factory, user_id=mock_user.user_id)
    chapter = await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=1,
        title="空内容章节",
        content="",
        status="draft",
    )

    response = await chapters_client.post(
        f"/api/chapters/{chapter.id}/regenerate-stream",
        json={
            "modification_source": "custom",
            "custom_instructions": "优化节奏",
            "target_word_count": 500,
            "auto_apply": False,
        },
    )
    assert response.status_code == 400


async def test_should_return_analysis_checker_and_auto_revision_payloads(
    chapters_client,
    chapters_session_factory,
    mock_user,
):
    project = await create_project(chapters_session_factory, user_id=mock_user.user_id)
    chapter = await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=1,
        title="分析闭环章节",
        content="旧正文",
        status="completed",
    )

    analysis = PlotAnalysis(
        project_id=project.id,
        chapter_id=chapter.id,
        plot_stage="发展",
        conflict_level=7,
        conflict_types=["人与人"],
        emotional_tone="紧张",
        hooks=[{"type": "悬念", "content": "门后有异响", "strength": 8, "position": "结尾"}],
        hooks_count=1,
        foreshadows=[{"content": "镜面异光", "type": "planted", "strength": 7}],
        foreshadows_planted=1,
        plot_points=[{"content": "主角决定独自断后", "importance": 0.9, "type": "conflict"}],
        plot_points_count=1,
        character_states=[],
        scenes=[{"location": "走廊", "atmosphere": "压抑"}],
        pacing="fast",
        overall_quality_score=8.6,
        pacing_score=8.1,
        engagement_score=8.8,
        coherence_score=8.2,
        analysis_report="分析报告",
        suggestions=["增加回收"],
        word_count=len(chapter.content or ""),
        dialogue_ratio=0.22,
        description_ratio=0.41,
        created_at=datetime.utcnow() - timedelta(minutes=1),
    )
    checker_result = {
        "overall_assessment": "存在关键断裂",
        "severity_counts": {"critical": 1, "warning": 2, "info": 0},
        "issues": [
            {
                "severity": "critical",
                "title": "冲突动机不足",
                "evidence": "转折过快",
                "suggestion": "补足主角犹豫过程",
            }
        ],
        "priority_actions": ["补冲突因果"],
        "revision_suggestions": ["补一段心理描写"],
    }
    reviser_result = {
        "critical_count": 1,
        "major_count": 0,
        "priority_issue_count": 1,
        "applied_critical_count": 1,
        "applied_issue_count": 1,
        "change_summary": "bridge the emotional and action transition",
        "revised_text": "Revised chapter text with the approaching noise behind the door.",
        "revised_text_preview": "Revised chapter text",
        "revised_word_count": 28,
        "unresolved_issues": [],
    }
    quality_metrics = {
        "overall_score": 82.4,
        "conflict_chain_hit_rate": 76.0,
        "rule_grounding_hit_rate": 88.0,
        "outline_alignment_rate": 81.0,
        "dialogue_naturalness_rate": 79.0,
        "opening_hook_rate": 92.0,
        "payoff_chain_rate": 74.0,
        "cliffhanger_rate": 83.0,
        "pacing_score": 8.2,
    }

    candidate_content = (
        "Candidate draft restores the alliance fracture after the dock control change, "
        "and the hidden key oath now surfaces with a visible cost."
    )
    candidate_metrics = {
        **quality_metrics,
        "quality_gate": {
            "status": "blocked",
            "decision": "manual_review",
            "failed_metrics": [
                {
                    "key": "conflict_chain_hit_rate",
                    "label": "Conflict chain",
                    "value": 61.0,
                    "threshold": 68.0,
                    "gap": 7.0,
                    "focus_area": "conflict",
                    "repair_target": "Strengthen the transition beat",
                }
            ],
        },
        "candidate_selection": {
            "candidate_index": 2,
            "candidate_count": 2,
            "selection_score": 84.6,
        },
        "quality_runtime_context": {
            "relationship_state_ledger": [
                "Alliance fracture: Lin and Su are still at odds",
                "Watchtower alarm: the crew expects the alarm signal tonight",
            ],
            "organization_state_ledger": [
                "Dock control change: Su now controls the docks",
            ],
            "foreshadow_state_ledger": [
                "Hidden key oath: the price of the hidden key still hangs over Lin",
            ],
            "foreshadow_payoff_plan": [
                "Hidden key payoff: reveal the price of the hidden key oath",
                "Royal seal payoff: identify who now holds the royal seal",
            ],
        },
        "continuity_preflight": {
            "status": "warning",
            "summary": "Need to keep the alliance fracture and dock control change in play.",
            "repair_targets": [
                "Carry forward the alliance fracture in action.",
                "Mention the dock control change in a consequential beat.",
            ],
            "warnings": [
                {
                    "item": "Watchtower alarm",
                    "focus_area": "relationship_continuity",
                }
            ],
        },
        "foreshadow_payoff_delay": {
            "status": "warning",
            "summary": "Key payoff still needs to land on the page.",
            "repair_targets": [
                "Reveal the price of the hidden key oath.",
                "Clarify who now holds the royal seal.",
            ],
        },
    }

    async with chapters_session_factory() as session:
        session.add(analysis)
        session.add(
            StoryMemory(
                project_id=project.id,
                chapter_id=chapter.id,
                memory_type="hook",
                title="结尾悬念",
                content="门后有异响",
                related_characters=[],
                related_locations=["走廊"],
                tags=["悬念"],
                importance_score=0.9,
                story_timeline=1,
                chapter_position=6,
                text_length=5,
                is_foreshadow=0,
                vector_id=f"vec-{chapter.id}",
            )
        )
        session.add(
            GenerationHistory(
                project_id=project.id,
                chapter_id=chapter.id,
                prompt="chapter_generation",
                generated_content=chapters_api._build_generation_history_payload("generated body", quality_metrics),
                model="chapter_generator_v1",
                created_at=datetime.utcnow() - timedelta(minutes=3),
            )
        )
        session.add(
            GenerationHistory(
                project_id=project.id,
                chapter_id=chapter.id,
                prompt="checker",
                generated_content=chapters_api._build_checker_history_payload(checker_result),
                model="chapter_text_checker_v1",
                created_at=datetime.utcnow() - timedelta(minutes=2),
            )
        )
        session.add(
            GenerationHistory(
                project_id=project.id,
                chapter_id=chapter.id,
                prompt="reviser",
                generated_content=chapters_api._build_reviser_history_payload(reviser_result),
                model="chapter_text_reviser_v1",
                created_at=datetime.utcnow() - timedelta(minutes=1),
            )
        )
        session.add(
            ChapterDraftAttempt(
                project_id=project.id,
                chapter_id=chapter.id,
                source="chapter",
                attempt_state="manual_review",
                quality_gate_action="manual_review",
                quality_gate_decision="manual_review",
                word_count=len(candidate_content),
                summary_preview="candidate summary",
                content_preview=candidate_content[:4000],
                quality_metrics=candidate_metrics,
                repair_payload={
                    "summary": "Candidate draft for quality gate follow-up",
                    "repair_targets": ["Improve the transition"],
                    "preserve_strengths": ["Keep the suspense"],
                    "candidate_full_content": candidate_content,
                    "content_complete": True,
                },
                created_at=datetime.utcnow() - timedelta(seconds=30),
            )
        )
        await session.commit()

    response = await chapters_client.get(f"/api/chapters/{chapter.id}/analysis")
    assert response.status_code == 200
    body = response.json()
    assert body["chapter_id"] == chapter.id
    assert body["analysis"]["plot_stage"] == "发展"
    assert body["memories"][0]["title"] == "结尾悬念"
    assert body["checker_result"]["severity_counts"]["critical"] == 1
    assert body["auto_revision_draft"]["critical_count"] == 1
    assert body["auto_revision_draft"]["major_count"] == 0
    assert body["auto_revision_draft"]["priority_issue_count"] == 1
    assert body["auto_revision_draft"]["applied_issue_count"] == 1
    assert body["auto_revision_draft"]["is_stale"] is True
    assert body["auto_revision_draft"].get("revised_text") is None
    assert body["candidate_draft"]["repair_targets"] == ["Improve the transition"]
    assert body["candidate_draft"]["can_apply"] is True
    assert body["candidate_draft"].get("content") is None
    continuity_highlights = body["candidate_draft"]["quality_highlights"]["continuity"]
    foreshadow_highlights = body["candidate_draft"]["quality_highlights"]["foreshadow"]
    assert any("Alliance fracture" in item for item in continuity_highlights["matched_items"])
    assert any("Watchtower alarm" in item for item in continuity_highlights["missing_items"])
    assert continuity_highlights["repair_targets"] == [
        "Carry forward the alliance fracture in action.",
        "Mention the dock control change in a consequential beat.",
    ]
    assert continuity_highlights["matched_evidence"]
    assert any("dock control change" in evidence["snippet"] for evidence in continuity_highlights["matched_evidence"])
    assert any("Hidden key" in item for item in foreshadow_highlights["matched_items"])
    assert any("Royal seal" in item for item in foreshadow_highlights["missing_items"])
    assert foreshadow_highlights["matched_evidence"]
    assert any("hidden key oath" in evidence["snippet"].lower() for evidence in foreshadow_highlights["matched_evidence"])
    assert body["quality_metrics"]["overall_score"] == 82.4
    assert body["quality_metrics"]["repair_guidance"]["summary"]
    assert body["quality_metrics"]["repair_guidance"]["focus_areas"]
    assert body["quality_metrics"]["quality_gate"]["status"]
    assert body["quality_metrics"]["quality_gate"]["decision"]
    assert body["quality_metrics_summary"]["chapter_count"] == 1
    assert body["quality_metrics_summary"]["avg_pacing_score"] == 8.2
    assert body["quality_metrics_summary"]["repair_guidance"]["summary"]
    assert body["quality_metrics_summary"]["quality_gate"]["status"]

    full_response = await chapters_client.get(f"/api/chapters/{chapter.id}/analysis?include_full_draft=true")
    assert full_response.status_code == 200
    assert (
        full_response.json()["auto_revision_draft"]["revised_text"]
        == reviser_result["revised_text"]
    )
    assert full_response.json()["candidate_draft"]["content"] == candidate_content
    assert full_response.json()["candidate_draft"]["quality_highlights"]["foreshadow"]["summary"] == "Key payoff still needs to land on the page."




async def test_should_delegate_analysis_route_to_compat_service(
    chapters_client,
    monkeypatch,
):
    captured: dict[str, Any] = {}

    async def fake_get_chapter_analysis_with_default_route_wiring(**kwargs):
        captured.update(kwargs)
        return {
            "chapter_id": kwargs["chapter_id"],
            "analysis": {"plot_stage": "development"},
            "memories": [],
            "checker_result": None,
            "checker_created_at": None,
            "auto_revision_draft": None,
            "candidate_draft": None,
            "quality_metrics": None,
            "quality_metrics_summary": {"chapter_count": 0},
            "created_at": None,
        }

    monkeypatch.setattr(
        chapter_analysis_routes_api,
        "get_chapter_analysis_with_default_route_wiring",
        fake_get_chapter_analysis_with_default_route_wiring,
    )

    response = await chapters_client.get(
        "/api/chapters/route-delegate/analysis",
        params={"include_full_draft": True},
    )

    assert response.status_code == 200
    assert response.json()["chapter_id"] == "route-delegate"
    assert captured["chapter_id"] == "route-delegate"
    assert captured["include_full_draft"] is True
    assert captured["request"] is not None
    assert captured["db_session"] is not None


async def test_should_delegate_annotation_route_to_compat_service(
    chapters_client,
    monkeypatch,
):
    captured: dict[str, Any] = {}

    async def fake_get_chapter_annotations_with_default_route_wiring(**kwargs):
        captured.update(kwargs)
        return {
            "chapter_id": kwargs["chapter_id"],
            "chapter_number": 1,
            "title": "route-delegate",
            "word_count": 0,
            "annotations": [],
            "has_analysis": False,
            "summary": {"total_annotations": 0, "hooks": 0, "foreshadows": 0, "plot_points": 0, "character_events": 0},
        }

    monkeypatch.setattr(
        chapter_annotation_routes_api,
        "get_chapter_annotations_with_default_route_wiring",
        fake_get_chapter_annotations_with_default_route_wiring,
    )

    response = await chapters_client.get('/api/chapters/annotation-route/annotations')

    assert response.status_code == 200
    assert response.json()["chapter_id"] == "annotation-route"
    assert captured["chapter_id"] == "annotation-route"
    assert captured["request"] is not None
    assert captured["db_session"] is not None


async def test_should_generate_second_candidate_with_retry_prompt_and_strategy(monkeypatch):
    class StubAIService:
        def __init__(self):
            self.calls: list[dict[str, Any]] = []

        async def generate_text_stream(self, **kwargs):
            self.calls.append(kwargs)
            yield f"candidate-{len(self.calls)}"

    ai_service = StubAIService()

    def evaluate_candidate_quality(content: str) -> dict[str, Any]:
        if content.endswith("1"):
            overall_score = 72.0
            decision = "manual_review"
        else:
            overall_score = 88.0
            decision = "allow_save"
        return {
            "overall_score": overall_score,
            "pacing_score": 8.0,
            "quality_runtime_context": {
                "quality_preset": "emotion_drama",
                "creative_mode": "relationship",
                "story_focus": "relationship_shift",
            },
            "quality_gate": {
                "status": "blocked" if decision != "allow_save" else "pass",
                "decision": decision,
                "failed_metrics": [{"label": "Conflict chain"}] if decision != "allow_save" else [],
            },
        }

    def build_candidate_quality_gate_plan(metrics: dict[str, Any], _attempt_offset: int) -> dict[str, Any]:
        quality_gate = metrics.get("quality_gate") if isinstance(metrics.get("quality_gate"), dict) else {}
        if quality_gate.get("decision") == "allow_save":
            return {"quality_gate": quality_gate, "message": "passed"}
        return {
            "quality_gate": quality_gate,
            "message": "need retry",
            "active_story_repair_payload": {
                "summary": "Retry with stronger transition",
                "repair_targets": ["Improve the transition"],
                "preserve_strengths": ["Keep the suspense"],
            },
        }

    result = await chapters_api._generate_best_ranked_candidate(
        ai_service=ai_service,
        base_generate_kwargs={"prompt": "base prompt", "temperature": 0.8},
        target_word_count=1200,
        source="chapter",
        generation_label="test-rerank",
        quality_evaluator=evaluate_candidate_quality,
        quality_gate_plan_builder=build_candidate_quality_gate_plan,
        max_candidates=2,
    )

    assert len(ai_service.calls) >= 2
    assert "Revision attempt #2" in ai_service.calls[1]["prompt"]
    assert "Alternative candidate strategy #2" in ai_service.calls[1]["prompt"]
    assert ai_service.calls[1]["temperature"] != 0.8
    assert result["candidate_index"] >= 2



async def test_should_prefer_word_budget_repair_over_full_second_candidate_for_pure_budget_issue():
    class StubAIService:
        def __init__(self):
            self.calls: list[dict[str, Any]] = []

        async def generate_text_stream(self, **kwargs):
            self.calls.append(kwargs)
            if len(self.calls) == 1:
                yield "A" * 1759
            else:
                yield "B" * 1240

    ai_service = StubAIService()

    def evaluate_candidate_quality(content: str) -> dict[str, Any]:
        if content.startswith("A"):
            decision = "manual_review"
            overall_score = 74.0
        else:
            decision = "allow_save"
            overall_score = 82.0
        return {
            "overall_score": overall_score,
            "pacing_score": 7.8,
            "quality_runtime_context": {
                "quality_preset": "plot_drive",
                "creative_mode": "hook",
                "story_focus": "advance_plot",
            },
            "quality_gate": {
                "status": "blocked" if decision != "allow_save" else "pass",
                "decision": decision,
                "failed_metrics": [],
            },
        }

    def build_candidate_quality_gate_plan(metrics: dict[str, Any], _attempt_offset: int) -> dict[str, Any]:
        quality_gate = metrics.get("quality_gate") if isinstance(metrics.get("quality_gate"), dict) else {}
        if quality_gate.get("decision") == "allow_save":
            return {"quality_gate": quality_gate, "message": "passed"}
        return {"quality_gate": quality_gate, "message": "budget drift only"}

    result = await chapters_api._generate_best_ranked_candidate(
        ai_service=ai_service,
        base_generate_kwargs={"prompt": "base prompt", "temperature": 0.8},
        target_word_count=1200,
        source="chapter",
        generation_label="test-budget-repair",
        quality_evaluator=evaluate_candidate_quality,
        quality_gate_plan_builder=build_candidate_quality_gate_plan,
        max_candidates=2,
    )

    assert len(ai_service.calls) == 2
    assert "Word-budget repair pass #2" in ai_service.calls[1]["prompt"]
    assert "Alternative candidate strategy #2" not in ai_service.calls[1]["prompt"]
    assert result["candidate_index"] == 2
    assert result["word_count"] == 1240
    assert result["generation_path"] == "word_budget_repair"
    assert result["attempt_kind"] == "word_budget_repair"
    assert result["rerank_used"] is False
    assert result["word_budget_repair_used"] is True
    assert result["winner_candidate_index"] == 2
    assert result["quality_metrics"]["candidate_selection"]["generation_path"] == "word_budget_repair"
    assert result["quality_metrics"]["candidate_selection"]["attempt_kind"] == "word_budget_repair"



async def test_should_not_keep_overlong_rerank_candidate_after_quality_gate_recompute():
    class StubAIService:
        def __init__(self):
            self.calls: list[dict[str, Any]] = []

        async def generate_text_stream(self, **kwargs):
            self.calls.append(kwargs)
            if len(self.calls) == 1:
                yield "A" * 2655
            elif len(self.calls) == 2:
                yield "B" * 2023
            elif len(self.calls) == 3:
                yield "C" * 1422
            else:
                yield "D" * 1434

    ai_service = StubAIService()

    def evaluate_candidate_quality(content: str) -> dict[str, Any]:
        if content.startswith("A"):
            decision = "manual_review"
            overall_score = 83.2
            failed_metrics = [
                {"label": "Cliffhanger", "focus_area": "cliffhanger"},
                {"label": "Rule grounding", "focus_area": "rule_grounding"},
            ]
        elif content.startswith("B"):
            decision = "allow_save"
            overall_score = 98.3
            failed_metrics = []
        elif content.startswith("C"):
            decision = "manual_review"
            overall_score = 96.1
            failed_metrics = [
                {"label": "Cliffhanger", "focus_area": "cliffhanger"},
            ]
        else:
            decision = "manual_review"
            overall_score = 93.1
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
        generation_label="test-recompute-before-rerank",
        quality_evaluator=evaluate_candidate_quality,
        quality_gate_plan_builder=build_candidate_quality_gate_plan,
        max_candidates=2,
    )

    assert len(ai_service.calls) >= 4
    assert result["candidate_index"] != 2
    assert result["word_count"] != 2023
    assert result["quality_metrics"]["candidate_selection"]["quality_gate_decision"] == "manual_review"
    candidate_pool = result["quality_metrics"]["candidate_pool_summary"]
    rerank_candidate = next(item for item in candidate_pool if item["candidate_index"] == 2)
    assert rerank_candidate["quality_gate_decision"] == "auto_repair"
    assert rerank_candidate["is_winner"] is False


























async def test_should_get_and_apply_auto_revision_draft(
    chapters_client,
    chapters_session_factory,
    mock_user,
):
    project = await create_project(chapters_session_factory, user_id=mock_user.user_id)
    chapter = await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=1,
        title="待应用草稿章节",
        content="旧正文",
        status="completed",
    )

    reviser_result = {
        "critical_count": 2,
        "major_count": 1,
        "priority_issue_count": 3,
        "applied_critical_count": 2,
        "applied_issue_count": 3,
        "change_summary": "修复关键断裂",
        "revised_text": "新正文已经覆盖旧正文，并补足了承接。",
        "revised_text_preview": "新正文已经覆盖旧正文",
        "revised_word_count": 18,
        "unresolved_issues": [],
    }

    async with chapters_session_factory() as session:
        reviser_history = GenerationHistory(
            project_id=project.id,
            chapter_id=chapter.id,
            prompt="reviser",
            generated_content=chapters_api._build_reviser_history_payload(reviser_result),
            model="chapter_text_reviser_v1",
        )
        session.add(reviser_history)
        await session.commit()
        await session.refresh(reviser_history)
        history_id = reviser_history.id

    draft_response = await chapters_client.get(
        f"/api/chapters/{chapter.id}/analysis/auto-revision-draft"
    )
    assert draft_response.status_code == 200
    draft = draft_response.json()["auto_revision_draft"]
    assert draft["history_id"] == history_id
    assert draft["priority_issue_count"] == 3
    assert draft["major_count"] == 1
    assert draft["applied_issue_count"] == 3
    assert draft["revised_text"] == reviser_result["revised_text"]
    assert draft["is_stale"] is False

    apply_response = await chapters_client.post(
        f"/api/chapters/{chapter.id}/analysis/auto-revision-draft/apply",
        json={"history_id": history_id},
    )
    assert apply_response.status_code == 200
    apply_body = apply_response.json()
    assert apply_body["success"] is True
    assert apply_body["draft_history_id"] == history_id
    assert apply_body["word_count"] == len(reviser_result["revised_text"])

    async with chapters_session_factory() as session:
        saved_chapter = await session.get(Chapter, chapter.id)
        assert saved_chapter is not None
        assert saved_chapter.content == reviser_result["revised_text"]

        history_result = await session.execute(
            select(GenerationHistory)
            .where(GenerationHistory.chapter_id == chapter.id)
            .order_by(GenerationHistory.created_at.desc())
        )
        histories = history_result.scalars().all()
        assert any(history.model == "chapter_text_reviser_apply_v1" for history in histories)







async def test_should_generate_auto_revision_draft_when_only_major_issues_exist(
    chapters_session_factory,
    monkeypatch,
):
    captured_prompt: dict[str, str] = {}

    class StubAIService:
        async def call_with_json_retry(self, **kwargs):
            captured_prompt["prompt"] = kwargs["prompt"]
            return {
                "revised_text": "她把门把手握得更紧，呼吸也跟着停了一拍。门外没有再响，可那份迟疑终于落在了动作上。",
                "applied_issues": ["补足人物迟疑过程", "把异响的即时反应落到动作里"],
                "unresolved_issues": ["结尾悬念还能再收紧一点"],
                "change_summary": "已补足 major 级承接与动作反应",
            }

    async def fake_get_template(*args, **kwargs):
        return chapters_api.PromptService.CHAPTER_TEXT_REVISER

    monkeypatch.setattr(chapters_api.PromptService, "get_template", fake_get_template)

    checker_result = {
        "severity_counts": {"critical": 0, "major": 2, "minor": 1},
        "issues": [
            {
                "severity": "major",
                "category": "衔接",
                "location": "开头第2段",
                "impact": "人物从听见异响到决定开门缺少迟疑过程",
                "suggestion": "补足人物迟疑过程",
            },
            {
                "severity": "major",
                "category": "表现",
                "location": "结尾第1段",
                "impact": "异响出现后缺少即时动作反馈",
                "suggestion": "把异响的即时反应落到动作里",
            },
            {
                "severity": "minor",
                "category": "文风",
                "location": "结尾句",
                "impact": "个别表达偏模板化",
                "suggestion": "压缩套句",
            },
        ],
    }

    async with chapters_session_factory() as session:
        reviser_result = await chapters_api._run_chapter_text_reviser(
            ai_service=StubAIService(),
            db_session=session,
            user_id="test-user",
            chapter_number=1,
            chapter_title="凌晨三点半的多余顾客",
            chapter_content="门外的异响又响了一次，她把手从门把上挪开，又按了回去。",
            checker_result=checker_result,
        )

    assert reviser_result is not None
    assert reviser_result["critical_count"] == 0
    assert reviser_result["major_count"] == 2
    assert reviser_result["priority_issue_count"] == 2
    assert reviser_result["applied_issue_count"] == 2
    assert "高优先问题清单" in captured_prompt["prompt"]
    assert "补足人物迟疑过程" in captured_prompt["prompt"]
    assert "把异响的即时反应落到动作里" in captured_prompt["prompt"]


async def test_should_auto_recover_stale_analysis_status_and_keep_none_compatible(
    chapters_client,
    chapters_session_factory,
    mock_user,
):
    project = await create_project(chapters_session_factory, user_id=mock_user.user_id)
    chapter_none = await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=1,
        title="无任务状态章节",
        content="正文",
        status="completed",
    )
    chapter_active = await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=2,
        title="活跃分析章节",
        content="正文",
        status="completed",
    )
    chapter_stale_running = await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=3,
        title="超时运行章节",
        content="正文",
        status="completed",
    )
    chapter_stale_pending = await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=4,
        title="超时待启动章节",
        content="正文",
        status="completed",
    )

    none_response = await chapters_client.get(f"/api/chapters/{chapter_none.id}/analysis/status")
    assert none_response.status_code == 200
    none_body = none_response.json()
    assert none_body["has_task"] is False
    assert none_body["status"] == "none"
    assert none_body["task_id"] is None

    active_running_time = datetime.now() - timedelta(minutes=4)
    stale_running_time = datetime.now() - timedelta(minutes=11)
    stale_pending_time = datetime.now() - timedelta(minutes=4)

    async with chapters_session_factory() as session:
        session.add(
            AnalysisTask(
                chapter_id=chapter_active.id,
                user_id=mock_user.user_id,
                project_id=project.id,
                status="running",
                progress=20,
                started_at=active_running_time,
                created_at=active_running_time,
            )
        )
        session.add(
            AnalysisTask(
                chapter_id=chapter_stale_running.id,
                user_id=mock_user.user_id,
                project_id=project.id,
                status="running",
                progress=56,
                started_at=stale_running_time,
                created_at=stale_running_time,
            )
        )
        session.add(
            AnalysisTask(
                chapter_id=chapter_stale_pending.id,
                user_id=mock_user.user_id,
                project_id=project.id,
                status="pending",
                progress=0,
                created_at=stale_pending_time,
            )
        )
        await session.commit()

    active_running = await chapters_client.get(f"/api/chapters/{chapter_active.id}/analysis/status")
    assert active_running.status_code == 200
    active_running_body = active_running.json()
    assert active_running_body["has_task"] is True
    assert active_running_body["status"] == "running"
    assert active_running_body["auto_recovered"] is False
    assert active_running_body["progress"] == 20

    recovered_running = await chapters_client.get(f"/api/chapters/{chapter_stale_running.id}/analysis/status")
    assert recovered_running.status_code == 200
    running_body = recovered_running.json()
    assert running_body["has_task"] is True
    assert running_body["status"] == "failed"
    assert running_body["auto_recovered"] is True
    assert running_body["error_code"] == "timeout"
    assert "自动恢复" in (running_body["error_message"] or "")

    recovered_pending = await chapters_client.get(f"/api/chapters/{chapter_stale_pending.id}/analysis/status")
    assert recovered_pending.status_code == 200
    pending_body = recovered_pending.json()
    assert pending_body["status"] == "failed"
    assert pending_body["auto_recovered"] is True
    assert pending_body["error_code"] == "timeout"
    assert "启动超时" in (pending_body["error_message"] or "")












async def test_should_restore_deferred_analysis_quality_snapshot_and_regeneration_compatibility(
    chapters_client,
    chapters_session_factory,
    mock_user,
):
    project = await create_project(chapters_session_factory, user_id=mock_user.user_id)
    chapter = await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=1,
        title="后台分析恢复章节",
        content="正文内容",
        status="completed",
    )

    async with chapters_session_factory() as session:
        task = BatchGenerationTask(
            project_id=project.id,
            user_id=mock_user.user_id,
            start_chapter_number=1,
            chapter_count=1,
            chapter_ids=[chapter.id],
            status="running",
            total_chapters=1,
            completed_chapters=0,
            current_chapter_id=chapter.id,
            current_chapter_number=1,
            current_retry_count=0,
            max_retries=3,
        )
        session.add(task)
        await session.commit()
        await session.refresh(task)
        task_id = task.id

    async with chapters_session_factory() as session:
        await chapters_api.publish_task_stream_event(
            task_id,
            {
                "type": "analysis_started",
                "chapter_id": chapter.id,
                "chapter_number": 1,
                "message": "analysis started",
                "progress": 85,
                "phase": "parsing",
                "candidate_index": 2,
                "candidate_count": 2,
                "word_count": 1320,
                "generation_path": "rerank_retry",
                "attempt_kind": "rerank_candidate",
                "rerank_used": True,
                "word_budget_repair_used": False,
                "winner_candidate_index": 2,
                "pre_compaction_total_length": 4210,
                "context_budget_limit": 2400,
                "compaction_applied": True,
                "compaction_details": {
                    "recent_chapters_context": {"before": 1850, "after": 860},
                    "foreshadow_reminders": {"before": 920, "after": 360},
                },
            },
            db_session=session,
        )
        await chapters_api._record_task_quality_metrics(
            task_id,
            {
                "chapter_id": chapter.id,
                "chapter_number": 1,
                "overall_score": 88.0,
                "conflict_chain_hit_rate": 80.0,
                "rule_grounding_hit_rate": 84.0,
                "outline_alignment_rate": 86.0,
                "dialogue_naturalness_rate": 78.0,
                "opening_hook_rate": 90.0,
                "payoff_chain_rate": 76.0,
                "cliffhanger_rate": 92.0,
                "pacing_score": 8.1,
            },
            db_session=session,
        )
        await chapters_api._set_task_active_story_repair_payload(
            task_id,
            {
                "summary": "Favor concrete scene pressure and emotional payoff.",
                "repair_targets": ["Raise external pressure", "Tighten chapter payoff"],
                "preserve_strengths": ["Keep character voice stable"],
                "focus_areas": ["pressure", "payoff"],
                "source": "manual_plus_recent_history_summary",
                "source_label": "Manual + recent quality trend",
                "scope": "batch",
                "updated_at": "2026-03-25T10:00:00",
            },
            db_session=session,
        )

    async def clear_runtime_caches() -> None:
        chapters_api.task_quality_metrics_cache.pop(task_id, None)
        async with chapters_api.task_workflow_lock:
            chapters_api.task_workflow_state_cache.pop(task_id, None)

    await clear_runtime_caches()

    status_response = await chapters_client.get(f"/api/chapters/batch-generate/{task_id}/status")
    assert status_response.status_code == 200
    status_body = status_response.json()
    assert status_body["stage_code"] == "6.writing.parsing"
    assert status_body["checkpoint"]["progress_phase"] == "parsing"
    assert status_body["checkpoint"]["last_event"] == "analysis_started"
    assert status_body["checkpoint"]["candidate_index"] == 2
    assert status_body["checkpoint"]["candidate_count"] == 2
    assert status_body["checkpoint"]["word_count"] == 1320
    assert status_body["checkpoint"]["generation_path"] == "rerank_retry"
    assert status_body["checkpoint"]["attempt_kind"] == "rerank_candidate"
    assert status_body["checkpoint"]["rerank_used"] is True
    assert status_body["checkpoint"]["word_budget_repair_used"] is False
    assert status_body["checkpoint"]["winner_candidate_index"] == 2
    assert status_body["checkpoint"]["pre_compaction_total_length"] == 4210
    assert status_body["checkpoint"]["context_budget_limit"] == 2400
    assert status_body["checkpoint"]["compaction_applied"] is True
    assert status_body["checkpoint"]["compaction_details"]["recent_chapters_context"]["after"] == 860
    assert status_body["latest_quality_metrics"]["overall_score"] == 88.0
    assert status_body["latest_quality_metrics"]["repair_guidance"]["summary"]
    assert status_body["latest_quality_metrics"]["quality_gate"]["status"] == "pass"
    assert status_body["quality_metrics_summary"]["chapter_count"] == 1
    assert status_body["quality_metrics_summary"]["avg_overall_score"] == 88.0
    assert status_body["quality_metrics_summary"]["avg_outline_alignment_rate"] == 86.0
    assert status_body["quality_metrics_summary"]["avg_dialogue_naturalness_rate"] == 78.0
    assert status_body["quality_metrics_summary"]["avg_pacing_score"] == 8.1
    assert status_body["quality_metrics_summary"]["repair_guidance"]["summary"]
    assert status_body["quality_metrics_summary"]["quality_gate"]["status"] == "pass"
    assert status_body["active_story_repair_payload"]["source"] == "manual_plus_recent_history_summary"
    assert status_body["active_story_repair_payload"]["source_label"] == "Manual + recent quality trend"

    await clear_runtime_caches()

    active_response = await chapters_client.get(
        f"/api/chapters/project/{project.id}/batch-generate/active"
    )
    assert active_response.status_code == 200
    active_body = active_response.json()
    assert active_body["has_active_task"] is True
    assert active_body["task"]["batch_id"] == task_id
    assert active_body["task"]["checkpoint"]["progress_phase"] == "parsing"
    assert active_body["task"]["checkpoint"]["last_event"] == "analysis_started"
    assert active_body["task"]["checkpoint"]["candidate_index"] == 2
    assert active_body["task"]["checkpoint"]["candidate_count"] == 2
    assert active_body["task"]["checkpoint"]["generation_path"] == "rerank_retry"
    assert active_body["task"]["checkpoint"]["attempt_kind"] == "rerank_candidate"
    assert active_body["task"]["checkpoint"]["pre_compaction_total_length"] == 4210
    assert active_body["task"]["checkpoint"]["context_budget_limit"] == 2400
    assert active_body["task"]["checkpoint"]["compaction_applied"] is True
    assert active_body["task"]["checkpoint"]["compaction_details"]["foreshadow_reminders"]["after"] == 360
    assert active_body["task"]["latest_quality_metrics"]["overall_score"] == 88.0
    assert active_body["task"]["latest_quality_metrics"]["repair_guidance"]["focus_areas"]
    assert active_body["task"]["latest_quality_metrics"]["quality_gate"]["status"] == "pass"
    assert active_body["task"]["quality_metrics_summary"]["avg_cliffhanger_rate"] == 92.0
    assert active_body["task"]["quality_metrics_summary"]["avg_pacing_score"] == 8.1
    assert active_body["task"]["quality_metrics_summary"]["repair_guidance"]["focus_areas"]
    assert active_body["task"]["quality_metrics_summary"]["quality_gate"]["status"] == "pass"
    assert active_body["task"]["active_story_repair_payload"]["source"] == "manual_plus_recent_history_summary"

    async with chapters_session_factory() as session:
        snapshot_result = await session.execute(
            select(BatchGenerationSnapshot).where(BatchGenerationSnapshot.batch_task_id == task_id)
        )
        snapshot = snapshot_result.scalar_one_or_none()
        assert snapshot is not None
        assert snapshot.workflow_runtime_state is not None
        assert snapshot.workflow_runtime_state["phase"] == "parsing"
        assert snapshot.workflow_runtime_state["last_event"] == "analysis_started"
        assert snapshot.workflow_runtime_state["candidate_index"] == 2
        assert snapshot.workflow_runtime_state["candidate_count"] == 2
        assert snapshot.workflow_runtime_state["word_count"] == 1320
        assert snapshot.workflow_runtime_state["generation_path"] == "rerank_retry"
        assert snapshot.workflow_runtime_state["attempt_kind"] == "rerank_candidate"
        assert snapshot.workflow_runtime_state["rerank_used"] is True
        assert snapshot.workflow_runtime_state["word_budget_repair_used"] is False
        assert snapshot.workflow_runtime_state["winner_candidate_index"] == 2
        assert snapshot.workflow_runtime_state["pre_compaction_total_length"] == 4210
        assert snapshot.workflow_runtime_state["context_budget_limit"] == 2400
        assert snapshot.workflow_runtime_state["compaction_applied"] is True
        assert snapshot.workflow_runtime_state["compaction_details"]["recent_chapters_context"]["before"] == 1850
        assert snapshot.workflow_runtime_state["active_story_repair_payload"]["source"] == "manual_plus_recent_history_summary"

    await clear_runtime_caches()

    active_tasks_response = await chapters_client.get("/api/chapters/batch-generate/active-tasks?limit=10")
    assert active_tasks_response.status_code == 200
    active_tasks_body = active_tasks_response.json()
    active_task_item = next(item for item in active_tasks_body["items"] if item["batch_id"] == task_id)
    assert active_task_item["checkpoint"]["progress_phase"] == "parsing"
    assert active_task_item["checkpoint"]["last_event"] == "analysis_started"
    assert active_task_item["checkpoint"]["candidate_index"] == 2
    assert active_task_item["checkpoint"]["candidate_count"] == 2
    assert active_task_item["checkpoint"]["generation_path"] == "rerank_retry"
    assert active_task_item["checkpoint"]["attempt_kind"] == "rerank_candidate"
    assert active_task_item["checkpoint"]["pre_compaction_total_length"] == 4210
    assert active_task_item["checkpoint"]["context_budget_limit"] == 2400
    assert active_task_item["checkpoint"]["compaction_applied"] is True
    assert active_task_item["active_story_repair_payload"]["source"] == "manual_plus_recent_history_summary"

    can_generate_response = await chapters_client.get(f"/api/chapters/{chapter.id}/can-generate")
    assert can_generate_response.status_code == 200
    assert can_generate_response.json()["can_generate"] is True









def test_should_include_story_runtime_contract_in_generation_history_payload():
    story_runtime_contract = {
        "guidance": {
            "plot_stage": "development",
            "story_focus": "alliance under pressure",
            "quality_preset": "cinematic",
        },
        "blueprint": {
            "chapter_count": 12,
            "current_chapter_number": 5,
            "target_word_count": 2400,
            "character_focus_names": ["Lin", "Su"],
            "foreshadow_payoff_plan": ["recover the hidden key"],
            "organization_state_ledger": ["ShadowGuild: control tightened around the docks"],
            "career_state_ledger": ["Lin/Strategist: stage 3 with supply-chain pressure"],
        },
    }

    payload = json.loads(
        chapters_api._build_generation_history_payload(
            "generated body",
            {"overall_score": 88.0},
            story_runtime_contract=story_runtime_contract,
        )
    )

    assert payload["quality_metrics"]["story_runtime_contract"] == story_runtime_contract
    assert payload["story_runtime_contract"] == story_runtime_contract
    assert payload["story_runtime_snapshot"]["plot_stage"] == "development"
    assert payload["story_runtime_snapshot"]["current_chapter_number"] == 5
    assert payload["story_runtime_snapshot"]["character_focus"] == ["Lin", "Su"]
