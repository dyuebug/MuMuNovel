from datetime import datetime

import pytest
import pytest_asyncio
from sqlalchemy.ext.asyncio import AsyncSession, async_sessionmaker, create_async_engine
from sqlalchemy.pool import StaticPool

from migrator_app.models import User
from migrator_app.models import load_all_models

load_all_models()


@pytest_asyncio.fixture
async def test_engine():
    engine = create_async_engine(
        "sqlite+aiosqlite://",
        connect_args={"check_same_thread": False},
        poolclass=StaticPool,
    )

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
