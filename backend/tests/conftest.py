from datetime import datetime
from typing import Optional

import pytest
import pytest_asyncio
from fastapi import FastAPI
from httpx import ASGITransport, AsyncClient
from sqlalchemy import delete
from sqlalchemy.ext.asyncio import AsyncSession, async_sessionmaker, create_async_engine
from sqlalchemy.pool import StaticPool

from app.api import settings as settings_api
from app.models.settings import Settings
from app.user_manager import User


def _build_test_app(test_db: AsyncSession, user: Optional[User] = None) -> FastAPI:
    app = FastAPI()
    app.include_router(settings_api.router, prefix="/api")

    async def override_get_db():
        yield test_db

    app.dependency_overrides[settings_api.get_db] = override_get_db

    if user is not None:
        def override_require_login() -> User:
            return user

        app.dependency_overrides[settings_api.require_login] = override_require_login

    return app


@pytest_asyncio.fixture
async def test_engine():
    engine = create_async_engine(
        "sqlite+aiosqlite://",
        connect_args={"check_same_thread": False},
        poolclass=StaticPool,
    )

    async with engine.begin() as conn:
        await conn.run_sync(Settings.__table__.create)

    try:
        yield engine
    finally:
        await engine.dispose()


@pytest_asyncio.fixture
async def test_db(test_engine) -> AsyncSession:
    session_maker = async_sessionmaker(
        test_engine,
        class_=AsyncSession,
        expire_on_commit=False,
    )

    async with session_maker() as session:
        yield session
        await session.execute(delete(Settings))
        await session.commit()


@pytest.fixture
def mock_user() -> User:
    now = datetime.utcnow().isoformat()
    return User(
        user_id="test_user_001",
        username="test_user",
        display_name="Test User",
        avatar_url=None,
        trust_level=1,
        is_admin=False,
        linuxdo_id="linuxdo_test_001",
        created_at=now,
        last_login=now,
    )


@pytest_asyncio.fixture
async def mock_settings(test_db: AsyncSession, mock_user: User) -> Settings:
    settings = Settings(
        user_id=mock_user.user_id,
        api_provider="openai",
        api_key="sk-existing",
        api_base_url="https://api.openai.com/v1",
        llm_model="gpt-4o-mini",
        temperature=0.7,
        max_tokens=2048,
        preferences="{}",
    )
    test_db.add(settings)
    await test_db.commit()
    await test_db.refresh(settings)
    return settings


@pytest_asyncio.fixture
async def async_client(test_db: AsyncSession, mock_user: User):
    app = _build_test_app(test_db=test_db, user=mock_user)
    transport = ASGITransport(app=app)
    async with AsyncClient(transport=transport, base_url="http://testserver") as client:
        yield client


@pytest_asyncio.fixture
async def unauth_async_client(test_db: AsyncSession):
    app = _build_test_app(test_db=test_db, user=None)
    transport = ASGITransport(app=app)
    async with AsyncClient(transport=transport, base_url="http://testserver") as client:
        yield client
