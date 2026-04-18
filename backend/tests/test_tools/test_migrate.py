from __future__ import annotations

import importlib.util
from pathlib import Path


MODULE_PATH = Path(__file__).resolve().parents[2] / 'scripts' / 'migrate.py'


def load_migrate_module():
    spec = importlib.util.spec_from_file_location('backend_scripts_migrate', MODULE_PATH)
    if spec is None or spec.loader is None:
        raise RuntimeError('unable to load migrate.py')
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def test_build_safe_migration_message_preserves_ascii_tokens():
    migrate = load_migrate_module()

    assert migrate.build_safe_migration_message('添加system_prompt字段到settings表') == 'system_prompt_settings'


def test_build_safe_migration_message_falls_back_to_hashed_slug_for_non_ascii_only():
    migrate = load_migrate_module()

    slug = migrate.build_safe_migration_message('初始化数据库')
    assert slug.startswith('migration_')
    assert len(slug) == len('migration_') + 12


def test_build_safe_migration_message_prefixes_non_alpha_ascii_slug():
    migrate = load_migrate_module()

    assert migrate.build_safe_migration_message('2026 V2 migration') == 'migration_2026_v2_migration'


def test_rewrite_revision_docstring_restores_original_message(tmp_path):
    migrate = load_migrate_module()
    revision_path = tmp_path / '20260418_test_revision.py'
    revision_path.write_text(
        '"""system_prompt_settings\n\nRevision ID: abc123\nRevises: prev123\nCreate Date: 2026-04-18\n\n"""\n',
        encoding='utf-8',
    )

    assert migrate._rewrite_revision_docstring(
        revision_path,
        '添加system_prompt字段到settings表',
        'system_prompt_settings',
    ) is True
    updated = revision_path.read_text(encoding='utf-8')
    assert updated.startswith('"""添加system_prompt字段到settings表\n')


def test_create_migration_uses_safe_slug_and_restores_docstring(monkeypatch, tmp_path):
    migrate = load_migrate_module()
    captured: dict[str, object] = {}
    revision_path = tmp_path / '20260418_system_prompt_settings.py'

    def fake_run_command(cmd, description):
        captured['cmd'] = cmd
        captured['description'] = description
        revision_path.write_text(
            '"""system_prompt_settings\n\nRevision ID: abc123\nRevises: prev123\nCreate Date: 2026-04-18\n\n"""\n',
            encoding='utf-8',
        )
        return True

    monkeypatch.setattr(migrate, 'run_command', fake_run_command)
    monkeypatch.setattr(migrate, '_collect_revision_files', lambda: {revision_path} if revision_path.exists() else set())

    assert migrate.create_migration('添加system_prompt字段到settings表') is True
    assert captured['cmd'] == ['alembic', 'revision', '--autogenerate', '-m', 'system_prompt_settings']
    assert captured['description'] == 'create migration: 添加system_prompt字段到settings表 -> system_prompt_settings'
    assert revision_path.read_text(encoding='utf-8').startswith('"""添加system_prompt字段到settings表\n')