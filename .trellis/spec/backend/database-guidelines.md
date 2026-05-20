# Database Guidelines

> Database patterns and conventions for this project.

---

## Overview

The backend uses SQLAlchemy ORM models under `app/models/`, async sessions
from `app/database.py`, and Alembic-based schema evolution. Query code is
distributed between route files and service/query services, but the project
expects schema changes, default semantics, and backward compatibility to be
treated as high-risk changes.

---

## Query Patterns

- FastAPI routes usually receive `AsyncSession` via `Depends(get_db)`.
- `get_db()` in `backend/app/database.py` enforces authenticated access,
  tracks session stats, rolls back open transactions, and always closes the
  session.
- Keep user isolation explicit. Some tables have a direct `user_id`; others
  are scoped indirectly via `project_id`. Before changing queries, identify
  which isolation path applies.
- Query-heavy logic should move into focused services or query services when
  route code starts repeating.
- Representative files:
  `backend/app/database.py`,
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
  `backend/app/database.py`
- User-scoped settings lookup and auto-bootstrap:
  `backend/app/api/background_tasks.py`
- Runtime-task persistence model examples:
  `backend/app/models/analysis_task.py`,
  `backend/app/models/batch_generation_task.py`,
  `backend/app/models/regeneration_task.py`
