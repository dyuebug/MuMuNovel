import json
from types import SimpleNamespace

import pytest
import pytest_asyncio
from sqlalchemy.ext.asyncio import AsyncSession

from app.api import wizard_stream
from app.database import Base
from app.models.project import Project


pytestmark = pytest.mark.asyncio


class FakeAIService:
    def __init__(self):
        self.calls = []
        self.user_id = None
        self.db_session = None

    async def generate_text_stream(self, **kwargs):
        self.calls.append(kwargs)
        payload = {
            "main_careers": [],
            "sub_careers": [],
        }
        yield json.dumps(payload, ensure_ascii=False)

    def _clean_json_response(self, text: str) -> str:
        return text


@pytest_asyncio.fixture
async def wizard_stream_db(test_db: AsyncSession):
    async with test_db.bind.begin() as conn:
        await conn.run_sync(Base.metadata.create_all)
    yield test_db


async def test_should_forward_enable_mcp_to_career_system_stream(monkeypatch, wizard_stream_db: AsyncSession, mock_user):
    fake_ai_service = FakeAIService()

    async def fake_collect_assets(**kwargs):
        return {"assets": []}

    async def fake_get_template(*args, **kwargs):
        return "template"

    def fake_format_prompt(template, **kwargs):
        return "career prompt"

    monkeypatch.setattr(wizard_stream.chapter_web_research_service, "collect_assets", fake_collect_assets)
    monkeypatch.setattr(wizard_stream.PromptService, "get_template", fake_get_template)
    monkeypatch.setattr(wizard_stream.PromptService, "format_prompt", fake_format_prompt)

    project = Project(
        user_id=mock_user.user_id,
        title="Career Project",
        description="seed project",
        theme="命运",
        genre="玄幻",
    )
    wizard_stream_db.add(project)
    await wizard_stream_db.commit()
    await wizard_stream_db.refresh(project)

    chunks = []
    async for chunk in wizard_stream.career_system_generator(
        {
            "project_id": project.id,
            "provider": "test-provider",
            "model": "test-model",
            "enable_mcp": False,
            "user_id": mock_user.user_id,
        },
        wizard_stream_db,
        fake_ai_service,
    ):
        chunks.append(chunk)

    assert fake_ai_service.calls
    assert fake_ai_service.calls[0]["auto_mcp"] is False
    assert any("生成完成" in chunk for chunk in chunks)


async def test_should_use_custom_web_research_query_for_world_building(monkeypatch, wizard_stream_db: AsyncSession, mock_user):
    fake_ai_service = FakeAIService()
    captured: dict[str, object] = {}

    async def fake_collect_assets(**kwargs):
        captured.update(kwargs)
        return {"assets": []}

    async def fake_get_template(*args, **kwargs):
        return "template"

    def fake_format_prompt(template, **kwargs):
        return "world prompt"

    monkeypatch.setattr(wizard_stream.chapter_web_research_service, "collect_assets", fake_collect_assets)
    monkeypatch.setattr(wizard_stream.PromptService, "get_template", fake_get_template)
    monkeypatch.setattr(wizard_stream.PromptService, "format_prompt", fake_format_prompt)

    chunks = []
    async for chunk in wizard_stream.world_building_generator(
        {
            "title": "World Project",
            "description": "seed project",
            "theme": "mystery",
            "genre": "fantasy",
            "enable_web_research": True,
            "web_research_query": "custom worldbuilding reference set",
            "user_id": mock_user.user_id,
        },
        wizard_stream_db,
        fake_ai_service,
    ):
        chunks.append(chunk)

    assert captured["exa_query"] == "custom worldbuilding reference set"
    assert captured["enable_web_research"] is True
    assert any("custom worldbuilding reference set" in chunk for chunk in chunks)


async def test_should_include_research_query_in_outline_result(monkeypatch, wizard_stream_db: AsyncSession, mock_user):
    class _OutlineAIService(FakeAIService):
        async def generate_text_stream(self, **kwargs):
            self.calls.append(kwargs)
            payload = [
                {
                    "title": "Opening",
                    "summary": "The world shakes.",
                    "content": "The world shakes and the hero wakes.",
                }
            ]
            yield json.dumps(payload, ensure_ascii=False)

    fake_ai_service = _OutlineAIService()

    async def fake_collect_assets(**kwargs):
        return {"query": "outline custom query", "assets": []}

    async def fake_get_template(*args, **kwargs):
        return "template"

    def fake_format_prompt(template, **kwargs):
        return "outline prompt"

    async def fake_build_story_packet(*args, **kwargs):
        guidance = SimpleNamespace(
            creative_mode=None,
            story_focus=None,
            plot_stage=None,
            story_creation_brief=None,
            quality_preset=None,
            quality_notes=None,
        )
        return SimpleNamespace(
            guidance=guidance,
            to_prompt_fields=lambda **kwargs: {},
        )

    async def fake_save_project_research_assets(**kwargs):
        return None

    monkeypatch.setattr(wizard_stream.chapter_web_research_service, "collect_assets", fake_collect_assets)
    monkeypatch.setattr(wizard_stream.PromptService, "get_template", fake_get_template)
    monkeypatch.setattr(wizard_stream.PromptService, "format_prompt", fake_format_prompt)
    monkeypatch.setattr(
        wizard_stream,
        "build_story_generation_packet_with_project_continuity",
        fake_build_story_packet,
    )
    monkeypatch.setattr(wizard_stream, "_save_project_research_assets", fake_save_project_research_assets)

    project = Project(
        user_id=mock_user.user_id,
        title="Outline Project",
        description="seed project",
        theme="mystery",
        genre="fantasy",
        outline_mode="one-to-many",
    )
    wizard_stream_db.add(project)
    await wizard_stream_db.commit()
    await wizard_stream_db.refresh(project)

    chunks = []
    async for chunk in wizard_stream.outline_generator(
        {
            "project_id": project.id,
            "chapter_count": 3,
            "narrative_perspective": "third_person",
            "target_words": 100000,
            "user_id": mock_user.user_id,
        },
        wizard_stream_db,
        fake_ai_service,
    ):
        chunks.append(chunk)

    joined = "".join(chunks)
    assert 'outline custom query' in joined
