# Directory Structure

> How backend code is organized in this project.

---

## Overview

The backend is a FastAPI application rooted at `backend/app/`, with a
thin-entry pattern at the top and most behavior split into bootstrap, API,
service, ORM, and schema layers.

The real application entrypoint is `backend/app/main.py`, but startup
composition now happens in `backend/app/bootstrap/app_factory.py`. New code
should usually land below `app/`, not in top-level scripts, unless it is
explicitly a migration, operations, or tooling concern.

---

## Directory Layout

```text
backend/
├── app/
│   ├── main.py
│   ├── bootstrap/          # app factory, lifespan, router/static registration
│   ├── api/                # FastAPI routers and request orchestration
│   ├── services/           # business logic, AI orchestration, task runtime
│   ├── models/             # SQLAlchemy ORM models
│   ├── schemas/            # Pydantic request/response contracts
│   ├── middleware/         # auth and request-id middleware
│   ├── mcp/                # MCP integration
│   ├── utils/              # focused helpers shared across modules
│   ├── database.py         # engine/session management and health checks
│   ├── config.py           # settings and environment loading
│   └── logger.py           # unified logging setup
├── alembic/                # PostgreSQL and SQLite migrations
├── scripts/                # migration and operational scripts
├── tests/                  # pytest API / service / schema coverage
├── tools/                  # diagnostics, smoke scripts, support tooling
└── static/                 # built frontend assets served by FastAPI
```

---

## Module Organization

### Entry and bootstrap

- Keep `backend/app/main.py` minimal. It should instantiate the app and keep
  local `uvicorn.run(...)` support only.
- Put startup composition, global exception handlers, middleware wiring,
  health routes, router registration, and static mounting in
  `backend/app/bootstrap/`.
- Example: `backend/app/main.py`, `backend/app/bootstrap/app_factory.py`.

### API layer

- Put HTTP and SSE endpoints in `backend/app/api/`.
- Routers should stay thin: parse request data, resolve auth/access, call
  services, and translate results into response models or streams.
- Shared route helpers belong in route-adjacent helpers such as
  `backend/app/api/common.py` and `backend/app/api/chapter_route_helpers.py`.
- Example: `backend/app/api/background_tasks.py`.

### Service layer

- Put business logic in `backend/app/services/`.
- This repository prefers many narrowly named service files over one giant
  service module. Common suffixes include:
  `*_entry_service.py`, `*_runtime_service.py`, `*_workflow_service.py`,
  `*_query_service.py`, `*_compat_service.py`.
- Before editing, confirm whether a file is the real implementation, a
  facade, or a compatibility shim.
- Example: `backend/app/services/background_task_manager.py`,
  `backend/app/services/chapter_crud_query_service.py`.

### ORM and schema layers

- SQLAlchemy models live in `backend/app/models/`.
- Pydantic request/response contracts live in `backend/app/schemas/`.
- Keep persistence structure and API contract structure separate even when
  they model the same domain.
- Example: `backend/app/models/chapter.py` vs
  `backend/app/schemas/chapter.py`.

### Database, migrations, and tooling

- Database engine/session lifecycle stays in `backend/app/database.py`.
- Schema evolution is handled by Alembic and related scripts under
  `backend/alembic/` and `backend/scripts/`.
- Do not introduce ad hoc startup-time schema mutation in feature code.

### Tests

- Put backend tests under `backend/tests/`.
- Existing suites are organized by concern, especially `test_api/`,
  `test_services/`, and `test_schemas/`.
- Follow that split instead of creating flat, mixed-purpose test files.

---

## Naming Conventions

- Use snake_case for Python files and folders.
- Name API route files by domain or workflow, not by vague HTTP nouns.
  Examples: `settings.py`, `background_tasks.py`,
  `chapter_generation_routes.py`.
- Name service files by responsibility and pipeline stage. If a workflow has
  distinct create/query/runtime/finalize stages, reflect that in filenames.
- Reserve `*_compat_service.py` for compatibility wrappers during gradual
  refactors. Avoid putting fresh core logic there.
- Keep chapter-domain files aligned with the existing split instead of adding
  new logic back into a monolithic `chapters.py`.

---

## Examples

- App composition:
  `backend/app/main.py`, `backend/app/bootstrap/app_factory.py`
- Thin route + service handoff:
  `backend/app/api/background_tasks.py`,
  `backend/app/services/background_task_manager.py`
- ORM / schema separation:
  `backend/app/models/chapter.py`,
  `backend/app/schemas/chapter.py`
- Session and health infrastructure:
  `backend/app/database.py`

## Anti-Patterns

- Do not put substantial business logic directly in route handlers.
- Do not treat facade or compat files as the default place for new behavior.
- Do not add schema-mutation side effects to startup code when the project is
  already using explicit migration flows.
