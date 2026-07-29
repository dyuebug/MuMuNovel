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


### Fast redeploy Rust migration contract

#### 1. Scope / Trigger

`redeploy-fast.ps1` is a runtime deployment path, not a code-only container
restart. When it rebuilds `rust-backend`, it MUST run the existing Rust
`db-migrator` one-shot service after PostgreSQL is healthy and before the
runtime container is recreated. This preserves existing database data while
applying pending revisions through the Rust-owned migration executor.

#### 2. Signatures

```powershell
# Required ordering in redeploy-fast.ps1
docker compose -f docker-compose.strangler.yml up -d postgres
docker compose -f docker-compose.strangler.yml run --name mumunovel-db-migrator-once --no-deps -T db-migrator
docker compose -f docker-compose.strangler.yml up -d --no-deps --force-recreate rust-backend
```

The `db-migrator` Compose service invokes `/app/server migration-executor`.
The temporary named container must be removed before and after the run so a
previous interrupted deploy cannot block the next invocation.

#### 3. Contracts

- `postgres` MUST be healthy before invoking `db-migrator`.
- `db-migrator` exit code `0` is required before restarting `rust-backend`.
- `--no-deps` is valid for the one-shot migrator only after the script has
  started and health-checked PostgreSQL itself.
- `/readyz` is the runtime readiness contract: it is `200` only when the live
  Alembic revision matches the Rust catalog head and required storage checks
  allow readiness.
- The deploy smoke command remains owner-scoped and targets
  `http://localhost:8005` with profile `deploy`.

#### 4. Validation & Error Matrix

| Condition | Expected behavior | Required operator evidence |
| --- | --- | --- |
| PostgreSQL is unavailable | Stop before migration | PostgreSQL diagnostics and non-zero deploy exit |
| Migrator exits non-zero | Do not recreate Rust/Nginx | Migrator and PostgreSQL diagnostics; preserve migration output |
| Live revision is behind catalog head | Migrator applies its Rust-owned pending revisions | `/readyz` reports matching `actual_head` and `expected_head` afterward |
| Runtime starts after a successful migration | Continue to gateway smoke | `/readyz` and deploy-profile smoke both return success |
| Smoke fails after migration | Fail deployment; do not report success | `tmp/smoke/tmp_strangler_gateway_smoke_latest.json` and container logs |

#### 5. Good / Base / Bad Cases

- **Good:** an idempotent migrator run at the current head returns `0`; the
  runtime is recreated and deploy smoke passes.
- **Base:** one or more Rust-owned revisions are pending; the migrator advances
  the live revision, then `/readyz` changes from not-ready to ready.
- **Bad:** restarting `rust-backend` with `--no-deps` immediately after only
  starting PostgreSQL. This bypasses the Compose dependency condition and can
  leave `/readyz` at `503` because the database revision is stale.

#### 6. Tests Required

1. Parse `redeploy-fast.ps1` and assert that `Invoke-DatabaseMigrator` is
   called before the Rust runtime recreation command.
2. Run the script against an isolated or approved local Docker database with a
   known stale revision; assert that migration succeeds, `/readyz` is `200`,
   and `actual_head == expected_head`.
3. Run `backend/tools/run_strangler_gateway_smoke.py --profile deploy`; assert
   `ok == true` and an empty `failed_probe_names` list.
4. For a forced migrator failure, assert that the Rust runtime recreate command
   is not reached and migration diagnostics are logged.

#### 7. Wrong vs Correct

```powershell
# Wrong: bypasses db-migrator and can run a stale schema.
docker compose -f docker-compose.strangler.yml up -d --no-deps --force-recreate rust-backend

# Correct: establish the schema contract before the runtime is recreated.
Invoke-DatabaseMigrator
docker compose -f docker-compose.strangler.yml up -d --no-deps --force-recreate rust-backend
```

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

## Bind-Mounted SPA Entrypoint Reload Contract

### 1. Scope / Trigger

This contract applies when the Rust runtime serves the SPA from `STATIC_DIR`
and that directory can change after process startup, including the Compose bind
mount `./backend/static:/app/static:ro`. A frontend production build replaces
hashed assets and rewrites `index.html` without rebuilding the running
container.

### 2. Signatures

```rust
async fn read_static_index(index_path: &Path) -> String;
```

```text
STATIC_DIR=/app/static
GET /assets/<hashed-file>  -> ServeDir rooted at STATIC_DIR/assets
GET /<spa-route>           -> current STATIC_DIR/index.html
GET /api/<unknown-route>   -> 404 JSON, never SPA HTML
```

### 3. Contracts

- The SPA fallback MUST read the current `index.html` when it serves a document
  request; it MUST NOT retain the startup copy for the process lifetime.
- Hashed assets remain file-served from `STATIC_DIR/assets`.
- Rebuilding the frontend inside the bind-mounted directory MUST NOT create a
  mixed deployment where HTML references an older entrypoint while assets are
  already from the new build.
- Unknown `/api/` routes retain the JSON `404` behavior.
- A missing or unreadable entrypoint preserves the existing empty-body fallback
  behavior; this change does not invent a new public error response.

### 4. Validation & Error Matrix

| Condition | Required behavior |
| --- | --- |
| `index.html` is replaced after Rust startup | Next SPA document response contains the new hashed entrypoint |
| Requested hashed asset exists | Serve the exact asset with the detected MIME type |
| Unknown path starts with `api/` | Return `404` JSON instead of `index.html` |
| `index.html` becomes unreadable | Return the existing empty fallback body; do not serve a stale cached entrypoint |

### 5. Good / Base / Bad Cases

- Good: build v2 rewrites `index.html`; the next browser refresh receives the
  v2 entrypoint without restarting Rust.
- Base: no frontend rebuild occurs; repeated SPA requests return the same file
  contents with only one small asynchronous file read per document request.
- Bad: Rust clones the startup `index.html` into the fallback closure while
  `/assets` reads the bind mount live, producing old HTML plus new assets.

### 6. Tests Required

- A focused async test writes `old-entrypoint`, reads it, overwrites the same
  file with `new-entrypoint`, and asserts the next read returns the new value.
- `cargo fmt --check` and `cargo check --tests` must pass.
- The Novel Autopilot Workbench E2E must continue to pass because its lazy
  chunk is one of the hashed assets selected by the SPA entrypoint.
- Runtime verification should compare the entrypoint referenced by HTTP `/`
  with the entrypoint in the mounted `/app/static/index.html`.

### 7. Wrong vs Correct

```rust
// Wrong: frontend builds after startup can never update this captured HTML.
let index_html = std::fs::read_to_string(&index_path).unwrap_or_default();
move || index_html.clone()

// Correct: every SPA document fallback observes the current bind-mounted file.
let index_path = index_path.clone();
async move { read_static_index(&index_path).await }
```
