from types import SimpleNamespace
from unittest.mock import AsyncMock

import pytest

from tools import ensure_alembic_version_table_capacity as tool


class _BeginContext:
    def __init__(self, connection):
        self._connection = connection

    async def __aenter__(self):
        return self._connection

    async def __aexit__(self, exc_type, exc, tb):
        return False


class _DummyEngine:
    def __init__(self, connection):
        self._connection = connection
        self.dispose = AsyncMock()

    def begin(self):
        return _BeginContext(self._connection)


@pytest.mark.asyncio
async def test_should_use_transactional_connection_when_expanding_version_table(monkeypatch):
    connection = SimpleNamespace(run_sync=AsyncMock(side_effect=[32, True, 64]))
    engine = _DummyEngine(connection)

    monkeypatch.setattr(tool, 'create_async_engine', lambda *args, **kwargs: engine)
    monkeypatch.setattr(tool, 'settings', SimpleNamespace(database_url='postgresql+asyncpg://test'))

    exit_code = await tool.main()

    assert exit_code == 0
    assert connection.run_sync.await_count == 3
    engine.dispose.assert_awaited_once()
