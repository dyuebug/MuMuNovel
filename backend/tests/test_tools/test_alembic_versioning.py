from types import SimpleNamespace
from unittest.mock import Mock

from sqlalchemy import String

from tools.alembic_versioning import (
    ALEMBIC_VERSION_NUM_LENGTH,
    ensure_version_table_column_capacity,
    patch_default_impl_version_table,
)


class _DummyInspector:
    def __init__(self, *, table_names, columns):
        self._table_names = table_names
        self._columns = columns

    def get_table_names(self, schema=None):
        return list(self._table_names)

    def get_columns(self, table_name, schema=None):
        return list(self._columns)


def test_should_alter_postgres_alembic_version_column_when_length_is_too_short(monkeypatch):
    connection = SimpleNamespace(
        dialect=SimpleNamespace(name='postgresql'),
        execute=Mock(),
    )
    monkeypatch.setattr(
        'tools.alembic_versioning.inspect',
        lambda _: _DummyInspector(
            table_names=['alembic_version'],
            columns=[{'name': 'version_num', 'type': String(32)}],
        ),
    )

    changed = ensure_version_table_column_capacity(connection)

    assert changed is True
    assert connection.execute.call_count == 1
    sql_text = str(connection.execute.call_args.args[0])
    assert f'ALTER COLUMN version_num TYPE VARCHAR({ALEMBIC_VERSION_NUM_LENGTH})' in sql_text


def test_should_skip_alter_for_sqlite_even_when_length_is_too_short(monkeypatch):
    connection = SimpleNamespace(
        dialect=SimpleNamespace(name='sqlite'),
        execute=Mock(),
    )
    monkeypatch.setattr(
        'tools.alembic_versioning.inspect',
        lambda _: _DummyInspector(
            table_names=['alembic_version'],
            columns=[{'name': 'version_num', 'type': String(32)}],
        ),
    )

    changed = ensure_version_table_column_capacity(connection)

    assert changed is False
    connection.execute.assert_not_called()


def test_should_patch_default_impl_version_table_to_use_longer_revision_capacity():
    class DummyImpl:
        pass

    patch_default_impl_version_table(DummyImpl)
    table = DummyImpl().version_table_impl(
        version_table='alembic_version',
        version_table_schema=None,
        version_table_pk=True,
    )

    assert table.c.version_num.type.length == ALEMBIC_VERSION_NUM_LENGTH
    assert any(constraint.name == 'alembic_version_pkc' for constraint in table.constraints)
