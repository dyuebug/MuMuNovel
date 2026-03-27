from app.api.outlines import _dump_model_like_payload
from app.schemas.outline import ChapterPlanItem


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
from app.database import Base
from app.models.chapter import Chapter
from app.models.outline import Outline
from app.models.project import Project


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
