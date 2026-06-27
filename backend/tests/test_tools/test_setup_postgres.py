from __future__ import annotations

import importlib.util
from pathlib import Path

import pytest


MODULE_PATH = Path(__file__).resolve().parents[2] / "scripts" / "setup_postgres.py"


def load_setup_postgres_module():
    spec = importlib.util.spec_from_file_location("backend_scripts_setup_postgres", MODULE_PATH)
    if spec is None or spec.loader is None:
        raise RuntimeError("unable to load setup_postgres.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def test_build_rust_migration_executor_command_defaults_to_cargo_run(monkeypatch):
    setup_postgres = load_setup_postgres_module()

    monkeypatch.delenv("RUST_MIGRATION_EXECUTOR_COMMAND", raising=False)

    command = setup_postgres.build_rust_migration_executor_command()

    assert command[:2] == ["cargo", "run"]
    assert command[-1] == "migration-executor"
    assert "backend-rs" in command[3]


def test_build_rust_migration_executor_command_allows_override(monkeypatch):
    setup_postgres = load_setup_postgres_module()

    monkeypatch.setenv("RUST_MIGRATION_EXECUTOR_COMMAND", "/app/server migration-executor")

    assert setup_postgres.build_rust_migration_executor_command() == [
        "/app/server",
        "migration-executor",
    ]


@pytest.mark.asyncio
async def test_initialize_tables_uses_rust_migration_executor(monkeypatch):
    setup_postgres = load_setup_postgres_module()
    captured: dict[str, object] = {}

    class _Result:
        returncode = 0
        stderr = ""

    def fake_run(command, *, capture_output, text, cwd, env):
        captured["command"] = command
        captured["capture_output"] = capture_output
        captured["text"] = text
        captured["cwd"] = cwd
        captured["database_url"] = env["DATABASE_URL"]
        return _Result()

    monkeypatch.setattr(
        setup_postgres,
        "build_rust_migration_executor_command",
        lambda: ["/app/server", "migration-executor"],
    )
    monkeypatch.setattr(setup_postgres.subprocess, "run", fake_run)

    setup = setup_postgres.PostgreSQLSetup(
        host="localhost",
        port=5432,
        db_name="mumuai_novel",
        db_user="mumuai",
        db_password="secret",
    )

    assert await setup.initialize_tables() is True
    assert captured["command"] == ["/app/server", "migration-executor"]
    assert captured["database_url"] == "postgresql://mumuai:secret@localhost:5432/mumuai_novel"
