import json
import uuid
from typing import Any, Optional

import pytest
import pytest_asyncio
from fastapi import FastAPI
from httpx import ASGITransport, AsyncClient
from sqlalchemy import select
from sqlalchemy.ext.asyncio import AsyncSession

from app.api import characters as characters_api
from app.database import Base
from app.models.career import Career, CharacterCareer
from app.models.character import Character
from app.models.generation_history import GenerationHistory
from app.models.project import Project
from app.models.relationship import CharacterRelationship, Organization, OrganizationMember

pytestmark = pytest.mark.asyncio


class StubAIService:
    def __init__(
        self,
        chunks: Optional[list[Any]] = None,
        clean_result: Optional[str] = None,
        error: Optional[Exception] = None,
    ):
        self.default_model = "stub-model"
        self.chunks = chunks or []
        self.clean_result = clean_result
        self.error = error
        self.last_call_kwargs: dict[str, Any] | None = None

    async def generate_text_stream(self, **kwargs):
        self.last_call_kwargs = kwargs
        if self.error:
            raise self.error
        for chunk in self.chunks:
            yield chunk

    def _clean_json_response(self, text: str) -> str:
        if self.clean_result is not None:
            return self.clean_result
        return text


def _build_characters_test_app(test_db: AsyncSession, user_id: Optional[str]) -> FastAPI:
    app = FastAPI()
    app.include_router(characters_api.router, prefix="/api")

    async def override_get_db():
        yield test_db

    async def override_get_user_ai_service():
        return app.state.user_ai_service

    app.dependency_overrides[characters_api.get_db] = override_get_db
    app.dependency_overrides[characters_api.get_user_ai_service] = override_get_user_ai_service

    @app.middleware("http")
    async def inject_user_id(request, call_next):
        if user_id is not None:
            request.state.user_id = user_id
        return await call_next(request)

    app.state.user_ai_service = StubAIService()
    return app


@pytest_asyncio.fixture
async def characters_db(test_db: AsyncSession):
    async with test_db.bind.begin() as conn:
        await conn.run_sync(Base.metadata.create_all)
    yield test_db


@pytest_asyncio.fixture
async def characters_app(characters_db: AsyncSession, mock_user):
    return _build_characters_test_app(characters_db, mock_user.user_id)


@pytest_asyncio.fixture
async def characters_client(characters_app: FastAPI):
    transport = ASGITransport(app=characters_app)
    async with AsyncClient(transport=transport, base_url="http://testserver") as client:
        yield client


@pytest_asyncio.fixture
async def unauth_characters_client(characters_db: AsyncSession):
    app = _build_characters_test_app(characters_db, None)
    transport = ASGITransport(app=app)
    async with AsyncClient(transport=transport, base_url="http://testserver") as client:
        yield client


async def create_project(db: AsyncSession, user_id: str, **overrides) -> Project:
    project = Project(
        id=overrides.pop("id", str(uuid.uuid4())),
        user_id=user_id,
        title=overrides.pop("title", "测试项目"),
        outline_mode=overrides.pop("outline_mode", "one-to-many"),
        **overrides,
    )
    db.add(project)
    await db.commit()
    await db.refresh(project)
    return project


async def create_character(db: AsyncSession, project_id: str, name: str, **overrides) -> Character:
    character = Character(
        id=overrides.pop("id", str(uuid.uuid4())),
        project_id=project_id,
        name=name,
        role_type=overrides.pop("role_type", "supporting"),
        is_organization=overrides.pop("is_organization", False),
        **overrides,
    )
    db.add(character)
    await db.commit()
    await db.refresh(character)
    return character


async def create_career(
    db: AsyncSession,
    project_id: str,
    name: str,
    career_type: str = "main",
    **overrides,
) -> Career:
    career = Career(
        id=overrides.pop("id", str(uuid.uuid4())),
        project_id=project_id,
        name=name,
        type=career_type,
        stages=overrides.pop(
            "stages",
            json.dumps([{"level": 1, "name": "初阶"}], ensure_ascii=False),
        ),
        max_stage=overrides.pop("max_stage", 10),
        **overrides,
    )
    db.add(career)
    await db.commit()
    await db.refresh(career)
    return career


def patch_prompt_template(monkeypatch):
    async def fake_get_template(cls, template_key: str, user_id: str, db: AsyncSession):
        return "{project_context}\n{user_input}"

    monkeypatch.setattr(characters_api.PromptService, "get_template", classmethod(fake_get_template))


async def test_should_create_character_successfully(
    characters_client: AsyncClient,
    characters_db: AsyncSession,
    mock_user,
):
    project = await create_project(characters_db, mock_user.user_id)

    payload = {
        "project_id": project.id,
        "name": "林舟",
        "age": "19",
        "gender": "男",
        "role_type": "protagonist",
        "personality": "冷静果断",
        "background": "来自北境",
        "appearance": "黑发",
        "traits": json.dumps(["坚毅", "理性"], ensure_ascii=False),
        "sub_careers": json.dumps(
            [{"career_id": "missing-career", "stage": 2}],
            ensure_ascii=False,
        ),
    }

    response = await characters_client.post("/api/characters", json=payload)
    assert response.status_code == 200

    body = response.json()
    assert body["project_id"] == project.id
    assert body["name"] == "林舟"
    assert body["role_type"] == "protagonist"
    assert body["sub_careers"][0]["career_id"] == "missing-career"

    result = await characters_db.execute(select(Character).where(Character.id == body["id"]))
    saved = result.scalar_one_or_none()
    assert saved is not None
    assert saved.name == "林舟"


async def test_should_get_character_detail_with_relationship_summary(
    characters_client: AsyncClient,
    characters_db: AsyncSession,
    mock_user,
):
    project = await create_project(characters_db, mock_user.user_id)
    hero = await create_character(characters_db, project.id, "主角")
    friend = await create_character(characters_db, project.id, "伙伴")

    relationship = CharacterRelationship(
        project_id=project.id,
        character_from_id=hero.id,
        character_to_id=friend.id,
        relationship_name="盟友",
        source="manual",
    )
    characters_db.add(relationship)
    await characters_db.commit()

    response = await characters_client.get(f"/api/characters/{hero.id}")
    assert response.status_code == 200

    body = response.json()
    assert body["id"] == hero.id
    assert "伙伴" in body["relationships"]
    assert "盟友" in body["relationships"]


async def test_should_list_project_characters_with_organization_members_summary(
    characters_client: AsyncClient,
    characters_db: AsyncSession,
    mock_user,
):
    project = await create_project(characters_db, mock_user.user_id)
    org_character = await create_character(
        characters_db,
        project.id,
        "银鹰骑士团",
        is_organization=True,
    )
    member_character = await create_character(characters_db, project.id, "副团长")

    organization = Organization(
        character_id=org_character.id,
        project_id=project.id,
        member_count=1,
        power_level=88,
        location="王都",
        motto="守护王国",
        color="#4455ff",
    )
    characters_db.add(organization)
    await characters_db.flush()

    member = OrganizationMember(
        organization_id=organization.id,
        character_id=member_character.id,
        position="副团长",
        source="manual",
    )
    characters_db.add(member)
    await characters_db.commit()

    response = await characters_client.get(f"/api/characters/project/{project.id}")
    assert response.status_code == 200

    body = response.json()
    assert body["total"] == 2

    org_item = next(item for item in body["items"] if item["id"] == org_character.id)
    assert "副团长" in org_item["organization_members"]


async def test_should_return_404_when_get_character_missing(characters_client: AsyncClient):
    response = await characters_client.get("/api/characters/not-exists")
    assert response.status_code == 404


async def test_should_update_organization_and_sync_org_fields(
    characters_client: AsyncClient,
    characters_db: AsyncSession,
    mock_user,
):
    project = await create_project(characters_db, mock_user.user_id)
    organization_character = await create_character(
        characters_db,
        project.id,
        "旧组织",
        is_organization=True,
        role_type="supporting",
    )

    payload = {
        "name": "新组织",
        "power_level": 95,
        "location": "北境",
        "motto": "荣耀至上",
        "color": "#00ffaa",
    }

    response = await characters_client.put(f"/api/characters/{organization_character.id}", json=payload)
    assert response.status_code == 200

    body = response.json()
    assert body["name"] == "新组织"
    assert body["power_level"] == 95
    assert body["location"] == "北境"
    assert body["motto"] == "荣耀至上"
    assert body["color"] == "#00ffaa"

    org_result = await characters_db.execute(
        select(Organization).where(Organization.character_id == organization_character.id)
    )
    organization = org_result.scalar_one_or_none()
    assert organization is not None
    assert organization.location == "北境"


async def test_should_delete_character_and_cleanup_career_relations(
    characters_client: AsyncClient,
    characters_db: AsyncSession,
    mock_user,
):
    project = await create_project(characters_db, mock_user.user_id)
    main_career = await create_career(characters_db, project.id, "剑士", career_type="main")
    character = await create_character(
        characters_db,
        project.id,
        "待删除角色",
        main_career_id=main_career.id,
        main_career_stage=1,
    )

    relation = CharacterCareer(
        character_id=character.id,
        career_id=main_career.id,
        career_type="main",
        current_stage=1,
        stage_progress=0,
    )
    characters_db.add(relation)
    await characters_db.commit()

    response = await characters_client.delete(f"/api/characters/{character.id}")
    assert response.status_code == 200
    assert "message" in response.json()

    deleted_character = await characters_db.execute(select(Character).where(Character.id == character.id))
    assert deleted_character.scalar_one_or_none() is None

    deleted_relations = await characters_db.execute(
        select(CharacterCareer).where(CharacterCareer.character_id == character.id)
    )
    assert deleted_relations.scalars().all() == []


async def test_should_stream_generated_character_and_persist_record(
    characters_client: AsyncClient,
    characters_app: FastAPI,
    characters_db: AsyncSession,
    mock_user,
    monkeypatch,
):
    patch_prompt_template(monkeypatch)
    project = await create_project(characters_db, mock_user.user_id)

    generated_character = {
        "name": "AI守卫",
        "age": "21",
        "gender": "女",
        "personality": "沉稳",
        "background": "帝都禁卫",
        "appearance": "银甲",
        "traits": ["谨慎", "忠诚"],
        "is_organization": False,
        "relationships": [],
        "organization_memberships": [],
        "career_info": {},
    }
    generated_json = json.dumps(generated_character, ensure_ascii=False)
    characters_app.state.user_ai_service = StubAIService(
        chunks=[{"content": generated_json[:30]}, {"content": generated_json[30:]}]
    )

    response = await characters_client.post(
        "/api/characters/generate-stream",
        json={"project_id": project.id, "name": "AI守卫", "role_type": "supporting"},
    )
    assert response.status_code == 200
    assert response.headers["content-type"].startswith("text/event-stream")

    stream_text = response.text
    assert '"type": "chunk"' in stream_text
    assert '"type": "result"' in stream_text
    assert '"type": "done"' in stream_text
    assert "AI守卫" in stream_text

    saved_character_result = await characters_db.execute(
        select(Character).where(
            Character.project_id == project.id,
            Character.name == "AI守卫",
        )
    )
    assert saved_character_result.scalar_one_or_none() is not None

    history_result = await characters_db.execute(
        select(GenerationHistory).where(GenerationHistory.project_id == project.id)
    )
    assert len(history_result.scalars().all()) == 1


async def test_should_return_error_event_when_stream_response_is_invalid_json(
    characters_client: AsyncClient,
    characters_app: FastAPI,
    characters_db: AsyncSession,
    mock_user,
    monkeypatch,
):
    patch_prompt_template(monkeypatch)
    project = await create_project(characters_db, mock_user.user_id)

    characters_app.state.user_ai_service = StubAIService(
        chunks=[{"content": "not-json"}],
        clean_result="not-json",
    )

    response = await characters_client.post(
        "/api/characters/generate-stream",
        json={"project_id": project.id, "name": "无效响应测试"},
    )
    assert response.status_code == 200

    stream_text = response.text
    assert '"type": "error"' in stream_text


async def test_should_export_characters_successfully(
    characters_client: AsyncClient,
    characters_db: AsyncSession,
    mock_user,
    monkeypatch,
):
    project = await create_project(characters_db, mock_user.user_id)
    character = await create_character(characters_db, project.id, "导出角色")

    captured: dict[str, Any] = {}

    async def fake_export_characters(character_ids, db):
        captured["character_ids"] = character_ids
        return {
            "version": "1.0.0",
            "export_type": "characters",
            "count": 1,
            "data": [{"name": "导出角色"}],
        }

    monkeypatch.setattr(characters_api.ImportExportService, "export_characters", fake_export_characters)

    response = await characters_client.post(
        "/api/characters/export",
        json={"character_ids": [character.id]},
    )
    assert response.status_code == 200

    body = response.json()
    assert body["count"] == 1
    assert captured["character_ids"] == [character.id]
    assert "characters_export_1_" in response.headers["Content-Disposition"]


async def test_should_return_400_when_export_ids_empty(characters_client: AsyncClient):
    response = await characters_client.post(
        "/api/characters/export",
        json={"character_ids": []},
    )
    assert response.status_code == 400


async def test_should_import_characters_successfully(
    characters_client: AsyncClient,
    characters_db: AsyncSession,
    mock_user,
    monkeypatch,
):
    project = await create_project(characters_db, mock_user.user_id)
    captured: dict[str, Any] = {}

    async def fake_import_characters(data, project_id, user_id, db):
        captured["data"] = data
        captured["project_id"] = project_id
        captured["user_id"] = user_id
        return {
            "success": True,
            "message": "导入完成",
            "statistics": {"imported": 1, "skipped": 0, "failed": 0},
            "details": {"imported": ["新角色"], "skipped": [], "errors": []},
            "warnings": [],
        }

    monkeypatch.setattr(characters_api.ImportExportService, "import_characters", fake_import_characters)

    files = {
        "file": (
            "characters.json",
            json.dumps({"data": [{"name": "新角色"}]}, ensure_ascii=False).encode("utf-8"),
            "application/json",
        )
    }
    response = await characters_client.post(
        f"/api/characters/import?project_id={project.id}",
        files=files,
    )
    assert response.status_code == 200

    body = response.json()
    assert body["success"] is True
    assert captured["project_id"] == project.id
    assert captured["user_id"] == mock_user.user_id


async def test_should_return_400_when_import_file_extension_invalid(
    characters_client: AsyncClient,
    characters_db: AsyncSession,
    mock_user,
):
    project = await create_project(characters_db, mock_user.user_id)

    files = {"file": ("characters.txt", b"{}", "text/plain")}
    response = await characters_client.post(
        f"/api/characters/import?project_id={project.id}",
        files=files,
    )
    assert response.status_code == 400
