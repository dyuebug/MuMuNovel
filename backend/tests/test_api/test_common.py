import pytest
import pytest_asyncio
from fastapi import HTTPException, Request
from sqlalchemy import delete
from sqlalchemy.ext.asyncio import AsyncSession, async_sessionmaker

from app.api import common as common_api
from app.models.project import Project

pytestmark = pytest.mark.asyncio


@pytest_asyncio.fixture
async def common_test_db(test_engine) -> AsyncSession:
    # 创建本模块测试所需的 projects 表
    async with test_engine.begin() as conn:
        await conn.run_sync(Project.__table__.create)

    session_maker = async_sessionmaker(
        test_engine,
        class_=AsyncSession,
        expire_on_commit=False,
    )

    async with session_maker() as session:
        yield session
        await session.execute(delete(Project))
        await session.commit()


async def _create_project(
    db: AsyncSession,
    user_id: str,
    title: str = "测试项目",
) -> Project:
    project = Project(
        user_id=user_id,
        title=title,
        description="用于 common API 测试",
    )
    db.add(project)
    await db.commit()
    await db.refresh(project)
    return project


def _build_request(user_id: str | None = None) -> Request:
    # 构造最小可用的 Request 对象
    request = Request(
        {
            "type": "http",
            "method": "GET",
            "path": "/",
            "headers": [],
            "query_string": b"",
        }
    )
    if user_id is not None:
        request.state.user_id = user_id
    return request


async def test_should_raise_401_when_verify_project_access_without_login(common_test_db):
    with pytest.raises(HTTPException) as exc_info:
        await common_api.verify_project_access(
            project_id="project-001",
            user_id=None,
            db=common_test_db,
        )

    assert exc_info.value.status_code == 401


async def test_should_raise_404_when_verify_project_access_project_not_found(common_test_db):
    owned_project = await _create_project(common_test_db, user_id="owner-user")

    with pytest.raises(HTTPException) as exc_info:
        await common_api.verify_project_access(
            project_id=owned_project.id,
            user_id="intruder-user",
            db=common_test_db,
        )

    assert exc_info.value.status_code == 404


async def test_should_return_project_when_verify_project_access_owner_matches(common_test_db):
    created = await _create_project(common_test_db, user_id="owner-user", title="可访问项目")

    project = await common_api.verify_project_access(
        project_id=created.id,
        user_id="owner-user",
        db=common_test_db,
    )

    assert project.id == created.id
    assert project.user_id == "owner-user"
    assert project.title == "可访问项目"


def test_should_return_user_id_when_get_user_id_from_request_state():
    request = _build_request(user_id="user-001")

    user_id = common_api.get_user_id(request)

    assert user_id == "user-001"


def test_should_return_none_when_get_user_id_without_state_value():
    request = _build_request()

    user_id = common_api.get_user_id(request)

    assert user_id is None


async def test_should_delegate_to_verify_project_access_when_verify_project_access_from_request(
    common_test_db,
    mocker,
):
    request = _build_request(user_id="request-user")
    expected_project = Project(id="project-123", user_id="request-user", title="委托项目")

    verify_mock = mocker.patch(
        "app.api.common.verify_project_access",
        new=mocker.AsyncMock(return_value=expected_project),
    )

    result = await common_api.verify_project_access_from_request(
        project_id="project-123",
        request=request,
        db=common_test_db,
    )

    assert result is expected_project
    verify_mock.assert_awaited_once_with("project-123", "request-user", common_test_db)


async def test_should_raise_401_when_verify_project_access_from_request_without_login(
    common_test_db,
):
    request = _build_request()

    with pytest.raises(HTTPException) as exc_info:
        await common_api.verify_project_access_from_request(
            project_id="project-001",
            request=request,
            db=common_test_db,
        )

    assert exc_info.value.status_code == 401
