import json
from types import SimpleNamespace

import pytest
import pytest_asyncio
from sqlalchemy.ext.asyncio import AsyncSession

from tests.test_support.database_test_support import Base
from migrator_app.models import Career
from migrator_app.models.project import Project
from tests.test_support import outlines_route_test_adapter as outlines_test_adapter
from tests.test_support import wizard_generation_test_support as wizard_generation_test_support


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

    monkeypatch.setattr(wizard_generation_test_support.chapter_web_research_service, "collect_assets", fake_collect_assets)
    monkeypatch.setattr(wizard_generation_test_support, "get_template", fake_get_template)
    monkeypatch.setattr(wizard_generation_test_support, "format_prompt", fake_format_prompt)

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
    async for chunk in wizard_generation_test_support.career_system_generator(
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

    monkeypatch.setattr(wizard_generation_test_support.chapter_web_research_service, "collect_assets", fake_collect_assets)
    monkeypatch.setattr(wizard_generation_test_support, "get_template", fake_get_template)
    monkeypatch.setattr(wizard_generation_test_support, "format_prompt", fake_format_prompt)

    chunks = []
    async for chunk in wizard_generation_test_support.world_building_generator(
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


async def test_should_forward_openai_compatible_request_options_to_world_building_stream(
    monkeypatch,
    wizard_stream_db: AsyncSession,
    mock_user,
):
    fake_ai_service = FakeAIService()
    fake_ai_service.api_provider = "sub2api"
    fake_ai_service.config = SimpleNamespace(retry=SimpleNamespace(max_retries=5))

    async def fake_collect_assets(**kwargs):
        return {"assets": []}

    async def fake_get_template(*args, **kwargs):
        return "template"

    def fake_format_prompt(template, **kwargs):
        return "world prompt"

    monkeypatch.setattr(wizard_generation_test_support.chapter_web_research_service, "collect_assets", fake_collect_assets)
    monkeypatch.setattr(wizard_generation_test_support, "get_template", fake_get_template)
    monkeypatch.setattr(wizard_generation_test_support, "format_prompt", fake_format_prompt)

    chunks = []
    async for chunk in wizard_generation_test_support.world_building_generator(
        {
            "title": "World Project",
            "description": "seed project",
            "theme": "mystery",
            "genre": "fantasy",
            "provider": "sub2api",
            "model": "deepseek-v4-pro",
            "user_id": mock_user.user_id,
        },
        wizard_stream_db,
        fake_ai_service,
    ):
        chunks.append(chunk)

    assert fake_ai_service.calls
    request_options = fake_ai_service.calls[0]["request_options"]
    assert request_options["prefer_chat_completions"] is True
    assert request_options["prefer_normalized_v1_candidate"] is True
    assert request_options["transport_max_retries"] == 2
    assert request_options["first_chunk_timeout"] == 20.0
    assert request_options["allow_non_stream_fallback"] is False
    assert chunks




async def test_should_use_custom_web_research_query_for_career_system(monkeypatch, wizard_stream_db: AsyncSession, mock_user):
    fake_ai_service = FakeAIService()
    captured: dict[str, object] = {}

    async def fake_collect_assets(**kwargs):
        captured.update(kwargs)
        return {
            "query": "career custom query",
            "assets": [{"title": "career reference", "source": "https://example.com/career", "summary": "Focus on layered careers and promotion logic."}],
        }

    async def fake_get_template(*args, **kwargs):
        return "template"

    def fake_format_prompt(template, **kwargs):
        return "career prompt"

    monkeypatch.setattr(wizard_generation_test_support.chapter_web_research_service, "collect_assets", fake_collect_assets)
    monkeypatch.setattr(wizard_generation_test_support, "get_template", fake_get_template)
    monkeypatch.setattr(wizard_generation_test_support, "format_prompt", fake_format_prompt)

    project = Project(
        user_id=mock_user.user_id,
        title="Career Research Project",
        description="seed project",
        theme="悬疑追凶",
        genre="都市悬疑",
        world_time_period="steam age",
        world_location="临江市旧城区",
        world_rules="advancement depends on relic resonance",
    )
    wizard_stream_db.add(project)
    await wizard_stream_db.commit()
    await wizard_stream_db.refresh(project)

    chunks = []
    async for chunk in wizard_generation_test_support.career_system_generator(
        {
            "project_id": project.id,
            "provider": "test-provider",
            "model": "test-model",
            "enable_web_research": True,
            "web_research_query": "career custom query",
            "user_id": mock_user.user_id,
        },
        wizard_stream_db,
        fake_ai_service,
    ):
        chunks.append(chunk)

    joined = ''.join(chunks)
    assert captured["exa_query"] == "career custom query"
    assert captured["enable_web_research"] is True
    assert 'career custom query' in joined
    assert 'career reference' in joined


async def test_should_use_custom_web_research_query_for_characters(monkeypatch, wizard_stream_db: AsyncSession, mock_user):
    class _CharacterAIService(FakeAIService):
        async def generate_text_stream(self, **kwargs):
            self.calls.append(kwargs)
            payload = [
                {
                    "name": "沈知微",
                    "age": "27",
                    "gender": "女",
                    "role_type": "protagonist",
                    "personality": "calm and sharp",
                    "background": "an investigative reporter chasing an old case.",
                    "appearance": "short hair and a dark trench coat.",
                    "career_assignment": {
                        "main_career": "investigative reporter",
                        "main_stage": 2,
                        "sub_careers": [],
                    },
                    "relationships_array": [],
                    "organization_memberships": [],
                    "traits": ["冷静", "敏锐"],
                }
            ]
            yield json.dumps(payload, ensure_ascii=False)

    fake_ai_service = _CharacterAIService()
    captured: dict[str, object] = {}

    async def fake_collect_assets(**kwargs):
        captured.update(kwargs)
        return {
            "query": "character custom query",
            "assets": [{"title": "reporter voice sample", "source": "https://example.com/character", "summary": "Balance professional instinct with investigation pressure."}],
        }

    async def fake_get_template(*args, **kwargs):
        return "template"

    def fake_format_prompt(template, **kwargs):
        return "character prompt"

    monkeypatch.setattr(wizard_generation_test_support.chapter_web_research_service, "collect_assets", fake_collect_assets)
    monkeypatch.setattr(wizard_generation_test_support, "get_template", fake_get_template)
    monkeypatch.setattr(wizard_generation_test_support, "format_prompt", fake_format_prompt)

    project = Project(
        user_id=mock_user.user_id,
        title="Character Research Project",
        description="seed project",
        theme="悬疑追凶",
        genre="都市悬疑",
        world_time_period="现代都市",
        world_location="old harbor city",
        world_rules="truth triggers cascading costs",
    )
    wizard_stream_db.add(project)
    await wizard_stream_db.commit()
    await wizard_stream_db.refresh(project)

    main_career = Career(
        project_id=project.id,
        name="investigative reporter",
        type="main",
        description="frontline journalist tracking the truth",
        category="调查记者",
        stages=json.dumps([{"level": 1, "name": "junior reporter"}, {"level": 2, "name": "investigative reporter"}], ensure_ascii=False),
        max_stage=5,
    )
    wizard_stream_db.add(main_career)
    await wizard_stream_db.commit()
    await wizard_stream_db.refresh(main_career)

    chunks = []
    async for chunk in wizard_generation_test_support.characters_generator(
        {
            "project_id": project.id,
            "count": 1,
            "theme": project.theme,
            "genre": project.genre,
            "world_context": {
                "time_period": project.world_time_period,
                "location": project.world_location,
                "atmosphere": "tense and damp",
                "rules": project.world_rules,
            },
            "enable_web_research": True,
            "web_research_query": "character custom query",
            "user_id": mock_user.user_id,
        },
        wizard_stream_db,
        fake_ai_service,
    ):
        chunks.append(chunk)

    joined = ''.join(chunks)
    assert captured["exa_query"] == "character custom query"
    assert captured["enable_web_research"] is True
    assert 'character custom query' in joined
    assert 'reporter voice sample' in joined


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

    monkeypatch.setattr(outlines_test_adapter.chapter_web_research_service, "collect_assets", fake_collect_assets)
    monkeypatch.setattr(outlines_test_adapter, "get_template", fake_get_template)
    monkeypatch.setattr(outlines_test_adapter, "format_prompt", fake_format_prompt)
    monkeypatch.setattr(
        outlines_test_adapter,
        "build_story_generation_packet_with_project_continuity",
        fake_build_story_packet,
    )
    monkeypatch.setattr(outlines_test_adapter, "_save_project_research_assets", fake_save_project_research_assets)

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
    async for chunk in outlines_test_adapter.new_outline_generator(
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



async def test_should_merge_reference_research_assets_into_world_building_prompt(
    monkeypatch,
    wizard_stream_db: AsyncSession,
    mock_user,
):
    class _WorldAIService(FakeAIService):
        async def generate_text_stream(self, **kwargs):
            self.calls.append(kwargs)
            payload = {
                "time_period": "industrial era",
                "location": "fog harbor",
                "atmosphere": "tense and damp",
                "rules": "truth has a price",
            }
            yield json.dumps(payload, ensure_ascii=False)

    fake_ai_service = _WorldAIService()
    captured_prompt: dict[str, object] = {}
    saved_payload: dict[str, object] = {}

    async def fake_collect_assets(**kwargs):
        return {
            "query": "world custom query",
            "assets": [
                {
                    "title": "fresh world clue",
                    "source": "https://example.com/world",
                    "summary": "Dockside trade rituals create social pressure.",
                }
            ],
        }

    async def fake_get_template(*args, **kwargs):
        return "template"

    def fake_format_prompt(template, **kwargs):
        captured_prompt.update(kwargs)
        titles = [asset["title"] for asset in kwargs.get("external_assets", [])]
        return "world prompt | " + " | ".join(titles)

    async def fake_save_project_research_assets(**kwargs):
        saved_payload.update(kwargs)
        return None

    monkeypatch.setattr(wizard_generation_test_support.chapter_web_research_service, "collect_assets", fake_collect_assets)
    monkeypatch.setattr(wizard_generation_test_support, "get_template", fake_get_template)
    monkeypatch.setattr(wizard_generation_test_support, "format_prompt", fake_format_prompt)
    monkeypatch.setattr(wizard_generation_test_support, "_save_project_research_assets", fake_save_project_research_assets)

    chunks = []
    async for chunk in wizard_generation_test_support.world_building_generator(
        {
            "title": "World Project",
            "description": "seed project",
            "theme": "mystery",
            "genre": "fantasy",
            "enable_web_research": True,
            "web_research_query": "world custom query",
            "reference_research_assets": [
                {
                    "title": "carried inspiration note",
                    "source": "https://example.com/inspiration",
                    "summary": "Autopsy details should stay grounded and procedural.",
                }
            ],
            "user_id": mock_user.user_id,
        },
        wizard_stream_db,
        fake_ai_service,
    ):
        chunks.append(chunk)

    merged_titles = [asset["title"] for asset in captured_prompt["external_assets"]]
    assert merged_titles == ["carried inspiration note", "fresh world clue"]
    assert captured_prompt["reference_assets"] == captured_prompt["external_assets"]
    assert [asset["title"] for asset in saved_payload["assets"]] == [
        "carried inspiration note",
        "fresh world clue",
    ]

    joined = "".join(chunks)
    assert "carried inspiration note" in joined
    assert "fresh world clue" in joined



async def test_should_merge_reference_research_assets_into_outline_prompt_and_result(
    monkeypatch,
    wizard_stream_db: AsyncSession,
    mock_user,
):
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
    captured_prompt: dict[str, object] = {}
    saved_payload: dict[str, object] = {}

    async def fake_collect_assets(**kwargs):
        return {
            "query": "outline custom query",
            "assets": [
                {
                    "title": "fresh outline clue",
                    "source": "https://example.com/outline",
                    "summary": "Slow-burn openings work better when stakes surface early.",
                }
            ],
        }

    async def fake_get_template(*args, **kwargs):
        return "template"

    def fake_format_prompt(template, **kwargs):
        captured_prompt.update(kwargs)
        titles = [asset["title"] for asset in kwargs.get("external_assets", [])]
        return "outline prompt | " + " | ".join(titles)

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
        saved_payload.update(kwargs)
        return None

    monkeypatch.setattr(outlines_test_adapter.chapter_web_research_service, "collect_assets", fake_collect_assets)
    monkeypatch.setattr(outlines_test_adapter, "get_template", fake_get_template)
    monkeypatch.setattr(outlines_test_adapter, "format_prompt", fake_format_prompt)
    monkeypatch.setattr(
        outlines_test_adapter,
        "build_story_generation_packet_with_project_continuity",
        fake_build_story_packet,
    )
    monkeypatch.setattr(outlines_test_adapter, "_save_project_research_assets", fake_save_project_research_assets)

    project = Project(
        user_id=mock_user.user_id,
        title="Outline Project",
        description="seed project",
        theme="mystery",
        genre="fantasy",
        outline_mode="one-to-many",
        world_time_period="industrial era",
        world_location="fog harbor",
        world_atmosphere="tense and damp",
        world_rules="truth has a price",
    )
    wizard_stream_db.add(project)
    await wizard_stream_db.commit()
    await wizard_stream_db.refresh(project)

    chunks = []
    async for chunk in outlines_test_adapter.new_outline_generator(
        {
            "project_id": project.id,
            "chapter_count": 3,
            "narrative_perspective": "third_person",
            "target_words": 100000,
            "enable_web_research": True,
            "web_research_query": "outline custom query",
            "reference_research_assets": [
                {
                    "title": "carried inspiration note",
                    "source": "https://example.com/inspiration",
                    "summary": "Keep the emotional hook visible in the first scene.",
                }
            ],
            "user_id": mock_user.user_id,
        },
        wizard_stream_db,
        fake_ai_service,
    ):
        chunks.append(chunk)

    merged_titles = [asset["title"] for asset in captured_prompt["external_assets"]]
    assert merged_titles == ["carried inspiration note", "fresh outline clue"]
    assert captured_prompt["reference_assets"] == captured_prompt["external_assets"]
    assert [asset["title"] for asset in saved_payload["assets"]] == [
        "carried inspiration note",
        "fresh outline clue",
    ]
    assert "carried inspiration note" in fake_ai_service.calls[0]["prompt"]
    assert "fresh outline clue" in fake_ai_service.calls[0]["prompt"]

    joined = "".join(chunks)
    assert "outline custom query" in joined
    assert "carried inspiration note" in joined
    assert "fresh outline clue" in joined
