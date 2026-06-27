# Directory Structure

> How backend code is organized in this project.

---

## Overview

The production backend runtime is now the Rust service under `backend-rs/`,
served through the strangler Nginx gateway on port `8005`.

The retired Python FastAPI tree under `backend/app/` is no longer a production
entrypoint. Any remaining Python runtime references must be treated as one of:
test support, migrator/schema metadata support, historical source maps, or
explicit rollback documentation. Do not introduce new production behavior under
`backend/app/`, and do not use `uvicorn app.main:app` as a current deploy
instruction.

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

### Production runtime and bootstrap

- Current production runtime lives in `backend-rs/` and is exposed by
  `docker-compose.strangler.yml` / `docker-compose.yml` through
  `rust-backend` and `nginx`.
- Current deployment and smoke validation should target
  `http://localhost:8005`.
- Do not restore `python-backend`, `mumunovel-python`, or
  `uvicorn app.main:app` as production entrypoints unless the same change also
  rolls back the Rust gateway and documents the rollback boundary.

### Retired Python runtime boundaries

- `backend/tests/test_support/app_runtime/` may provide a test-only FastAPI app
  for legacy API/support tests.
- `backend/migrator_app/`, `backend/scripts/`, and `backend/tools/` may keep
  Python code for Alembic/schema migration and diagnostics.
- Tests may keep negative assertions for deleted `app.*` modules. These
  assertions prove production Python modules are not importable; they are not
  active runtime owners.

### Rust API layer

- Put active HTTP and SSE endpoints under `backend-rs/src/api/`.
- Route handlers should stay thin: parse request data, resolve auth/access,
  call services, and translate results into response models or streams.
- Keep retired Python route files as historical/source-map references only;
  do not add fresh production behavior there.

### Rust service layer

- Put active business logic under `backend-rs/src/services/`.
- This repository prefers focused service owners over catch-all modules.
  Common Rust owner boundaries include route owners, runtime-state owners,
  task payload owners, query/read-context owners, and workflow owners.
- Before editing a Python service reference, confirm whether it is a deleted
  production source map, test fixture, migrator support, or historical
  rollback note.

### ORM and schema layers

- Active runtime models and DTOs should be owned from `backend-rs/`.
- Python SQLAlchemy/Pydantic files may remain only as migrator/test-support
  fixtures or historical source maps.
- Keep persistence structure and API contract structure separate even when
  they model the same domain.

### Database, migrations, and tooling

- Runtime database access belongs to Rust service owners.
- Schema evolution is handled by Alembic and related migrator support under
  `backend/alembic/`, `backend/migrator_app/`, and `backend/scripts/`.
- Do not introduce ad hoc startup-time schema mutation in feature code.

### Tests

- Put backend tests under `backend/tests/`.
- Existing suites are organized by concern, especially `test_api/`,
  `test_services/`, and `test_schemas/`.
- Follow that split instead of creating flat, mixed-purpose test files.
- Use `backend/tests/test_support/` for retired Python runtime shims and
  fixtures when a legacy test still needs them.

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
