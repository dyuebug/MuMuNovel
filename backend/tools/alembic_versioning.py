from __future__ import annotations

from typing import Optional

from sqlalchemy import Column, MetaData, PrimaryKeyConstraint, String, Table, inspect, text
from sqlalchemy.engine import Connection

ALEMBIC_VERSION_NUM_LENGTH = 64


def _quote_identifier(identifier: str) -> str:
    return '"' + identifier.replace('"', '""') + '"'


def _qualify_table_name(table_name: str, schema: Optional[str] = None) -> str:
    if schema:
        return f"{_quote_identifier(schema)}.{_quote_identifier(table_name)}"
    return _quote_identifier(table_name)


def build_version_table(
    *,
    version_table: str,
    version_table_schema: Optional[str],
    version_table_pk: bool,
) -> Table:
    table = Table(
        version_table,
        MetaData(),
        Column("version_num", String(ALEMBIC_VERSION_NUM_LENGTH), nullable=False),
        schema=version_table_schema,
    )
    if version_table_pk:
        table.append_constraint(
            PrimaryKeyConstraint("version_num", name=f"{version_table}_pkc")
        )
    return table


def patch_default_impl_version_table(default_impl_cls: type) -> None:
    patched_length = getattr(default_impl_cls, "_mumu_version_table_length", None)
    if patched_length == ALEMBIC_VERSION_NUM_LENGTH:
        return

    def version_table_impl(self, *, version_table: str, version_table_schema: Optional[str], version_table_pk: bool, **kw):
        return build_version_table(
            version_table=version_table,
            version_table_schema=version_table_schema,
            version_table_pk=version_table_pk,
        )

    default_impl_cls.version_table_impl = version_table_impl
    setattr(default_impl_cls, "_mumu_version_table_length", ALEMBIC_VERSION_NUM_LENGTH)


def ensure_version_table_column_capacity(
    connection: Connection,
    *,
    version_table: str = "alembic_version",
    version_table_schema: Optional[str] = None,
) -> bool:
    inspector = inspect(connection)
    table_names = inspector.get_table_names(schema=version_table_schema)
    if version_table not in table_names:
        return False

    version_column = next(
        (
            column
            for column in inspector.get_columns(version_table, schema=version_table_schema)
            if column.get("name") == "version_num"
        ),
        None,
    )
    if version_column is None:
        return False

    current_length = getattr(version_column.get("type"), "length", None)
    if current_length is None or current_length >= ALEMBIC_VERSION_NUM_LENGTH:
        return False

    if connection.dialect.name == "sqlite":
        return False

    qualified_table_name = _qualify_table_name(version_table, version_table_schema)
    connection.execute(
        text(
            f"ALTER TABLE {qualified_table_name} "
            f"ALTER COLUMN version_num TYPE VARCHAR({ALEMBIC_VERSION_NUM_LENGTH})"
        )
    )
    return True
