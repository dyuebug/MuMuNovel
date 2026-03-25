#!/usr/bin/env python3
"""
Database migration utility.
Used by local/dev/prod deployment flows to serialize Alembic upgrades.
"""
from __future__ import annotations

import contextlib
import hashlib
import os
from pathlib import Path
import subprocess
import sys
import time
from typing import Iterator

try:
    import psycopg2
except Exception:  # pragma: no cover
    psycopg2 = None

project_root = Path(__file__).parent.parent
sys.path.insert(0, str(project_root))

from app.logger import get_logger

logger = get_logger(__name__)

MIGRATION_LOCK_NAME = "mumuainovel:alembic"
MIGRATION_LOCK_TIMEOUT_SECONDS = max(int(os.getenv("MIGRATION_LOCK_TIMEOUT_SECONDS", "300") or 300), 5)
MIGRATION_LOCK_POLL_INTERVAL = max(float(os.getenv("MIGRATION_LOCK_POLL_INTERVAL", "1") or 1), 0.2)


def run_command(cmd: list, description: str) -> bool:
    """Run a shell command and return True on success."""
    try:
        logger.info("Starting %s...", description)
        result = subprocess.run(
            cmd,
            cwd=project_root,
            capture_output=True,
            text=True,
            check=False,
        )

        if result.returncode == 0:
            logger.info("Succeeded: %s", description)
            if result.stdout:
                print(result.stdout)
            return True

        logger.error("Failed: %s", description)
        if result.stderr:
            print(result.stderr, file=sys.stderr)
        return False
    except Exception as exc:
        logger.error("Exception while running %s: %s", description, exc)
        return False


def _resolve_database_url() -> str:
    return os.getenv(
        "DATABASE_URL",
        "postgresql+asyncpg://mumuai:password@localhost:5432/mumuai_novel",
    )


def _resolve_sync_database_url() -> str | None:
    database_url = _resolve_database_url().strip()
    if not database_url:
        return None
    if database_url.startswith("postgresql+asyncpg://"):
        return database_url.replace("postgresql+asyncpg://", "postgresql://", 1)
    if database_url.startswith("postgresql+psycopg2://"):
        return database_url.replace("postgresql+psycopg2://", "postgresql://", 1)
    if database_url.startswith("postgresql://"):
        return database_url
    return None


def _advisory_lock_key(name: str) -> int:
    digest = hashlib.sha1(name.encode("utf-8")).hexdigest()
    return int(digest[:15], 16)


@contextlib.contextmanager
def _postgres_migration_lock(description: str) -> Iterator[None]:
    if psycopg2 is None:
        raise RuntimeError("psycopg2 is unavailable")

    sync_database_url = _resolve_sync_database_url()
    if not sync_database_url:
        raise RuntimeError("current database is not PostgreSQL")

    lock_key = _advisory_lock_key(MIGRATION_LOCK_NAME)
    started_at = time.monotonic()
    connection = psycopg2.connect(sync_database_url)
    connection.autocommit = True
    cursor = connection.cursor()
    acquired = False

    try:
        while time.monotonic() - started_at < MIGRATION_LOCK_TIMEOUT_SECONDS:
            cursor.execute("SELECT pg_try_advisory_lock(%s)", (lock_key,))
            row = cursor.fetchone()
            acquired = bool(row and row[0])
            if acquired:
                logger.info("Acquired PostgreSQL migration lock: %s", description)
                break
            logger.info("Another migration is in progress, waiting for advisory lock...")
            time.sleep(MIGRATION_LOCK_POLL_INTERVAL)

        if not acquired:
            raise TimeoutError(f"Timed out waiting for migration lock after {MIGRATION_LOCK_TIMEOUT_SECONDS}s")

        yield
    finally:
        try:
            if acquired:
                cursor.execute("SELECT pg_advisory_unlock(%s)", (lock_key,))
                logger.info("Released PostgreSQL migration lock")
        finally:
            cursor.close()
            connection.close()


@contextlib.contextmanager
def _file_migration_lock(description: str) -> Iterator[None]:
    lock_path = project_root / ".migration-singleflight.lock"
    lock_path.touch(exist_ok=True)
    lock_file = lock_path.open("r+", encoding="utf-8")
    started_at = time.monotonic()
    acquired = False

    try:
        if os.name == "nt":
            import msvcrt

            while time.monotonic() - started_at < MIGRATION_LOCK_TIMEOUT_SECONDS:
                try:
                    lock_file.seek(0)
                    msvcrt.locking(lock_file.fileno(), msvcrt.LK_NBLCK, 1)
                    acquired = True
                    break
                except OSError:
                    logger.info("Another migration is in progress, waiting for file lock...")
                    time.sleep(MIGRATION_LOCK_POLL_INTERVAL)
        else:
            import fcntl

            while time.monotonic() - started_at < MIGRATION_LOCK_TIMEOUT_SECONDS:
                try:
                    fcntl.flock(lock_file.fileno(), fcntl.LOCK_EX | fcntl.LOCK_NB)
                    acquired = True
                    break
                except OSError:
                    logger.info("Another migration is in progress, waiting for file lock...")
                    time.sleep(MIGRATION_LOCK_POLL_INTERVAL)

        if not acquired:
            raise TimeoutError(f"Timed out waiting for file lock after {MIGRATION_LOCK_TIMEOUT_SECONDS}s")

        logger.info("Acquired migration file lock: %s", description)
        yield
    finally:
        try:
            if acquired:
                if os.name == "nt":
                    import msvcrt

                    lock_file.seek(0)
                    msvcrt.locking(lock_file.fileno(), msvcrt.LK_UNLCK, 1)
                else:
                    import fcntl

                    fcntl.flock(lock_file.fileno(), fcntl.LOCK_UN)
                logger.info("Released migration file lock")
        finally:
            lock_file.close()


@contextlib.contextmanager
def _migration_single_flight(description: str) -> Iterator[None]:
    try:
        with _postgres_migration_lock(description):
            yield
            return
    except Exception as exc:
        logger.warning("PostgreSQL advisory lock unavailable, falling back to file lock: %s", exc)

    with _file_migration_lock(description):
        yield


def create_migration(message: str = None):
    """Create a new Alembic revision."""
    if not message:
        message = input("Enter migration message: ").strip()
        if not message:
            message = "auto_migration"

    cmd = ["alembic", "revision", "--autogenerate", "-m", message]
    return run_command(cmd, f"create migration: {message}")


def ensure_alembic_version_table_capacity() -> bool:
    """Check Alembic version table capacity before upgrade."""
    cmd = [sys.executable, "tools/ensure_alembic_version_table_capacity.py"]
    return run_command(cmd, "check alembic version table capacity")


def upgrade_database(revision: str = "head"):
    """Upgrade database to the target revision."""
    with _migration_single_flight(f"upgrade:{revision}"):
        if not ensure_alembic_version_table_capacity():
            return False
        cmd = ["alembic", "upgrade", revision]
        return run_command(cmd, f"upgrade database to {revision}")


def downgrade_database(revision: str = "-1"):
    """Downgrade database to a target revision."""
    with _migration_single_flight(f"downgrade:{revision}"):
        cmd = ["alembic", "downgrade", revision]
        return run_command(cmd, f"downgrade database to {revision}")


def show_current():
    """Show current revision."""
    cmd = ["alembic", "current"]
    return run_command(cmd, "show current revision")


def show_history():
    """Show migration history."""
    cmd = ["alembic", "history", "--verbose"]
    return run_command(cmd, "show migration history")


def show_heads():
    """Show latest heads."""
    cmd = ["alembic", "heads"]
    return run_command(cmd, "show migration heads")


def stamp_database(revision: str = "head"):
    """Stamp revision without executing migrations."""
    with _migration_single_flight(f"stamp:{revision}"):
        cmd = ["alembic", "stamp", revision]
        return run_command(cmd, f"stamp database as {revision}")


def auto_migrate():
    """Create and execute an automatic migration."""
    logger.info("=" * 60)
    logger.info("Starting automatic migration flow")
    logger.info("=" * 60)

    if not create_migration("auto_migration"):
        logger.error("Automatic migration failed while creating revision")
        return False

    if not upgrade_database():
        logger.error("Automatic migration failed while upgrading database")
        return False

    logger.info("=" * 60)
    logger.info("Automatic migration flow completed")
    logger.info("=" * 60)
    return True


def init_database():
    """Initialize database for first deployment."""
    logger.info("=" * 60)
    logger.info("Initializing database")
    logger.info("=" * 60)

    if not create_migration("initial_migration"):
        logger.warning("Unable to create initial migration, it may already exist")

    if not upgrade_database():
        logger.error("Database initialization failed")
        return False

    logger.info("=" * 60)
    logger.info("Database initialization completed")
    logger.info("=" * 60)
    return True


def main():
    """CLI entrypoint."""
    if len(sys.argv) < 2:
        print("Usage:")
        print("  python migrate.py create [message]     - create revision")
        print("  python migrate.py upgrade [revision]   - upgrade database (default: head)")
        print("  python migrate.py downgrade [revision] - downgrade database (default: -1)")
        print("  python migrate.py current              - show current revision")
        print("  python migrate.py history              - show migration history")
        print("  python migrate.py heads                - show migration heads")
        print("  python migrate.py stamp [revision]     - stamp revision (default: head)")
        print("  python migrate.py auto                 - auto create + upgrade")
        print("  python migrate.py init                 - initialize database")
        sys.exit(1)

    command = sys.argv[1]

    if command == "create":
        message = sys.argv[2] if len(sys.argv) > 2 else None
        success = create_migration(message)
    elif command == "upgrade":
        revision = sys.argv[2] if len(sys.argv) > 2 else "head"
        success = upgrade_database(revision)
    elif command == "downgrade":
        revision = sys.argv[2] if len(sys.argv) > 2 else "-1"
        success = downgrade_database(revision)
    elif command == "current":
        success = show_current()
    elif command == "history":
        success = show_history()
    elif command == "heads":
        success = show_heads()
    elif command == "stamp":
        revision = sys.argv[2] if len(sys.argv) > 2 else "head"
        success = stamp_database(revision)
    elif command == "auto":
        success = auto_migrate()
    elif command == "init":
        success = init_database()
    else:
        logger.error("Unknown command: %s", command)
        success = False

    sys.exit(0 if success else 1)


if __name__ == "__main__":
    main()
