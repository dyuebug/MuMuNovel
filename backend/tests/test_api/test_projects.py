import json
from datetime import datetime
from unittest.mock import AsyncMock

import pytest
import pytest_asyncio
from fastapi import FastAPI, HTTPException, Request
from httpx import ASGITransport, AsyncClient
from sqlalchemy import select
from sqlalchemy.ext.asyncio import AsyncSession, async_sessionmaker

from app.api import projects as projects_api
from app.database import Base
from app.models.project import Project
from app.schemas.import_export import ImportResult, ProjectExportData

pytestmark = pytest.mark.asyncio


def _build_projects_app(test_db: AsyncSession) -> FastAPI:
    app = FastAPI()
    app.include_router(projects_api.router, prefix="/api")

    async def override_get_db():
        yield test_db

    app.dependency_overrides[projects_api.get_db] = override_get_db

    @app.middleware("http")
    async def inject_user_id(request: Request, call_next):
        request.state.user_id = request.headers.get("x-test-user-id")
        return await call_next(request)

    return app


def auth_headers(user_id: str) -> dict[str, str]:
    return {"x-test-user-id": user_id}


def build_project_payload(**overrides):
    payload = {
        "title": "测试项目",
        "description": "用于测试项目 API 的描述",
        "theme": "成长",
        "genre": "fantasy",
        "target_words": 120000,
        "outline_mode": "one-to-many",
    }
    payload.update(overrides)
    return payload


@pytest_asyncio.fixture
async def project_test_db(test_engine) -> AsyncSession:
    async with test_engine.begin() as conn:
        await conn.run_sync(Base.metadata.create_all)

    session_maker = async_sessionmaker(
        test_engine,
        class_=AsyncSession,
        expire_on_commit=False,
    )
    async with session_maker() as session:
        yield session


@pytest_asyncio.fixture
async def projects_client(project_test_db: AsyncSession):
    app = _build_projects_app(project_test_db)
    transport = ASGITransport(app=app)
    async with AsyncClient(transport=transport, base_url="http://testserver") as client:
        yield client


async def create_project_record(
    db: AsyncSession,
    user_id: str,
    **overrides,
) -> Project:
    data = {
        "title": f"项目-{datetime.utcnow().timestamp()}",
        "description": "seed project",
        "outline_mode": "one-to-many",
    }
    data.update(overrides)

    project = Project(user_id=user_id, **data)
    db.add(project)
    await db.commit()
    await db.refresh(project)
    return project


async def fetch_project(db: AsyncSession, project_id: str):
    result = await db.execute(select(Project).where(Project.id == project_id))
    return result.scalar_one_or_none()


async def test_should_create_project_when_authenticated(
    projects_client,
    project_test_db,
    mock_user,
):
    response = await projects_client.post(
        "/api/projects",
        json=build_project_payload(),
        headers=auth_headers(mock_user.user_id),
    )
    assert response.status_code == 200
    body = response.json()
    assert body["title"] == "测试项目"
    assert body["outline_mode"] == "one-to-many"

    saved = await fetch_project(project_test_db, body["id"])
    assert saved is not None
    assert saved.user_id == mock_user.user_id


async def test_should_return_401_when_create_project_without_login(projects_client):
    response = await projects_client.post("/api/projects", json=build_project_payload())
    assert response.status_code == 401


async def test_should_return_422_when_create_project_with_invalid_outline_mode(
    projects_client,
    mock_user,
):
    response = await projects_client.post(
        "/api/projects",
        json=build_project_payload(outline_mode="invalid-mode"),
        headers=auth_headers(mock_user.user_id),
    )
    assert response.status_code == 422


async def test_should_return_500_when_create_project_commit_fails(
    projects_client,
    mock_user,
    project_test_db,
    monkeypatch,
):
    monkeypatch.setattr(
        project_test_db,
        "commit",
        AsyncMock(side_effect=RuntimeError("db commit failed")),
    )

    response = await projects_client.post(
        "/api/projects",
        json=build_project_payload(title="db-error-project"),
        headers=auth_headers(mock_user.user_id),
    )
    assert response.status_code == 500


async def test_should_list_only_current_user_projects_with_pagination(
    projects_client,
    project_test_db,
    mock_user,
):
    p1 = await create_project_record(project_test_db, mock_user.user_id, title="A")
    p2 = await create_project_record(project_test_db, mock_user.user_id, title="B")
    p3 = await create_project_record(project_test_db, mock_user.user_id, title="C")
    p_other = await create_project_record(project_test_db, "another-user", title="X")

    response = await projects_client.get(
        "/api/projects",
        params={"skip": 1, "limit": 2},
        headers=auth_headers(mock_user.user_id),
    )
    assert response.status_code == 200
    body = response.json()
    assert body["total"] == 3
    assert len(body["items"]) == 2

    returned_ids = {item["id"] for item in body["items"]}
    owned_ids = {p1.id, p2.id, p3.id}
    assert returned_ids.issubset(owned_ids)
    assert p_other.id not in returned_ids


async def test_should_return_empty_items_when_list_limit_is_zero(
    projects_client,
    project_test_db,
    mock_user,
):
    await create_project_record(project_test_db, mock_user.user_id, title="Only One")

    response = await projects_client.get(
        "/api/projects",
        params={"skip": 0, "limit": 0},
        headers=auth_headers(mock_user.user_id),
    )
    assert response.status_code == 200
    body = response.json()
    assert body["total"] == 1
    assert body["items"] == []


async def test_should_return_401_when_list_projects_without_login(projects_client):
    response = await projects_client.get("/api/projects")
    assert response.status_code == 401


async def test_should_get_project_detail_when_owner(
    projects_client,
    project_test_db,
    mock_user,
):
    project = await create_project_record(
        project_test_db,
        mock_user.user_id,
        title="Detail Project",
    )

    response = await projects_client.get(
        f"/api/projects/{project.id}",
        headers=auth_headers(mock_user.user_id),
    )
    assert response.status_code == 200
    assert response.json()["id"] == project.id


async def test_should_return_404_when_get_project_not_owned(
    projects_client,
    project_test_db,
):
    project = await create_project_record(project_test_db, "owner-user", title="Owned")

    response = await projects_client.get(
        f"/api/projects/{project.id}",
        headers=auth_headers("intruder"),
    )
    assert response.status_code == 404


async def test_should_update_project_fields_when_owner(
    projects_client,
    project_test_db,
    mock_user,
):
    project = await create_project_record(project_test_db, mock_user.user_id, title="Before")

    response = await projects_client.put(
        f"/api/projects/{project.id}",
        json={"title": "After", "target_words": 200000},
        headers=auth_headers(mock_user.user_id),
    )
    assert response.status_code == 200
    body = response.json()
    assert body["title"] == "After"
    assert body["target_words"] == 200000


async def test_should_return_404_when_update_project_not_owned(
    projects_client,
    project_test_db,
):
    project = await create_project_record(project_test_db, "project-owner", title="Keep")

    response = await projects_client.put(
        f"/api/projects/{project.id}",
        json={"title": "Hacked"},
        headers=auth_headers("another-user"),
    )
    assert response.status_code == 404


async def test_should_delete_project_and_cleanup_memories(
    projects_client,
    project_test_db,
    mock_user,
    monkeypatch,
):
    project = await create_project_record(project_test_db, mock_user.user_id, title="To Delete")
    cleanup_mock = AsyncMock(return_value=None)
    monkeypatch.setattr(projects_api.memory_service, "delete_project_memories", cleanup_mock)

    response = await projects_client.delete(
        f"/api/projects/{project.id}",
        headers=auth_headers(mock_user.user_id),
    )
    assert response.status_code == 200
    assert "message" in response.json()
    cleanup_mock.assert_awaited_once_with(mock_user.user_id, project.id)

    deleted = await fetch_project(project_test_db, project.id)
    assert deleted is None


async def test_should_return_401_when_delete_project_without_login(
    projects_client,
    project_test_db,
):
    project = await create_project_record(project_test_db, "owner-user", title="No Login Delete")

    response = await projects_client.delete(f"/api/projects/{project.id}")
    assert response.status_code == 401


async def test_should_return_404_when_delete_project_not_owned(
    projects_client,
    project_test_db,
):
    project = await create_project_record(project_test_db, "owner-user", title="Protected")

    response = await projects_client.delete(
        f"/api/projects/{project.id}",
        headers=auth_headers("intruder-user"),
    )
    assert response.status_code == 404


async def test_should_export_project_data_as_json_file(
    projects_client,
    project_test_db,
    mock_user,
    monkeypatch,
):
    project = await create_project_record(project_test_db, mock_user.user_id, title="Exportable")
    export_mock = AsyncMock(
        return_value=ProjectExportData(
            export_time=datetime.utcnow().isoformat(),
            project={"title": project.title},
        )
    )
    monkeypatch.setattr(projects_api.ImportExportService, "export_project", export_mock)

    response = await projects_client.post(
        f"/api/projects/{project.id}/export-data",
        json={
            "include_generation_history": True,
            "include_writing_styles": True,
            "include_careers": False,
            "include_memories": True,
            "include_plot_analysis": False,
        },
        headers=auth_headers(mock_user.user_id),
    )
    assert response.status_code == 200
    assert response.headers["content-type"].startswith("application/json")
    assert "attachment;" in response.headers["content-disposition"]
    assert ".json" in response.headers["content-disposition"]
    assert response.json()["project"]["title"] == project.title

    kwargs = export_mock.await_args.kwargs
    assert kwargs["project_id"] == project.id
    assert kwargs["db"] is project_test_db
    assert kwargs["include_generation_history"] is True
    assert kwargs["include_writing_styles"] is True
    assert kwargs["include_careers"] is False
    assert kwargs["include_memories"] is True
    assert kwargs["include_plot_analysis"] is False


async def test_should_return_401_when_export_project_data_without_login(
    projects_client,
    project_test_db,
):
    project = await create_project_record(project_test_db, "owner-user", title="No Login Export")

    response = await projects_client.post(
        f"/api/projects/{project.id}/export-data",
        json={},
    )
    assert response.status_code == 401


async def test_should_return_404_when_export_project_data_not_owned(
    projects_client,
    project_test_db,
):
    project = await create_project_record(project_test_db, "owner-user", title="Other Export")

    response = await projects_client.post(
        f"/api/projects/{project.id}/export-data",
        json={},
        headers=auth_headers("intruder-user"),
    )
    assert response.status_code == 404


async def test_should_return_403_when_export_project_data_service_denied(
    projects_client,
    project_test_db,
    mock_user,
    monkeypatch,
):
    project = await create_project_record(project_test_db, mock_user.user_id, title="Forbidden")
    monkeypatch.setattr(
        projects_api.ImportExportService,
        "export_project",
        AsyncMock(side_effect=HTTPException(status_code=403, detail="forbidden")),
    )

    response = await projects_client.post(
        f"/api/projects/{project.id}/export-data",
        json={},
        headers=auth_headers(mock_user.user_id),
    )
    assert response.status_code == 403


async def test_should_import_project_from_valid_json_file(
    projects_client,
    project_test_db,
    mock_user,
    monkeypatch,
):
    import_result = ImportResult(
        success=True,
        project_id="imported-project-id",
        message="导入成功",
        statistics={"projects": 1},
        details={"imported": ["project"]},
        warnings=[],
    )
    import_mock = AsyncMock(return_value=import_result)
    monkeypatch.setattr(projects_api.ImportExportService, "import_project", import_mock)

    payload = {"version": "1.1.0", "project": {"title": "Imported"}}
    response = await projects_client.post(
        "/api/projects/import",
        files={
            "file": (
                "project.json",
                json.dumps(payload, ensure_ascii=False).encode("utf-8"),
                "application/json",
            )
        },
        headers=auth_headers(mock_user.user_id),
    )
    assert response.status_code == 200
    body = response.json()
    assert body["success"] is True
    assert body["project_id"] == "imported-project-id"

    args = import_mock.await_args.args
    assert args[0] == payload
    assert args[1] is project_test_db
    assert args[2] == mock_user.user_id


async def test_should_return_401_when_import_project_without_login(projects_client):
    response = await projects_client.post(
        "/api/projects/import",
        files={"file": ("project.json", b"{}", "application/json")},
    )
    assert response.status_code == 401


async def test_should_return_400_when_import_file_extension_is_not_json(
    projects_client,
    mock_user,
):
    response = await projects_client.post(
        "/api/projects/import",
        files={"file": ("project.txt", b"{}", "text/plain")},
        headers=auth_headers(mock_user.user_id),
    )
    assert response.status_code == 400


async def test_should_return_400_when_import_file_has_invalid_json(
    projects_client,
    mock_user,
):
    response = await projects_client.post(
        "/api/projects/import",
        files={"file": ("broken.json", b"{not-valid-json}", "application/json")},
        headers=auth_headers(mock_user.user_id),
    )
    assert response.status_code == 400


async def test_should_check_project_consistency_with_auto_fix_flag(
    projects_client,
    project_test_db,
    mock_user,
    monkeypatch,
):
    project = await create_project_record(project_test_db, mock_user.user_id, title="Consistency")
    expected_report = {
        "project_id": project.id,
        "checks": {
            "organization_records": {"status": "ok", "checked": 0, "fixed": 0},
        },
    }
    check_mock = AsyncMock(return_value=expected_report)
    monkeypatch.setattr(projects_api, "run_full_data_consistency_check", check_mock)

    response = await projects_client.post(
        f"/api/projects/{project.id}/check-consistency?auto_fix=false",
        headers=auth_headers(mock_user.user_id),
    )
    assert response.status_code == 200
    assert response.json() == expected_report

    args = check_mock.await_args.args
    assert args[0] == project.id
    assert args[1] is project_test_db
    assert args[2] is False


async def test_should_return_401_when_check_consistency_without_login(
    projects_client,
    project_test_db,
):
    project = await create_project_record(project_test_db, "owner-user", title="No Login Check")
    response = await projects_client.post(f"/api/projects/{project.id}/check-consistency")
    assert response.status_code == 401


async def test_should_return_404_when_check_consistency_on_project_not_owned(
    projects_client,
    project_test_db,
):
    project = await create_project_record(project_test_db, "owner-user", title="Other Check")
    response = await projects_client.post(
        f"/api/projects/{project.id}/check-consistency",
        headers=auth_headers("intruder-user"),
    )
    assert response.status_code == 404
