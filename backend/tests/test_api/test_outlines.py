from app.api.outlines import _dump_model_like_payload
from app.schemas.outline import ChapterPlanItem
import json
from datetime import datetime, timedelta
from typing import Any

import pytest
from app.models.generation_history import GenerationHistory


def test_should_dump_chapter_plan_item_payload_from_pydantic_model():
    item = ChapterPlanItem(
        sub_index=1,
        title="Chapter One",
        plot_summary="The protagonist enters the center of the storm.",
        key_events=["enter ruins", "trigger anomaly"],
        character_focus=["Lin Chuan"],
        emotional_tone="tense",
        narrative_goal="establish conflict",
        conflict_type="external",
        estimated_words=3000,
        scenes=["ruin gate", "underground hall"],
    )

    payload = _dump_model_like_payload(item)

    assert payload["title"] == "Chapter One"
    assert payload["key_events"] == ["enter ruins", "trigger anomaly"]


def test_should_dump_chapter_plan_item_payload_from_mapping():
    payload = _dump_model_like_payload({
        "sub_index": 2,
        "title": "Chapter Two",
        "plot_summary": "The crisis keeps spreading.",
        "key_events": ["discover clue"],
        "character_focus": ["Lin Chuan", "Su Jin"],
        "emotional_tone": "oppressive",
        "narrative_goal": "advance suspense",
        "conflict_type": "mixed",
        "estimated_words": 3200,
        "scenes": ["old district"],
    })

    assert payload["sub_index"] == 2
    assert payload["character_focus"] == ["Lin Chuan", "Su Jin"]

from fastapi import FastAPI, Request
from httpx import ASGITransport, AsyncClient
from sqlalchemy import select
from sqlalchemy.ext.asyncio import AsyncSession, async_sessionmaker

from app.api import outlines as outlines_api
from types import SimpleNamespace
from app.database import Base
from app.models.chapter import Chapter
from app.models.outline import Outline
from app.models.project import Project
from app.models.character import Character
from app.models.career import Career, CharacterCareer
from app.models.relationship import CharacterRelationship, Organization, OrganizationMember


@pytest.fixture(autouse=True)
def reset_outlines_runtime_caches():
    if hasattr(outlines_api, "outline_quality_summary_cache"):
        outlines_api.outline_quality_summary_cache.clear()
    yield
    if hasattr(outlines_api, "outline_quality_summary_cache"):
        outlines_api.outline_quality_summary_cache.clear()



def _build_outline_quality_history_payload(metrics: dict[str, Any]) -> str:
    return json.dumps(
        {
            "log_type": "chapter_generation_quality_v1",
            "quality_metrics": metrics,
        },
        ensure_ascii=False,
    )


async def test_should_reuse_outline_quality_summary_cached_snapshot(test_engine, monkeypatch):
    async with test_engine.begin() as conn:
        await conn.run_sync(Base.metadata.create_all)

    session_maker = async_sessionmaker(
        test_engine,
        class_=AsyncSession,
        expire_on_commit=False,
    )

    async with session_maker() as session:
        project = Project(
            user_id="outline-cache-user",
            title="Outline Cache Project",
            description="cache summary test",
            outline_mode="one-to-one",
        )
        session.add(project)
        await session.flush()

        chapter_one = Chapter(project_id=project.id, chapter_number=1, title="?1?", content="A")
        chapter_two = Chapter(project_id=project.id, chapter_number=2, title="?2?", content="B")
        chapter_three = Chapter(project_id=project.id, chapter_number=3, title="?3?", content="C")
        session.add_all([chapter_one, chapter_two, chapter_three])
        await session.flush()

        now = datetime.utcnow()
        session.add_all(
            [
                GenerationHistory(
                    project_id=project.id,
                    chapter_id=chapter_one.id,
                    prompt="chapter one quality",
                    generated_content=_build_outline_quality_history_payload(
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
                    created_at=now - timedelta(minutes=3),
                ),
                GenerationHistory(
                    project_id=project.id,
                    chapter_id=chapter_two.id,
                    prompt="chapter two quality",
                    generated_content=_build_outline_quality_history_payload(
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
                    created_at=now - timedelta(minutes=2),
                ),
                GenerationHistory(
                    project_id=project.id,
                    chapter_id=chapter_three.id,
                    prompt="chapter three quality",
                    generated_content=_build_outline_quality_history_payload(
                        {
                            "overall_score": 84.0,
                            "conflict_chain_hit_rate": 71.0,
                            "rule_grounding_hit_rate": 86.0,
                            "outline_alignment_rate": 74.0,
                            "dialogue_naturalness_rate": 82.0,
                            "opening_hook_rate": 77.0,
                            "payoff_chain_rate": 65.0,
                            "cliffhanger_rate": 88.0,
                            "pacing_score": 7.4,
                        }
                    ),
                    model="default",
                    created_at=now - timedelta(minutes=1),
                ),
            ]
        )
        await session.commit()
        project_id = project.id

    original_extract_metrics = outlines_api.extract_quality_metrics_from_history_payload
    calls = {"extract": 0}

    def counting_extract_metrics(*args, **kwargs):
        calls["extract"] += 1
        return original_extract_metrics(*args, **kwargs)

    monkeypatch.setattr(outlines_api, "extract_quality_metrics_from_history_payload", counting_extract_metrics)

    async with session_maker() as session:
        first_summary = await outlines_api._load_outline_quality_summary(session, project_id, chapter_limit=3)
        second_summary = await outlines_api._load_outline_quality_summary(session, project_id, chapter_limit=3)

    assert first_summary == second_summary
    assert first_summary.get("chapter_count") == 3
    assert calls["extract"] == 3


async def test_should_restore_outline_quality_summary_from_persisted_snapshot_after_cache_clear(test_engine, monkeypatch):
    async with test_engine.begin() as conn:
        await conn.run_sync(Base.metadata.create_all)

    session_maker = async_sessionmaker(
        test_engine,
        class_=AsyncSession,
        expire_on_commit=False,
    )

    async with session_maker() as session:
        project = Project(
            user_id="outline-cache-user",
            title="Outline Persisted Cache Project",
            description="persist summary test",
            outline_mode="one-to-one",
        )
        session.add(project)
        await session.flush()

        chapter_one = Chapter(project_id=project.id, chapter_number=1, title="?1?", content="A")
        chapter_two = Chapter(project_id=project.id, chapter_number=2, title="?2?", content="B")
        session.add_all([chapter_one, chapter_two])
        await session.flush()

        now = datetime.utcnow()
        session.add_all(
            [
                GenerationHistory(
                    project_id=project.id,
                    chapter_id=chapter_one.id,
                    prompt="chapter one quality",
                    generated_content=_build_outline_quality_history_payload(
                        {
                            "overall_score": 79.0,
                            "conflict_chain_hit_rate": 63.0,
                            "rule_grounding_hit_rate": 81.0,
                            "outline_alignment_rate": 66.0,
                            "dialogue_naturalness_rate": 78.0,
                            "opening_hook_rate": 73.0,
                            "payoff_chain_rate": 59.0,
                            "cliffhanger_rate": 83.0,
                            "pacing_score": 6.8,
                        }
                    ),
                    model="default",
                    created_at=now - timedelta(minutes=2),
                ),
                GenerationHistory(
                    project_id=project.id,
                    chapter_id=chapter_two.id,
                    prompt="chapter two quality",
                    generated_content=_build_outline_quality_history_payload(
                        {
                            "overall_score": 82.0,
                            "conflict_chain_hit_rate": 69.0,
                            "rule_grounding_hit_rate": 84.0,
                            "outline_alignment_rate": 70.0,
                            "dialogue_naturalness_rate": 81.0,
                            "opening_hook_rate": 76.0,
                            "payoff_chain_rate": 63.0,
                            "cliffhanger_rate": 86.0,
                            "pacing_score": 7.2,
                        }
                    ),
                    model="default",
                    created_at=now - timedelta(minutes=1),
                ),
            ]
        )
        await session.commit()
        project_id = project.id

    persisted_snapshots: dict[tuple[str, int], dict[str, Any]] = {}
    original_extract_metrics = outlines_api.extract_quality_metrics_from_history_payload
    calls = {"extract": 0}

    def counting_extract_metrics(*args, **kwargs):
        calls["extract"] += 1
        return original_extract_metrics(*args, **kwargs)

    def fake_persist_snapshot(project_id: str, limit: int, snapshot: dict[str, Any]) -> None:
        persisted_snapshots[(project_id, limit)] = json.loads(json.dumps(snapshot, ensure_ascii=False))

    def fake_load_snapshot(project_id: str, limit: int) -> dict[str, Any] | None:
        snapshot = persisted_snapshots.get((project_id, limit))
        if snapshot is None:
            return None
        return json.loads(json.dumps(snapshot, ensure_ascii=False))

    monkeypatch.setattr(outlines_api, "extract_quality_metrics_from_history_payload", counting_extract_metrics)
    monkeypatch.setattr(outlines_api, "persist_outline_quality_summary_snapshot", fake_persist_snapshot)
    monkeypatch.setattr(outlines_api, "load_outline_quality_summary_snapshot", fake_load_snapshot)

    async with session_maker() as session:
        first_summary = await outlines_api._load_outline_quality_summary(session, project_id, chapter_limit=2)

    assert first_summary.get("chapter_count") == 2
    assert calls["extract"] == 2
    assert (project_id, 2) in persisted_snapshots

    outlines_api.outline_quality_summary_cache.clear()

    async with session_maker() as session:
        second_summary = await outlines_api._load_outline_quality_summary(session, project_id, chapter_limit=2)

    assert second_summary == first_summary
    assert calls["extract"] == 2


async def test_should_create_chapters_from_dict_plan_payload_via_api(test_engine, mock_user):
    async with test_engine.begin() as conn:
        await conn.run_sync(Base.metadata.create_all)

    session_maker = async_sessionmaker(
        test_engine,
        class_=AsyncSession,
        expire_on_commit=False,
    )

    app = FastAPI()
    app.include_router(outlines_api.router, prefix="/api")

    async def override_get_db():
        async with session_maker() as session:
            yield session

    def override_get_user_ai_service():
        return object()

    app.dependency_overrides[outlines_api.get_db] = override_get_db
    app.dependency_overrides[outlines_api.get_user_ai_service] = override_get_user_ai_service

    @app.middleware("http")
    async def inject_user_id(request: Request, call_next):
        request.state.user_id = mock_user.user_id
        return await call_next(request)

    async with session_maker() as seed_session:
        project = Project(
            user_id=mock_user.user_id,
            title="API Outline Project",
            description="seed project",
            outline_mode="one-to-many",
        )
        seed_session.add(project)
        await seed_session.flush()

        outline = Outline(
            project_id=project.id,
            title="Outline A",
            content="Outline content",
            order_index=1,
        )
        seed_session.add(outline)
        await seed_session.commit()
        await seed_session.refresh(project)
        await seed_session.refresh(outline)
        outline_id = outline.id
        project_id = project.id

    transport = ASGITransport(app=app)
    async with AsyncClient(transport=transport, base_url="http://testserver") as client:
        response = await client.post(
            f"/api/outlines/{outline_id}/create-chapters-from-plans",
            json={
                "chapter_plans": [
                    {
                        "sub_index": 1,
                        "title": "Chapter One",
                        "plot_summary": "The protagonist enters the storm.",
                        "key_events": ["enter storm"],
                        "character_focus": ["Lin Chuan"],
                        "emotional_tone": "tense",
                        "narrative_goal": "start conflict",
                        "conflict_type": "external",
                        "estimated_words": 2800,
                        "scenes": ["city gate"],
                    }
                ]
            },
        )

    assert response.status_code == 200
    body = response.json()
    assert body["outline_id"] == outline_id
    assert body["chapters_created"] == 1
    assert body["created_chapters"][0]["title"] == "Chapter One"

    async with session_maker() as verify_session:
        result = await verify_session.execute(
            select(Chapter).where(
                Chapter.project_id == project_id,
                Chapter.outline_id == outline_id,
            )
        )
        chapters = result.scalars().all()

    assert len(chapters) == 1
    assert chapters[0].title == "Chapter One"



async def test_should_create_outline_stream_without_duplicate_story_packet_chapter_count(
    test_engine,
    mock_user,
    monkeypatch,
):
    from fastapi import FastAPI, Request
    from httpx import ASGITransport, AsyncClient
    from sqlalchemy.ext.asyncio import AsyncSession, async_sessionmaker

    from app.api import outlines as outlines_api
    from app.database import Base
    from app.models.outline import Outline
    from app.models.project import Project
    from app.services.chapter_quality_context_service import (
        StoryBlueprint,
        StoryGenerationGuidance,
        StoryPacket,
    )

    async with test_engine.begin() as conn:
        await conn.run_sync(Base.metadata.create_all)

    session_maker = async_sessionmaker(
        test_engine,
        class_=AsyncSession,
        expire_on_commit=False,
    )

    app = FastAPI()
    app.include_router(outlines_api.router, prefix="/api")

    async def override_get_db():
        async with session_maker() as session:
            yield session

    class FakeAIService:
        def __init__(self):
            self.calls = []
            self.user_id = None
            self.db_session = None

        async def generate_text_stream(self, **kwargs):
            self.calls.append(kwargs)
            yield json.dumps([
                {
                    "title": "Opening Chapter",
                    "summary": "The protagonist enters the harbor under pressure.",
                    "content": "The protagonist enters the harbor under pressure.",
                }
            ], ensure_ascii=False)

    fake_ai_service = FakeAIService()

    def override_get_user_ai_service():
        return fake_ai_service

    app.dependency_overrides[outlines_api.get_db] = override_get_db
    app.dependency_overrides[outlines_api.get_user_ai_service] = override_get_user_ai_service

    @app.middleware("http")
    async def inject_user_id(request: Request, call_next):
        request.state.user_id = mock_user.user_id
        return await call_next(request)

    async def fake_get_template(*args, **kwargs):
        return "template"

    def fake_format_prompt(template, **kwargs):
        return f"formatted::{kwargs['chapter_count']}"

    async def fake_build_story_packet(*args, **kwargs):
        return StoryPacket.from_guidance(
            StoryGenerationGuidance(
                creative_mode="hook",
                story_focus="advance_plot",
                plot_stage="development",
                story_creation_brief="keep pressure visible",
                quality_preset="plot_drive",
                quality_notes="maintain strong opening momentum",
            ),
            blueprint=StoryBlueprint(chapter_count=42),
        )

    async def fake_check_characters(**kwargs):
        return {"created_count": 0, "created_characters": []}

    async def fake_check_organizations(**kwargs):
        return {"created_count": 0, "created_organizations": []}

    monkeypatch.setattr(outlines_api.PromptService, "get_template", fake_get_template)
    monkeypatch.setattr(outlines_api.PromptService, "format_prompt", fake_format_prompt)
    monkeypatch.setattr(
        outlines_api,
        "build_story_generation_packet_with_project_continuity",
        fake_build_story_packet,
    )
    monkeypatch.setattr(outlines_api, "_check_and_create_missing_characters_from_outlines", fake_check_characters)
    monkeypatch.setattr(outlines_api, "_check_and_create_missing_organizations_from_outlines", fake_check_organizations)

    async with session_maker() as seed_session:
        project = Project(
            user_id=mock_user.user_id,
            title="Fresh Outline Project",
            description="seed project",
            theme="Harbor pressure",
            genre="Urban fantasy",
            narrative_perspective="third_person",
            outline_mode="one-to-many",
        )
        seed_session.add(project)
        await seed_session.commit()
        await seed_session.refresh(project)
        project_id = project.id

    transport = ASGITransport(app=app)
    async with AsyncClient(transport=transport, base_url="http://testserver") as client:
        response = await client.post(
            "/api/outlines/generate-stream",
            json={
                "project_id": project_id,
                "theme": "Harbor pressure",
                "chapter_count": 1,
                "narrative_perspective": "third_person",
                "target_words": 6000,
                "provider": "sub2api",
                "model": "gpt-5.4",
                "enable_mcp": False,
            },
        )

    assert response.status_code == 200
    assert len(fake_ai_service.calls) == 1

    async with session_maker() as verify_session:
        result = await verify_session.execute(
            select(Outline)
            .where(Outline.project_id == project_id)
            .order_by(Outline.order_index)
        )
        outlines = result.scalars().all()

    assert len(outlines) == 1
    assert outlines[0].order_index == 1
async def test_should_continue_generate_stream_without_duplicate_chapter_count_and_honor_auto_mcp(
    test_engine,
    mock_user,
    monkeypatch,
):
    import json

    from fastapi import FastAPI, Request
    from httpx import ASGITransport, AsyncClient
    from sqlalchemy import select
    from sqlalchemy.ext.asyncio import AsyncSession, async_sessionmaker

    from app.api import outlines as outlines_api
    from app.database import Base
    from app.models.outline import Outline
    from app.models.project import Project

    async with test_engine.begin() as conn:
        await conn.run_sync(Base.metadata.create_all)

    session_maker = async_sessionmaker(
        test_engine,
        class_=AsyncSession,
        expire_on_commit=False,
    )

    app = FastAPI()
    app.include_router(outlines_api.router, prefix="/api")

    async def override_get_db():
        async with session_maker() as session:
            yield session

    class FakeAIService:
        def __init__(self):
            self.calls = []
            self.user_id = None
            self.db_session = None

        async def generate_text_stream(self, **kwargs):
            self.calls.append(kwargs)
            yield json.dumps([
                {
                    "title": "第2章",
                    "summary": "续写摘要",
                    "content": "续写摘要",
                }
            ], ensure_ascii=False)

    fake_ai_service = FakeAIService()

    def override_get_user_ai_service():
        return fake_ai_service

    app.dependency_overrides[outlines_api.get_db] = override_get_db
    app.dependency_overrides[outlines_api.get_user_ai_service] = override_get_user_ai_service

    @app.middleware("http")
    async def inject_user_id(request: Request, call_next):
        request.state.user_id = mock_user.user_id
        return await call_next(request)

    from app.services.chapter_quality_context_service import (
        StoryBlueprint,
        StoryGenerationGuidance,
        StoryPacket,
    )

    async def fake_get_template(*args, **kwargs):
        return "template"

    def fake_format_prompt(template, **kwargs):
        return f"formatted::{template}"

    async def fake_build_story_packet(*args, **kwargs):
        return StoryPacket.from_guidance(
            StoryGenerationGuidance(
                creative_mode="balanced",
                story_focus="advance_plot",
                plot_stage="development",
                story_creation_brief=None,
                quality_preset="plot_drive",
                quality_notes="续写阶段保持节奏推进。",
            ),
            blueprint=StoryBlueprint(chapter_count=99),
        )

    async def fake_continue_context(*args, **kwargs):
        return {
            "recent_outlines": "第1章：已有内容",
            "characters_info": "",
            "memory_guidance": "",
            "quality_repair_guidance": "",
            "quality_trend_guidance": "",
            "stats": {
                "total_outlines": 1,
                "recent_outlines_count": 1,
                "characters_count": 0,
                "total_length": 12,
            },
        }

    monkeypatch.setattr(outlines_api.PromptService, "get_template", fake_get_template)
    monkeypatch.setattr(outlines_api.PromptService, "format_prompt", fake_format_prompt)
    monkeypatch.setattr(
        outlines_api,
        "build_story_generation_packet_with_project_continuity",
        fake_build_story_packet,
    )
    monkeypatch.setattr(outlines_api, "_build_outline_continue_context", fake_continue_context)

    async with session_maker() as seed_session:
        project = Project(
            user_id=mock_user.user_id,
            title="Continuation Project",
            description="seed project",
            theme="命运对抗",
            genre="都市异能",
            narrative_perspective="third_person",
            outline_mode="one-to-many",
        )
        seed_session.add(project)
        await seed_session.flush()

        seed_outline = Outline(
            project_id=project.id,
            title="第1章",
            content="已有大纲",
            order_index=1,
        )
        seed_session.add(seed_outline)
        await seed_session.commit()
        await seed_session.refresh(project)
        project_id = project.id

    transport = ASGITransport(app=app)
    async with AsyncClient(transport=transport, base_url="http://testserver") as client:
        response = await client.post(
            "/api/outlines/generate-stream",
            json={
                "project_id": project_id,
                "theme": "命运对抗",
                "chapter_count": 1,
                "narrative_perspective": "third_person",
                "target_words": 6000,
                "mode": "continue",
                "provider": "sub2api",
                "model": "gpt-5.4",
                "enable_mcp": False,
            },
        )

    assert response.status_code == 200
    body = response.text
    assert '"type": "error"' not in body
    assert fake_ai_service.calls
    assert fake_ai_service.calls[0]["auto_mcp"] is False
    assert fake_ai_service.calls[0]["request_options"]["prefer_chat_completions"] is True

    async with session_maker() as verify_session:
        result = await verify_session.execute(
            select(Outline)
            .where(Outline.project_id == project_id)
            .order_by(Outline.order_index)
        )
        outlines = result.scalars().all()

    assert len(outlines) == 2
    assert outlines[-1].order_index == 2
    assert outlines[-1].title == "第2章"


async def test_should_build_story_packet_once_across_multiple_continue_batches(
    test_engine,
    mock_user,
    monkeypatch,
):
    from fastapi import FastAPI, Request
    from httpx import ASGITransport, AsyncClient
    from sqlalchemy.ext.asyncio import AsyncSession, async_sessionmaker

    from app.api import outlines as outlines_api
    from app.database import Base
    from app.models.outline import Outline
    from app.models.project import Project
    from app.services.chapter_quality_context_service import (
        StoryBlueprint,
        StoryGenerationGuidance,
        StoryPacket,
    )

    async with test_engine.begin() as conn:
        await conn.run_sync(Base.metadata.create_all)

    session_maker = async_sessionmaker(
        test_engine,
        class_=AsyncSession,
        expire_on_commit=False,
    )

    app = FastAPI()
    app.include_router(outlines_api.router, prefix="/api")

    async def override_get_db():
        async with session_maker() as session:
            yield session

    class FakeAIService:
        def __init__(self):
            self.calls = []
            self.user_id = None
            self.db_session = None

        async def generate_text_stream(self, **kwargs):
            call_index = len(self.calls) + 1
            self.calls.append(kwargs)
            start_chapter = 2 if call_index == 1 else 7
            batch_size = 5 if call_index == 1 else 1
            payload = [
                {
                    "title": f"Chapter {start_chapter + offset}",
                    "summary": f"Outline summary {start_chapter + offset}",
                    "content": f"Outline summary {start_chapter + offset}",
                }
                for offset in range(batch_size)
            ]
            yield json.dumps(payload, ensure_ascii=False)

    fake_ai_service = FakeAIService()

    def override_get_user_ai_service():
        return fake_ai_service

    app.dependency_overrides[outlines_api.get_db] = override_get_db
    app.dependency_overrides[outlines_api.get_user_ai_service] = override_get_user_ai_service

    @app.middleware("http")
    async def inject_user_id(request: Request, call_next):
        request.state.user_id = mock_user.user_id
        return await call_next(request)

    async def fake_get_template(*args, **kwargs):
        return "template"

    def fake_format_prompt(template, **kwargs):
        return f"formatted::{kwargs['chapter_count']}::{kwargs['current_chapter_count']}"

    captured = {"story_packet_calls": 0, "scheduled": 0, "static_context_calls": 0, "static_context_ids": []}

    async def fake_build_story_packet(*args, **kwargs):
        captured["story_packet_calls"] += 1
        return StoryPacket.from_guidance(
            StoryGenerationGuidance(
                creative_mode="balanced",
                story_focus="advance_plot",
                plot_stage="development",
                story_creation_brief=None,
                quality_preset="plot_drive",
                quality_notes="keep momentum steady",
            ),
            blueprint=StoryBlueprint(chapter_count=99),
        )

    async def fake_build_static_context(*args, **kwargs):
        captured["static_context_calls"] += 1
        return {
            "project_info": "project",
            "characters_info": "prefetched characters",
            "quality_repair_guidance": "prefetched repair",
            "quality_trend_guidance": "prefetched trend",
            "stats": {
                "characters_count": 0,
                "characters_info_length": 20,
                "detailed_characters_count": 0,
                "compacted_characters_count": 0,
                "quality_repair_guidance_length": 17,
                "quality_trend_guidance_length": 16,
            },
        }

    async def fake_continue_context(*args, **kwargs):
        captured["static_context_ids"].append(id(kwargs.get("prebuilt_static_context")))
        return {
            "recent_outlines": "chapter 1 existing outline",
            "characters_info": "",
            "memory_guidance": "",
            "quality_repair_guidance": "",
            "quality_trend_guidance": "",
            "stats": {
                "total_outlines": 1,
                "recent_outlines_count": 1,
                "characters_count": 0,
                "total_length": 12,
            },
        }

    async def fake_check_characters(**kwargs):
        return {"created_count": 0, "created_characters": []}

    async def fake_check_organizations(**kwargs):
        return {"created_count": 0, "created_organizations": []}

    def fake_schedule_postprocess(**kwargs):
        captured["scheduled"] += 1

    monkeypatch.setattr(outlines_api.PromptService, "get_template", fake_get_template)
    monkeypatch.setattr(outlines_api.PromptService, "format_prompt", fake_format_prompt)
    monkeypatch.setattr(
        outlines_api,
        "build_story_generation_packet_with_project_continuity",
        fake_build_story_packet,
    )
    monkeypatch.setattr(outlines_api, "_build_outline_continue_static_context", fake_build_static_context)
    monkeypatch.setattr(outlines_api, "_build_outline_continue_context", fake_continue_context)
    monkeypatch.setattr(outlines_api, "_check_and_create_missing_characters_from_outlines", fake_check_characters)
    monkeypatch.setattr(outlines_api, "_check_and_create_missing_organizations_from_outlines", fake_check_organizations)
    monkeypatch.setattr(outlines_api, "_schedule_outline_postprocess_background", fake_schedule_postprocess)

    async with session_maker() as seed_session:
        project = Project(
            user_id=mock_user.user_id,
            title="Continuation Project",
            description="seed project",
            theme="Fate pressure",
            genre="Urban fantasy",
            narrative_perspective="third_person",
            outline_mode="one-to-many",
        )
        seed_session.add(project)
        await seed_session.flush()

        seed_outline = Outline(
            project_id=project.id,
            title="Chapter 1",
            content="Existing outline",
            order_index=1,
        )
        seed_session.add(seed_outline)
        await seed_session.commit()
        await seed_session.refresh(project)
        project_id = project.id

    transport = ASGITransport(app=app)
    async with AsyncClient(transport=transport, base_url="http://testserver") as client:
        response = await client.post(
            "/api/outlines/generate-stream",
            json={
                "project_id": project_id,
                "theme": "Fate pressure",
                "chapter_count": 6,
                "narrative_perspective": "third_person",
                "target_words": 12000,
                "mode": "continue",
                "provider": "sub2api",
                "model": "gpt-5.4",
                "plot_stage": "development",
                "story_direction": "keep advancing the mainline",
                "requirements": "",
                "enable_mcp": False,
            },
        )

    assert response.status_code == 200
    assert captured["story_packet_calls"] == 1
    assert captured["static_context_calls"] == 1
    assert len(set(captured["static_context_ids"])) == 1
    assert len(fake_ai_service.calls) == 2
    assert captured["scheduled"] == 1

    async with session_maker() as verify_session:
        result = await verify_session.execute(
            select(Outline)
            .where(Outline.project_id == project_id)
            .order_by(Outline.order_index)
        )
        outlines = result.scalars().all()

    assert len(outlines) == 7
    assert outlines[-1].order_index == 7
async def test_should_defer_final_continue_outline_postprocess_to_background(test_engine, mock_user, monkeypatch):
    import json

    from fastapi import FastAPI, Request
    from httpx import ASGITransport, AsyncClient
    from sqlalchemy.ext.asyncio import AsyncSession, async_sessionmaker

    from app.api import outlines as outlines_api
    from app.database import Base
    from app.models.outline import Outline
    from app.models.project import Project
    from app.services.chapter_quality_context_service import (
        StoryBlueprint,
        StoryGenerationGuidance,
        StoryPacket,
    )

    async with test_engine.begin() as conn:
        await conn.run_sync(Base.metadata.create_all)

    session_maker = async_sessionmaker(
        test_engine,
        class_=AsyncSession,
        expire_on_commit=False,
    )

    app = FastAPI()
    app.include_router(outlines_api.router, prefix="/api")

    async def override_get_db():
        async with session_maker() as session:
            yield session

    class FakeAIService:
        def __init__(self):
            self.calls = []
            self.user_id = None
            self.db_session = None

        async def generate_text_stream(self, **kwargs):
            self.calls.append(kwargs)
            yield json.dumps([
                {
                    "title": "?2?",
                    "summary": "????",
                    "content": "????",
                }
            ], ensure_ascii=False)

    fake_ai_service = FakeAIService()

    def override_get_user_ai_service():
        return fake_ai_service

    app.dependency_overrides[outlines_api.get_db] = override_get_db
    app.dependency_overrides[outlines_api.get_user_ai_service] = override_get_user_ai_service

    @app.middleware("http")
    async def inject_user_id(request: Request, call_next):
        request.state.user_id = mock_user.user_id
        return await call_next(request)

    async def fake_get_template(*args, **kwargs):
        return "template"

    def fake_format_prompt(template, **kwargs):
        return f"formatted::{template}"

    async def fake_build_story_packet(*args, **kwargs):
        return StoryPacket.from_guidance(
            StoryGenerationGuidance(
                creative_mode="balanced",
                story_focus="advance_plot",
                plot_stage="development",
                story_creation_brief=None,
                quality_preset="plot_drive",
                quality_notes="???????????",
            ),
            blueprint=StoryBlueprint(chapter_count=99),
        )

    async def fake_continue_context(*args, **kwargs):
        return {
            "recent_outlines": "?1??????",
            "characters_info": "",
            "memory_guidance": "",
            "quality_repair_guidance": "",
            "quality_trend_guidance": "",
            "stats": {
                "total_outlines": 1,
                "recent_outlines_count": 1,
                "characters_count": 0,
                "total_length": 12,
            },
        }

    captured = {
        "char_calls": 0,
        "org_calls": 0,
        "scheduled_kwargs": None,
    }

    async def fake_check_characters(**kwargs):
        captured["char_calls"] += 1
        return {"created_count": 0, "created_characters": []}

    async def fake_check_organizations(**kwargs):
        captured["org_calls"] += 1
        return {"created_count": 0, "created_organizations": []}

    def fake_schedule_outline_postprocess_background(**kwargs):
        captured["scheduled_kwargs"] = kwargs

    monkeypatch.setattr(outlines_api.PromptService, "get_template", fake_get_template)
    monkeypatch.setattr(outlines_api.PromptService, "format_prompt", fake_format_prompt)
    monkeypatch.setattr(
        outlines_api,
        "build_story_generation_packet_with_project_continuity",
        fake_build_story_packet,
    )
    monkeypatch.setattr(outlines_api, "_build_outline_continue_context", fake_continue_context)
    monkeypatch.setattr(outlines_api, "_check_and_create_missing_characters_from_outlines", fake_check_characters)
    monkeypatch.setattr(outlines_api, "_check_and_create_missing_organizations_from_outlines", fake_check_organizations)
    monkeypatch.setattr(outlines_api, "_schedule_outline_postprocess_background", fake_schedule_outline_postprocess_background)

    async with session_maker() as seed_session:
        project = Project(
            user_id=mock_user.user_id,
            title="Deferred Continuation Project",
            description="seed project",
            theme="????",
            genre="????",
            narrative_perspective="third_person",
            outline_mode="one-to-many",
        )
        seed_session.add(project)
        await seed_session.flush()

        seed_outline = Outline(
            project_id=project.id,
            title="?1?",
            content="????",
            order_index=1,
        )
        seed_session.add(seed_outline)
        await seed_session.commit()
        await seed_session.refresh(project)
        project_id = project.id

    transport = ASGITransport(app=app)
    async with AsyncClient(transport=transport, base_url="http://testserver") as client:
        response = await client.post(
            "/api/outlines/generate-stream",
            json={
                "project_id": project_id,
                "theme": "????",
                "chapter_count": 1,
                "narrative_perspective": "third_person",
                "target_words": 6000,
                "mode": "continue",
                "provider": "sub2api",
                "model": "gpt-5.4",
                "enable_mcp": False,
            },
        )

    assert response.status_code == 200
    assert '"type": "error"' not in response.text
    assert captured["char_calls"] == 0
    assert captured["org_calls"] == 0
    assert captured["scheduled_kwargs"] is not None
    assert captured["scheduled_kwargs"]["project_id"] == project_id
    assert captured["scheduled_kwargs"]["enable_mcp"] is False
    assert captured["scheduled_kwargs"]["outline_data"][0]["title"] == "?2?"

async def test_should_build_outline_continue_context_with_bulk_prefetched_relationships_and_careers(test_engine, monkeypatch):
    async with test_engine.begin() as conn:
        await conn.run_sync(Base.metadata.create_all)

    session_maker = async_sessionmaker(
        test_engine,
        class_=AsyncSession,
        expire_on_commit=False,
    )

    async with session_maker() as session:
        project = Project(
            user_id="user-1",
            title="Context Project",
            theme="悬疑冒险",
            genre="东方奇幻",
            narrative_perspective="third_person",
        )
        session.add(project)
        await session.flush()

        hero = Character(
            project_id=project.id,
            name="林川",
            role_type="protagonist",
            personality="冷静克制",
        )
        ally = Character(
            project_id=project.id,
            name="苏槿",
            role_type="supporting",
            personality="敏锐果决",
        )
        organization_character = Character(
            project_id=project.id,
            name="夜巡司",
            role_type="supporting",
            is_organization=True,
            organization_type="情报组织",
            organization_purpose="追查城内异象",
        )
        session.add_all([hero, ally, organization_character])
        await session.flush()

        session.add(
            CharacterRelationship(
                project_id=project.id,
                character_from_id=hero.id,
                character_to_id=ally.id,
                relationship_name="盟友",
            )
        )

        organization = Organization(
            project_id=project.id,
            character_id=organization_character.id,
            member_count=1,
        )
        session.add(organization)
        await session.flush()

        session.add(
            OrganizationMember(
                organization_id=organization.id,
                character_id=hero.id,
                position="统领",
            )
        )

        career = Career(
            project_id=project.id,
            name="夜巡人",
            type="main",
            stages="[]",
            max_stage=5,
        )
        session.add(career)
        await session.flush()

        session.add(
            CharacterCareer(
                character_id=hero.id,
                career_id=career.id,
                career_type="main",
                current_stage=2,
            )
        )

        outline = Outline(
            project_id=project.id,
            title="第1章",
            content="城门异动初现",
            order_index=1,
            structure=json.dumps(
                {"summary": "夜巡司首次锁定异常源头", "characters": ["林川", "苏槿"]},
                ensure_ascii=False,
            ),
        )
        session.add(outline)
        await session.commit()
        await session.refresh(project)
        await session.refresh(hero)
        await session.refresh(ally)
        await session.refresh(organization_character)
        await session.refresh(outline)

        async def fake_build_context_for_generation(**kwargs):
            return {
                "recent_context": "前情提要：夜巡司已发现城南裂隙。",
                "character_states": "角色状态：林川表面镇定，内心警惕。",
                "foreshadows": "怀表异响尚未回收",
                "plot_points": "关键矛盾：裂隙正在扩散。",
            }

        async def fake_quality_guidance(*args, **kwargs):
            return {
                "quality_repair_guidance": "保持冲突链清晰",
                "quality_trend_guidance": "延续悬疑压迫感",
            }

        monkeypatch.setattr(
            outlines_api.memory_service,
            "build_context_for_generation",
            fake_build_context_for_generation,
        )
        monkeypatch.setattr(
            outlines_api,
            "_build_outline_quality_guidance_bundle",
            fake_quality_guidance,
        )

        context = await outlines_api._build_outline_continue_context(
            user_id="user-1",
            project=project,
            latest_outlines=[outline],
            characters=[hero, ally, organization_character],
            current_chapter=2,
            chapter_count=2,
            plot_stage="development",
            story_direction="追查裂隙源头",
            requirements="强化角色协作",
            db=session,
        )

    assert "与苏槿：盟友" in context["characters_info"]
    assert "职业：夜巡人（2阶段）" in context["characters_info"]
    assert "组织成员：林川（统领）" in context["characters_info"]
    assert "保持冲突链清晰" in context["quality_repair_guidance"]
    assert "怀表异响尚未回收" in context["memory_guidance"]


async def test_should_compact_outline_continue_context_payload_for_prompt_budget(test_engine, monkeypatch):
    async with test_engine.begin() as conn:
        await conn.run_sync(Base.metadata.create_all)

    session_maker = async_sessionmaker(
        test_engine,
        class_=AsyncSession,
        expire_on_commit=False,
    )

    async with session_maker() as session:
        project = Project(
            user_id="user-2",
            title="Budget Project",
            theme="????",
            genre="????",
            narrative_perspective="third_person",
        )
        session.add(project)
        await session.flush()

        characters = []
        for index in range(12):
            character = Character(
                project_id=project.id,
                name=f"??{index + 1}",
                role_type="protagonist" if index == 0 else "supporting",
                personality="????" * 20,
                background="????????????" * 20,
            )
            characters.append(character)
        session.add_all(characters)
        await session.flush()

        outlines = []
        for chapter_no in range(1, 11):
            outline = Outline(
                project_id=project.id,
                title=f"?{chapter_no}?",
                content="?????????" * 40,
                order_index=chapter_no,
                structure=json.dumps(
                    {
                        "summary": "???????????????????????" * 20,
                        "key_points": ["????" * 10, "????" * 10, "????" * 10],
                        "characters": [characters[0].name, characters[1].name, characters[2].name],
                        "emotion": "???????" * 8,
                        "goal": "???????????????" * 8,
                        "scenes": ["???" * 8, "????" * 8, "???" * 8],
                    },
                    ensure_ascii=False,
                ),
            )
            outlines.append(outline)
        session.add_all(outlines)
        await session.commit()
        await session.refresh(project)

        async def fake_build_context_for_generation(**kwargs):
            return {
                "recent_context": "????????????",
                "character_states": "????????????",
                "foreshadows": "???????",
                "plot_points": "?????????????",
            }

        async def fake_quality_guidance(*args, **kwargs):
            return {
                "quality_repair_guidance": "??????",
                "quality_trend_guidance": "??????",
            }

        monkeypatch.setattr(
            outlines_api.memory_service,
            "build_context_for_generation",
            fake_build_context_for_generation,
        )
        monkeypatch.setattr(
            outlines_api,
            "_build_outline_quality_guidance_bundle",
            fake_quality_guidance,
        )

        context = await outlines_api._build_outline_continue_context(
            user_id="user-2",
            project=project,
            latest_outlines=outlines,
            characters=characters,
            current_chapter=11,
            chapter_count=2,
            plot_stage="development",
            story_direction="???1???????????2??",
            requirements="??????????????",
            db=session,
        )

    assert context["stats"]["recent_outlines_count"] == 8
    assert context["stats"]["detailed_characters_count"] == 10
    assert context["stats"]["compacted_characters_count"] == 2
    assert "??????" in context["characters_info"]
    assert "..." in context["recent_outlines"]

async def test_should_commit_outline_postprocess_items_incrementally_in_background(monkeypatch):
    captured = {
        "character_kwargs": None,
        "organization_kwargs": None,
        "commits": [],
    }

    class FakeSession:
        def __init__(self, label: str):
            self.label = label

        async def commit(self):
            captured["commits"].append(self.label)

    class FakeSessionContext:
        def __init__(self, label: str):
            self.session = FakeSession(label)

        async def __aenter__(self):
            return self.session

        async def __aexit__(self, exc_type, exc, tb):
            return False

    session_labels = iter(["characters", "organizations"])

    async def fake_get_session_factory(_session_key):
        def factory():
            return FakeSessionContext(next(session_labels))
        return factory

    async def fake_check_characters(**kwargs):
        captured["character_kwargs"] = kwargs
        return {"created_count": 1, "created_characters": []}

    async def fake_check_organizations(**kwargs):
        captured["organization_kwargs"] = kwargs
        return {"created_count": 1, "created_organizations": []}

    monkeypatch.setattr(outlines_api, "get_session_factory", fake_get_session_factory)
    monkeypatch.setattr(outlines_api, "_check_and_create_missing_characters_from_outlines", fake_check_characters)
    monkeypatch.setattr(outlines_api, "_check_and_create_missing_organizations_from_outlines", fake_check_organizations)

    ai_service = type("FakeAIService", (), {"user_id": "u-1", "db_session": object()})()

    await outlines_api._run_outline_postprocess_background(
        outline_data=[{"title": "Outline", "characters": [{"name": "Lin", "type": "character"}]}],
        project_id="project-1",
        user_ai_service=ai_service,
        user_id="u-1",
        enable_mcp=True,
    )

    assert captured["character_kwargs"] is not None
    assert captured["organization_kwargs"] is not None
    assert captured["character_kwargs"]["commit_per_item"] is True
    assert captured["organization_kwargs"]["commit_per_item"] is True
    assert captured["commits"] == ["characters", "organizations"]


def test_should_build_outline_request_options_for_sub2api_generation():
    ai_service = SimpleNamespace(
        api_provider="sub2api",
        config=SimpleNamespace(retry=SimpleNamespace(max_retries=1)),
    )

    request_options = outlines_api._build_outline_generation_request_options(ai_service)

    assert request_options == {
        "prefer_chat_completions": True,
        "transport_max_retries": 1,
        "first_chunk_timeout": 20.0,
        "allow_non_stream_fallback": False,
    }
