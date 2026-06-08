import json
from types import SimpleNamespace
from typing import Any

import pytest
import pytest_asyncio
from fastapi import FastAPI
from httpx import ASGITransport, AsyncClient
from sqlalchemy.ext.asyncio import async_sessionmaker, create_async_engine
from sqlalchemy.pool import StaticPool

from app.api import chapters as chapters_api
from app.services.compat import chapter_generation_route_compat_service
from app.api import chapter_analysis_routes as chapter_analysis_routes_api
from app.api import chapter_analysis_task_routes as chapter_analysis_task_routes_api
from app.api import chapter_annotation_routes as chapter_annotation_routes_api
from app.api import chapter_crud_routes as chapter_crud_routes_api
from app.api import chapter_batch_generation_routes as chapter_batch_generation_routes_api
from app.api import chapter_draft_routes as chapter_draft_routes_api
from app.api import chapter_generation_routes as chapter_generation_routes_api
from app.api import chapter_quality_routes as chapter_quality_routes_api
from app.api import chapter_expansion_plan_routes as chapter_expansion_plan_routes_api
from app.api import chapter_partial_regeneration_routes as chapter_partial_regeneration_routes_api
from app.api import chapter_regeneration_routes as chapter_regeneration_routes_api
from app.database import Base, get_db as app_get_db
from app.models.chapter import Chapter
from app.models.outline import Outline
from app.models.project import Project
from app.services import manual_chapter_analysis_execution_service

REAL_EXECUTE_BATCH_GENERATION_IN_ORDER = chapters_api.execute_batch_generation_in_order

class FakeAIService:
    def __init__(self):
        self.chunks = ["濞翠礁绱￠悧鍥唽A", "濞翠礁绱￠悧鍥唽B"]
        self.calls: list[dict[str, Any]] = []

    async def generate_text_stream(self, **kwargs):
        self.calls.append(kwargs)
        for chunk in self.chunks:
            yield chunk

@pytest.fixture
def fake_ai_service():
    return FakeAIService()

@pytest.fixture(autouse=True)
def mock_side_effect_services(monkeypatch):
    async def fake_delete_chapter_memories(*args, **kwargs):
        return None

    async def fake_delete_chapter_foreshadows(*args, **kwargs):
        return {"deleted_count": 0}

    async def fake_auto_plant_pending_foreshadows(*args, **kwargs):
        return {"planted_count": 0}

    async def fake_analyze_chapter_background(*args, **kwargs):
        return True

    async def fake_execute_batch_generation(*args, **kwargs):
        return None

    monkeypatch.setattr(
        chapters_api.memory_service,
        "delete_chapter_memories",
        fake_delete_chapter_memories,
    )
    monkeypatch.setattr(
        chapters_api.foreshadow_service,
        "delete_chapter_foreshadows",
        fake_delete_chapter_foreshadows,
    )
    monkeypatch.setattr(
        chapter_crud_routes_api.memory_service,
        "delete_chapter_memories",
        fake_delete_chapter_memories,
    )
    monkeypatch.setattr(
        chapter_crud_routes_api.foreshadow_service,
        "delete_chapter_foreshadows",
        fake_delete_chapter_foreshadows,
    )
    monkeypatch.setattr(
        chapters_api.foreshadow_service,
        "auto_plant_pending_foreshadows",
        fake_auto_plant_pending_foreshadows,
    )
    monkeypatch.setattr(
        chapters_api,
        "analyze_chapter_background",
        fake_analyze_chapter_background,
    )
    monkeypatch.setattr(
        manual_chapter_analysis_execution_service,
        "execute_chapter_analysis_background",
        fake_analyze_chapter_background,
    )
    monkeypatch.setattr(
        chapter_generation_route_compat_service,
        "execute_chapter_analysis_background",
        fake_analyze_chapter_background,
    )
    monkeypatch.setattr(
        chapter_analysis_task_routes_api,
        "execute_chapter_analysis_background",
        fake_analyze_chapter_background,
    )
    monkeypatch.setattr(
        chapters_api,
        "execute_batch_generation_in_order",
        fake_execute_batch_generation,
    )

def _build_quality_history_payload(metrics: dict[str, Any]) -> str:
    return json.dumps(
        {
            "log_type": "chapter_generation_quality_v1",
            "quality_metrics": metrics,
        },
        ensure_ascii=False,
    )

@pytest.fixture(autouse=True)
def reset_chapters_runtime_caches():
    chapters_api.task_quality_metrics_cache.clear()
    chapters_api.task_workflow_state_cache.clear()
    if hasattr(chapters_api, "project_quality_trend_cache"):
        chapters_api.project_quality_trend_cache.clear()
    yield
    chapters_api.task_quality_metrics_cache.clear()
    chapters_api.task_workflow_state_cache.clear()
    if hasattr(chapters_api, "project_quality_trend_cache"):
        chapters_api.project_quality_trend_cache.clear()

@pytest_asyncio.fixture
async def chapters_session_factory():
    engine = create_async_engine(
        "sqlite+aiosqlite://",
        connect_args={"check_same_thread": False},
        poolclass=StaticPool,
    )

    async with engine.begin() as conn:
        await conn.run_sync(Base.metadata.create_all)

    try:
        yield async_sessionmaker(engine, expire_on_commit=False)
    finally:
        await engine.dispose()

@pytest_asyncio.fixture
async def chapters_client(chapters_session_factory, fake_ai_service, mock_user, monkeypatch):
    app = FastAPI()
    app.include_router(chapter_crud_routes_api.router, prefix="/api")
    app.include_router(chapter_analysis_routes_api.router, prefix="/api")
    app.include_router(chapter_analysis_task_routes_api.router, prefix="/api")
    app.include_router(chapter_annotation_routes_api.router, prefix="/api")
    app.include_router(chapter_batch_generation_routes_api.router, prefix="/api")
    app.include_router(chapter_draft_routes_api.router, prefix="/api")
    app.include_router(chapter_generation_routes_api.router, prefix="/api")
    app.include_router(chapter_quality_routes_api.router, prefix="/api")
    app.include_router(chapter_expansion_plan_routes_api.router, prefix="/api")
    app.include_router(chapter_partial_regeneration_routes_api.router, prefix="/api")
    app.include_router(chapter_regeneration_routes_api.router, prefix="/api")

    async def override_get_db(_request=None):
        async with chapters_session_factory() as session:
            try:
                yield session
            finally:
                # Allow upstream services to manage transactions, but ensure we don't
                # return a session to the pool with a pending/failed transaction.
                try:
                    if session.in_transaction():
                        await session.rollback()
                except Exception:
                    pass

    async def override_get_user_ai_service():
        return fake_ai_service

    @app.middleware("http")
    async def inject_user_state(request, call_next):
        header_user_id = request.headers.get("x-test-user-id", mock_user.user_id)
        if header_user_id == "__none__":
            request.state.user_id = None
            request.state.user = None
        else:
            request.state.user_id = header_user_id
            request.state.user = (
                mock_user
                if header_user_id == mock_user.user_id
                else SimpleNamespace(user_id=header_user_id)
            )
        return await call_next(request)

    app.dependency_overrides[app_get_db] = override_get_db
    app.dependency_overrides[chapters_api.get_user_ai_service] = override_get_user_ai_service

    monkeypatch.setattr(chapters_api, "get_db", override_get_db)
    monkeypatch.setattr(chapter_regeneration_routes_api, "get_db", override_get_db)
    monkeypatch.setattr(chapter_generation_route_compat_service, "get_db", override_get_db)

    transport = ASGITransport(app=app)
    async with AsyncClient(transport=transport, base_url="http://testserver") as client:
        yield client

def parse_sse_data(stream_text: str) -> list[dict[str, Any]]:
    events: list[dict[str, Any]] = []
    for line in stream_text.splitlines():
        if line.startswith("data: "):
            events.append(json.loads(line.removeprefix("data: ")))
    return events

async def create_project(chapters_session_factory, user_id: str, **overrides) -> Project:
    async with chapters_session_factory() as session:
        project = Project(
            user_id=user_id,
            title=overrides.get("title", "test-project"),
            genre=overrides.get("genre", "fantasy"),
            theme=overrides.get("theme", "adventure"),
            outline_mode=overrides.get("outline_mode", "one-to-many"),
            current_words=overrides.get("current_words", 0),
            narrative_perspective=overrides.get("narrative_perspective", "third_person"),
            default_creative_mode=overrides.get("default_creative_mode"),
            default_story_focus=overrides.get("default_story_focus"),
            default_plot_stage=overrides.get("default_plot_stage"),
            default_story_creation_brief=overrides.get("default_story_creation_brief"),
            default_quality_preset=overrides.get("default_quality_preset"),
            default_quality_notes=overrides.get("default_quality_notes"),
        )
        session.add(project)
        await session.commit()
        await session.refresh(project)
        return project

async def create_outline(
    chapters_session_factory,
    project_id: str,
    order_index: int = 1,
    title: str = "outline-1",
    content: str = "outline content",
) -> Outline:
    async with chapters_session_factory() as session:
        outline = Outline(
            project_id=project_id,
            title=title,
            content=content,
            order_index=order_index,
        )
        session.add(outline)
        await session.commit()
        await session.refresh(outline)
        return outline

async def create_chapter(
    chapters_session_factory,
    project_id: str,
    chapter_number: int,
    title: str,
    content: str | None = None,
    outline_id: str | None = None,
    status: str = "draft",
    expansion_plan: str | None = None,
) -> Chapter:
    async with chapters_session_factory() as session:
        chapter = Chapter(
            project_id=project_id,
            chapter_number=chapter_number,
            title=title,
            content=content,
            word_count=len(content) if content else 0,
            status=status,
            outline_id=outline_id,
            expansion_plan=expansion_plan,
        )
        session.add(chapter)
        await session.commit()
        await session.refresh(chapter)
        return chapter
