# Database Guidelines

> Database patterns and conventions for this project.

---

## Overview

The backend uses SQLAlchemy ORM models under `app/models/`, async sessions
from `backend/tests/test_support/database_test_support.py` for test-only
support, and Alembic-based schema evolution. Query code is
distributed between route files and service/query services, but the project
expects schema changes, default semantics, and backward compatibility to be
treated as high-risk changes.

---

## Query Patterns

- FastAPI routes usually receive `AsyncSession` via `Depends(get_db)`.
- `backend/tests/test_support/database_test_support.py` is the remaining
  Python database/session support boundary for tests; it tracks session
  stats, rolls back open transactions, and always closes the session.
- Keep user isolation explicit. Some tables have a direct `user_id`; others
  are scoped indirectly via `project_id`. Before changing queries, identify
  which isolation path applies.
- Query-heavy logic should move into focused services or query services when
  route code starts repeating.
- Representative files:
  `backend/tests/test_support/database_test_support.py`,
  `backend/app/api/background_tasks.py`,
  `backend/app/services/chapter_crud_query_service.py`.

---

## Migrations

- Schema evolution is managed through Alembic, not runtime schema sync.
- The current repository also has explicit migration orchestration scripts
  under `backend/scripts/`, including deployment-time migration runners.
- If a model field changes, trace all of:
  ORM model, Alembic migration, Pydantic schema, service logic, and tests.
- Treat PostgreSQL and SQLite support as a compatibility concern when touching
  migrations or bootstrap assumptions.
- Do not rely on startup side effects to create or fix tables.

### Convention: Central Alembic Model Registry

**What**: Alembic env files must load models through the centralized
`migrator_app.models.load_all_models()` entrypoint instead of
duplicating manual import lists.

**Why**: The project still keeps a frozen Python migrator metadata package for
schema health and legacy inspection. Centralizing the registry avoids env-file
drift, keeps PostgreSQL and SQLite bootstrap behavior aligned, and makes it
obvious which helper owns metadata population.

**Signature**:

```python
from migrator_app.models import Base, load_all_models

loaded_model_names: tuple[str, ...] = load_all_models()
```

**Contracts**:
- `migrator_app.models` exports the lazy model package, `Base`, and the
  Alembic `load_all_models()` helper for metadata population.
- Call `load_all_models()` before binding `target_metadata = Base.metadata`.
- The helper is expected to import every declared ORM model module so Alembic
  can see the full metadata set.
- The helper is used by `backend/alembic/postgres/env.py`; the legacy SQLite
  Alembic profile has been retired.
- The returned tuple is informational; the important side effect is metadata
  registration on `Base`.
- PostgreSQL Alembic env should resolve `DATABASE_URL` directly from
  environment variables and `.env` loading in the env module itself instead of
  depending on the retired `migrator_app.config` compatibility layer.
- Migration maintenance scripts should prefer standard-library logging over
  importing the retired `migrator_app.logger` compatibility layer.
- `migrator_app.config` and `migrator_app.logger` are retired from the
  migrator package. Test-only rollback adapters may resolve those historical
  module names through `backend/tests/test_support/retired_runtime_test_support.py`, but
  live migration code must not import them.
- The remaining Python database/session helper used by tests lives in
  `backend/tests/test_support/database_test_support.py`; live Rust migration
  contracts should point at that test-support boundary instead of the retired
  runtime database module.

**Validation & Error Matrix**:
- Helper not called before `target_metadata` setup -> Alembic metadata may be
  incomplete and revision health checks can miss tables.
- Helper called multiple times -> safe; repeated imports should not change the
  metadata contract.
- Legacy SQLite env is restored or imported -> reject; the profile is retired.
- Env module forgets to load `DATABASE_URL` locally -> the migration tool can
  fall back to the wrong connection string or import a broader Python config
  package than necessary.

**Good/Base/Bad Cases**:
- Good: one helper populates all migrator models, and both env files call it.
- Base: PostgreSQL env uses the helper; SQLite env keeps the same call pattern
  behind its legacy opt-in gate.
- Bad: each env file hard-codes its own model import list.
- Bad: an env file binds `Base.metadata` before the model registry is loaded.

**Tests Required**:
- Add a focused test that calls `load_all_models()` and asserts representative
  Alembic tables exist in `Base.metadata.tables`.
- Keep Alembic versioning / revision health tests passing after registry
  changes.
- If env loading changes, run the schema migration metadata owner tests plus
  the revision health check.
- Test fixtures that call `Base.metadata.create_all()` should preload the full
  registry through `migrator_app.models.load_all_models()` when they need
  cross-table foreign keys such as `chapters` or `story_memories`.

**Wrong vs Correct**:

#### Wrong

```python
from migrator_app.models import Chapter, Project, User

target_metadata = Base.metadata
```

#### Correct

```python
from migrator_app.models import load_all_models

load_all_models()
target_metadata = Base.metadata
```

### Convention: Retired SQLite Alembic Profile

**What**: The SQLite Alembic profile is retired and no longer kept as a
runtime or inspection entrypoint. The repository should not add new code that
depends on `backend/alembic/sqlite/env.py`, `alembic-sqlite.ini`, or a legacy
SQLite migration gate.

**Why**: The final migration target is Rust-owned PostgreSQL schema
management. Keeping a second Alembic profile around invites drift, duplicate
bootstrap logic, and false confidence that SQLite is still a supported
migration path.

**Contract**:
- PostgreSQL migrations run through Rust `migration-executor` and
  `backend/alembic/postgres/env.py`.
- SQLite migration scripts, if any historical copies remain in the tree, are
  archive-only source-map artifacts and must not be treated as supported
  runtime inputs.
- Tests should not rely on an opt-in SQLite gate to prove active behavior.

**Validation**:
- Search the active backend code for `MUMUNOVEL_ALLOW_LEGACY_SQLITE_ALEMBIC`
  and `backend/alembic/sqlite/env.py`; they should not appear in live
  implementation paths.
- If the repository still contains archived SQLite migration history, it must
  be isolated from the default migration flow and documented as retired.

---

## Naming Conventions

- ORM model files use snake_case and generally map one primary model group per
  file, for example `project.py`, `chapter.py`, `batch_generation_task.py`.
- Table/column naming is persistence-driven and should remain aligned with the
  existing SQLAlchemy/Alembic shape instead of being renamed casually for API
  aesthetics.
- When adding broadly shared models, update `backend/app/models/__init__.py`
  so callers can keep using the centralized import surface.

---

## Common Mistakes

- Adding fields without checking Alembic coverage.
- Silently widening nullability or relying only on Python-side defaults when
  the database semantics matter to shared-runtime behavior.
- Editing task/runtime tables without reviewing recovery, polling, and UI
  consumers.
- Bypassing `get_db()` session handling patterns in normal request code.

## Additional Notes

- Be explicit about default values and nullable behavior. This repository has
  ongoing work around schema/default drift, especially for runtime task tables.
- If a Python-side default encodes business semantics, verify whether the
  database has an equivalent server default. Do not assume ORM defaults alone
  are sufficient.
- Task-oriented tables such as `analysis_tasks`, `batch_generation_tasks`,
  and `regeneration_tasks` require extra care because they are consumed across
  recovery, polling, and UI layers.

## Examples

- Session dependency and rollback/close behavior:
  `backend/tests/test_support/database_test_support.py`
- User-scoped settings lookup and auto-bootstrap:
  `backend/app/api/background_tasks.py`
- Runtime-task persistence model examples:
  `backend/app/models/analysis_task.py`,
  `backend/app/models/batch_generation_task.py`,
  `backend/app/models/regeneration_task.py`

