#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Ensure Alembic version table can store long revision identifiers."""

from __future__ import annotations

import asyncio
import sys
from pathlib import Path
from typing import Optional

from sqlalchemy import inspect
from sqlalchemy.ext.asyncio import create_async_engine
from sqlalchemy.pool import NullPool

PROJECT_ROOT = Path(__file__).resolve().parent.parent
if str(PROJECT_ROOT) not in sys.path:
    sys.path.insert(0, str(PROJECT_ROOT))

from app.config import settings
from tools.alembic_versioning import (
    ALEMBIC_VERSION_NUM_LENGTH,
    ensure_version_table_column_capacity,
)


def _get_version_num_length(connection) -> Optional[int]:
    inspector = inspect(connection)
    if "alembic_version" not in inspector.get_table_names():
        return None
    for column in inspector.get_columns("alembic_version"):
        if column.get("name") == "version_num":
            return getattr(column.get("type"), "length", None)
    return None


async def main() -> int:
    engine = create_async_engine(settings.database_url, poolclass=NullPool)
    try:
        async with engine.begin() as connection:
            before = await connection.run_sync(_get_version_num_length)
            changed = await connection.run_sync(ensure_version_table_column_capacity)
            after = await connection.run_sync(_get_version_num_length)

        if before is None:
            print("INFO: alembic_version table not found; it will be created during migration.")
            return 0
        if changed:
            print(
                f"OK: expanded alembic_version.version_num from VARCHAR({before}) to VARCHAR({after or ALEMBIC_VERSION_NUM_LENGTH})"
            )
            return 0
        if after is not None and after >= ALEMBIC_VERSION_NUM_LENGTH:
            print(f"OK: alembic_version.version_num already has sufficient capacity: VARCHAR({after})")
            return 0

        print(
            "WARN: alembic_version.version_num capacity is still insufficient; "
            f"current_length={after}, target_length={ALEMBIC_VERSION_NUM_LENGTH}"
        )
        return 1
    finally:
        await engine.dispose()


if __name__ == "__main__":
    raise SystemExit(asyncio.run(main()))
