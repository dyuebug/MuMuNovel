# Design

## Scope

This task plans Rust Strangler Phase 4: security and config hardening.

The purpose is to turn the Phase 4 bullets from
`docs/architecture/rust-strangler-refactor-plan-2026-05-17.zh-CN.md`
into a concrete execution design for the current repository state, without
mixing implementation into this planning task.

The next execution wave should be a dedicated Phase 4 task, separate from the
current Phase 3 chapter-generation follow-up.

## Why Phase 4 Starts Now

The active follow-up task has already pushed Phase 3 close to its practical
stop point:

- the highest-signal chapter-domain seam work is done
- remaining safe work is mostly alias / wrapper cleanup
- further meaningful Phase 3 work would require re-entering risky
  batch-generation runtime/status semantics

That means the next high-value planning step is no longer "find one more Phase
3 alias to delete", but "prepare the next execution phase with clear ownership
and rollout controls".

## Phase 4 Work Categories

### 1. JWT secret hardening

Current code anchor:

- `backend-rs/src/config.rs`
- `backend-rs/src/api/auth.rs`
- `backend-rs/src/api/router.rs`

Current state:

- `config.rs` generates a random JWT secret when `JWT_SECRET` is missing
- auth/session behavior depends on that value for token and OAuth-state
  signing

Risk:

- non-development deployments can silently boot with a random secret
- restart invalidates tokens unpredictably
- auth behavior can look "working" while persistence semantics are broken

Target contract:

- development may keep a controlled fallback if explicitly allowed
- non-development must fail fast when `JWT_SECRET` is empty
- the policy must be visible in startup logs and testable in isolation

### 2. CORS configuration hardening

Current code anchor:

- `backend-rs/src/api/router.rs`
- `backend-rs/src/config.rs`

Current state:

- `cfg.cors_origins` exists but is not actually used to build the CORS layer
- router currently uses `permissive()` / `very_permissive()`

Risk:

- production-like environments may unintentionally allow overly broad origins
- configuration appears supported but is operationally ignored

Target contract:

- `CORS_ORIGINS` must become the runtime source of truth for non-development
  origin policy
- development ergonomics can remain broader, but only under an explicit mode
- invalid origin config should fail clearly, not silently downgrade to
  permissive behavior

### 3. Cookie writing consolidation

Current code anchor:

- `backend-rs/src/api/auth.rs`

Current state:

- cookie writing is assembled by hand in multiple helper functions
- flags such as `HttpOnly`, `SameSite`, and `Max-Age` are encoded via string
  formatting

Risk:

- policy drift between auth cookies
- future secure/samesite changes require touching multiple ad hoc formatters
- mistakes are easy to introduce and hard to audit

Target contract:

- one local cookie-construction boundary should own shared cookie attributes
- HttpOnly vs non-HttpOnly differences should remain explicit
- future secure-policy changes should happen in one place

### 4. Public-path access policy cleanup

Current code anchor:

- `backend-rs/src/api/router.rs`
- adjacent auth/middleware code discovered during execution

Current state:

- the architecture plan identifies string-whitelist style public-path handling
  as a hardening target
- the execution task must first audit where public/open-path ownership
  currently lives before rewriting it

Risk:

- route visibility can drift as new endpoints are added
- security policy becomes encoded in scattered string checks

Target contract:

- public-path policy should have one explicit owner
- route openness should be auditable without reading unrelated handlers
- the first execution wave may stop at consolidation even if a deeper policy
  abstraction is deferred

### 5. SQLite / development fallback tightening

Current code anchor:

- `backend-rs/src/db/connection.rs`
- `backend-rs/src/config.rs`
- `backend-rs/src/main.rs`

Current state:

- empty `DATABASE_URL` falls back to `sqlite::memory:`

Risk:

- deployment misconfiguration can silently boot against an in-memory database
- health may look green while runtime persistence is effectively broken

Target contract:

- development may retain a deliberate opt-in fallback if needed
- non-development must fail fast on empty database configuration
- startup logs and validation must make the chosen DB mode explicit

## Execution Shape for the Next Task

Phase 4 should not be implemented as one broad "security cleanup" patch.
It should be split into narrow waves:

1. Config and bootstrap guardrails
   - JWT secret policy
   - database fallback policy
   - startup validation / fail-fast behavior

2. Router and CORS hardening
   - `CORS_ORIGINS` parsing and enforcement
   - explicit development vs non-development behavior
   - audit of public/open route policy

3. Auth boundary cleanup
   - cookie builder consolidation
   - secure attribute policy review
   - public-path ownership consolidation if still local to auth/middleware

## Design Principles

1. Fail fast in non-development

- Security-sensitive config should not silently downgrade.

2. Keep development escape hatches explicit

- If a fallback remains for local/dev use, it must be opt-in or clearly
  environment-bounded.

3. Prefer one owner per policy

- one owner for JWT startup policy
- one owner for CORS parsing/application
- one owner for cookie construction
- one owner for public-path access policy

4. Avoid cross-phase mixing

- do not reopen chapter-domain route/workflow refactors under the banner of
  Phase 4
- do not mix schema ownership work back into this phase

## Validation Expectations

The next execution task should validate at multiple levels:

- focused Rust unit tests for config parsing or helper behavior where possible
- `cargo check`
- targeted auth/router tests if those modules have existing coverage points
- explicit startup-failure checks for invalid production-like config

## Rollout and Risk Notes

- JWT hardening can break existing ad hoc local setups if the environment mode
  boundary is chosen badly
- CORS hardening can break desktop/web clients if origin parsing is too strict
  or mismatched with real deployment values
- cookie hardening can affect login persistence and OAuth callback flows
- DB fallback hardening can change boot behavior from "starts unsafely" to
  "fails loudly"; this is intended, but should be introduced with explicit
  diagnostics

## Suggested Start Gate for the Next Task

Do not start Phase 4 implementation until:

- this planning task is reviewed
- the execution order is approved
- the current Phase 3 follow-up is either paused or explicitly capped at any
  remaining high-value safe slice
