# Quality Guidelines

> Code quality expectations for backend changes.

---

## Overview

Backend quality in this repository is driven by three themes:

- keep route handlers thin and move behavior into services
- preserve compatibility across shared runtime flows, especially tasks and
  generation pipelines
- verify schema/API changes with pytest and cross-layer review

The codebase contains both active refactors and compatibility layers, so
"working code" is not enough; changes must also land in the correct layer.

---

## Design Expectations

- Prefer focused modules over giant catch-all services.
- When touching generation, regeneration, analysis, or batch workflows, trace
  the whole pipeline before editing one file in isolation.
- Separate persistence concerns (`models/`) from API contracts (`schemas/`)
  and from behavior (`services/`).
- Keep bootstrap, runtime services, compat wrappers, and HTTP routes in their
  own lanes.

## Scenario: Rust chapter batch generation route seam

### 1. Scope / Trigger
- Trigger: a change touches `backend-rs/src/api/chapter_batch_generation.rs`
  or its service-owned task/query/stream/runtime helpers.
- Why this needs code-spec depth: the route group spans HTTP, SSE, task
  persistence, resume/recover semantics, and compatibility response shapes.

### 2. Signatures
- Route entrypoints may accept HTTP request payloads or SSE stream requests,
  but must delegate business execution to service helpers.
- Service signatures should be grouped by responsibility:
  `create plan`, `workflow`, `query/view context`, `stream builder`,
  `runtime executor`.
- `tokio::spawn` remains a route boundary concern unless the background
  ownership model changes for the whole runtime.

### 3. Contracts
- Request boundary contract: route parses inputs, performs basic validation,
  resolves access context, and builds `AIConfig`.
- Provider context contract: placeholder/default prompt-context provider payload
  should be assembled once at the request/workflow preparation boundary, then
  passed explicitly through dispatch/runtime calls. Route handlers and stream
  helpers should not recreate the same default payload locally.
- Response boundary contract: route may return compat response envelopes, but
  task status payload assembly belongs to service helpers.
- Stream contract: polling state transitions and SSE event payload shape must
  be owned by shared stream helpers, not rebuilt inline per endpoint.
- Persistence contract: checkpoint updates, snapshot persistence, and runtime
  state advancement belong to service/runtime helpers.

### 4. Validation & Error Matrix
- Invalid request fields -> reject at route boundary before spawning work.
- Access / ownership failure -> reject at route boundary.
- Task orchestration failure -> service returns the domain error; route only
  translates it to transport form.
- Polling / stream state mismatch -> fix in stream helper, not with route-local
  field patches.
- Checkpoint / snapshot write failure -> fail in runtime helper and preserve a
  single error path for task status.

### 5. Good/Base/Bad Cases
- Good: route extracts request context, calls one service helper, and returns
  the compat response or stream. If runtime needs provider payload, it comes
  from a prepared request/workflow result object and is only forwarded here.
- Base: route keeps `tokio::spawn` and transport-specific wiring, while
  service owns the create/query/runtime logic.
- Bad: route mutates task status, builds checkpoint payloads, and assembles SSE
  events inline inside the same handler.
- Bad: route or stream helper calls a local
  `resolve_default_prompt_context_provider_payload()` fallback even though the
  request/workflow preparation step can own that default once.

### 6. Tests Required
- Unit tests for active task/status response builders.
- Unit tests for SSE event builder and stream terminal conditions.
- Targeted route or integration checks that verify delegation still preserves
  request validation and compat response shape.
- Runtime tests for checkpoint/snapshot progression when changing task-flow
  semantics.
- When provider payload ownership moves across boundaries, add or update a
  focused unit test on the new owner object/helper rather than relying on a
  route-level smoke only.

### 7. Wrong vs Correct
#### Wrong
- Route handler parses request, loads task state, mutates checkpoint, formats
  status response, and pushes SSE event payloads inline.
- Route or stream helper recreates placeholder provider payload locally before
  dispatching runtime work.

#### Correct
- Route handler keeps transport concerns only, then delegates create/query/
  stream/runtime behavior to service-owned helpers with focused tests.
- Prepared request/workflow objects own default provider payload assembly once,
  and downstream route/stream/runtime boundaries only pass the explicit
  payload through.

## Scenario: Rust startup and runtime hardening boundary

### 1. Scope / Trigger
- Trigger: a change touches Rust startup/runtime boundary files such as
  `backend-rs/src/config.rs`, `backend-rs/src/main.rs`,
  `backend-rs/src/db/connection.rs`, `backend-rs/src/api/router.rs`,
  `backend-rs/src/api/auth.rs`, or `backend-rs/src/middleware/auth.rs`.
- Why this needs code-spec depth: these files own environment wiring,
  startup failure policy, credentialed browser access, cookie policy, and
  public-vs-protected route boundaries. A small local edit can silently widen
  runtime exposure or make deployment config lie about actual behavior.

### 2. Signatures
- `config::load() -> Result<AppConfig, ConfigError>` is the startup config
  entrypoint and must remain the owner of runtime-mode-sensitive validation.
- `db::connection::connect(cfg: &AppConfig)` consumes a validated
  `cfg.database_url`; it must not invent a second fallback policy.
- `api::router::build(...) -> Result<Router, RouterBuildError>` owns CORS
  layer construction and may fail when `CORS_ORIGINS` is invalid for the
  selected runtime mode.
- Auth cookie writers in `backend-rs/src/api/auth.rs` must route through one
  local cookie builder/helper boundary instead of ad hoc format strings.
- `middleware::auth::is_public(path: &str)` remains the owner of public-path
  auth bypass policy and should be expressed as explicit exact/prefix match
  tables, not a long inline boolean chain.

### 3. Contracts
- Environment keys:
  - `DEBUG=false` means non-development runtime policy.
  - `JWT_SECRET` is required in non-development; development may generate an
    ephemeral local secret with an explicit warning log.
  - `DATABASE_URL` is required in non-development; development may fall back
    to `sqlite::memory:` with an explicit warning log.
  - `CORS_ORIGINS` must be the actual router input. In non-development it must
    be either a comma-separated explicit origin list or startup/router build
    must fail. `*` is development-only.
- CORS contract:
  - credentialed browser flows are supported, so explicit-origin mode must
    keep `allow_credentials(true)`.
  - origin parsing must reject userinfo, path segments, query, fragment, and
    malformed absolute origins.
- Cookie contract:
  - shared attributes (`Path`, `SameSite`, `Max-Age`) come from one builder.
  - `HttpOnly` vs non-`HttpOnly` stays explicit at the call boundary.
- Public path contract:
  - health/docs/auth bootstrap endpoints and static asset prefixes may stay
    public only through the middleware owner boundary.
  - route composition in `router.rs` must not silently change module exposure
    while refactoring CORS or startup behavior.

### 4. Validation & Error Matrix
- Non-development + empty `JWT_SECRET` -> `ConfigError::MissingJwtSecret` and
  process exits during startup.
- Non-development + empty `DATABASE_URL` -> `ConfigError::MissingDatabaseUrl`
  and process exits during startup.
- Non-development + `CORS_ORIGINS="*"` -> `RouterBuildError::WildcardCorsOriginsNotAllowed`
  and process exits during router build.
- `CORS_ORIGINS` contains malformed or non-origin values -> `RouterBuildError::InvalidCorsOrigin`.
- Editing router composition and dropping an existing route merge -> treat as a
  behavioral regression even if `cargo check` still passes; restore the route
  and re-run focused validation.

### 5. Good/Base/Bad Cases
- Good: runtime mode is decided once in config loading, startup errors fail
  fast before serving traffic, router CORS behavior matches `CORS_ORIGINS`,
  cookie formatting flows through one helper, and public paths are auditable
  from one matcher table.
- Base: development keeps explicit convenience fallbacks for local bootstrap,
  but warnings make the fallback visible and non-development never reuses the
  same implicit behavior.
- Bad: `db::connection` or `router.rs` adds its own hidden fallback after
  config validation already ran.
- Bad: router refactor changes the `.merge(...)` chain and silently drops an
  existing route group while focusing on unrelated runtime hardening.
- Bad: a new auth cookie path reintroduces hand-built `Set-Cookie` strings
  outside the local cookie builder.

### 6. Tests Required
- Unit tests for runtime-mode-sensitive config helpers:
  - development allows ephemeral JWT / in-memory DB fallback
  - non-development rejects missing JWT / DB URL
- Unit tests for CORS parsing:
  - development wildcard allowed
  - non-development wildcard rejected
  - explicit origins normalized/deduplicated
  - path-bearing origins rejected
- Unit tests for cookie rendering:
  - `HttpOnly` cookie shape
  - frontend-visible cookie shape
  - clear-cookie shape
- Unit tests for public/protected path classification:
  - representative exact public paths
  - `/assets` prefix
  - representative protected API paths
- After router composition edits, run at least one targeted review against the
  `.merge(...)` list so route groups were not dropped accidentally.

### 7. Wrong vs Correct
#### Wrong
- `config::load()` silently generates a production secret or leaves
  `DATABASE_URL` empty and lets downstream code guess.
- `router.rs` ignores `cfg.cors_origins`, uses a permissive default for every
  mode, and accidentally removes an unrelated route merge in the same patch.
- Auth handlers build `Set-Cookie` strings in multiple helper variants with
  duplicated `Path` / `SameSite` fragments.
- Public route policy is encoded as an ever-growing inline boolean expression
  with no tests for representative paths.

#### Correct
- `config::load()` centralizes runtime mode classification and returns typed
  startup errors for non-development misconfiguration.
- `router.rs` builds CORS from validated config, preserves credential support,
  and keeps the route merge surface intact during refactors.
- Auth handlers use one cookie builder/helper boundary with explicit
  `HttpOnly` control.
- Public route policy stays local to `middleware/auth.rs` and is expressed as
  exact/prefix policy tables with focused tests.

---

## Change Checklist

- If you changed a route payload, did you update the related Pydantic schema,
  frontend consumer, and tests?
- If you changed a model, did you review migration impact, default semantics,
  and recovery/state consumers?
- If you edited a compat or facade file, did you confirm whether the real
  implementation also needs changes?
- If the change affects long-running tasks, did you inspect persistence,
  resume, polling, and SSE consumers?

---

## Testing Expectations

- Run backend pytest when feasible.
- Reuse the existing split:
  - API tests in `backend/tests/test_api/`
  - service tests in `backend/tests/test_services/`
  - schema tests in `backend/tests/test_schemas/`
- Async tests are normal here; `pytest.ini` is configured with
  `asyncio_mode = auto`.
- Prefer adding or updating targeted tests near the affected area instead of
  one oversized integration test when a narrower regression test is enough.

---

## Common Mistakes

- Editing a compat layer without checking the underlying implementation.
- Treating task/runtime tables as simple CRUD state even though UI recovery and
  polling depend on them.
- Changing request/response fields without tracing frontend consumers.
- Assuming `/health` is enough when readiness and DB warmup semantics matter.
- Refactoring `backend-rs/src/api/router.rs` and accidentally dropping an
  existing `.merge(...)` route group while working on unrelated middleware or
  CORS changes.

---

## Examples

- Thin route, rich service split:
  `backend/app/api/background_tasks.py`,
  `backend/app/services/background_task_manager.py`
- Bootstrap-owned exception handling and readiness:
  `backend/app/bootstrap/app_factory.py`
- Targeted API tests:
  `backend/tests/test_api/test_settings.py`

## Forbidden / Discouraged Patterns

- Do not pile new business logic into `app/api/`.
- Do not use startup-time schema mutation as a shortcut around migrations.
- Do not add new functionality to frozen compatibility facades unless the task
  is explicitly transitional.
