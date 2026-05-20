# Design

## Scope

This task plans the first execution wave of Rust Phase 4: config/bootstrap
guardrails.

The purpose is to harden startup-time configuration behavior so
non-development executions cannot silently fall back to unsafe defaults.

This task is intentionally narrow:

- startup/config/bootstrap only
- no CORS implementation
- no cookie consolidation
- no public-path policy consolidation

## Current State

### JWT secret fallback

Current owner:

- `backend-rs/src/config.rs`

Current behavior:

- `JWT_SECRET` is loaded via `env_or("JWT_SECRET", "")`
- if empty, Rust generates a random UUID-derived secret
- startup logs a warning, then continues

Why this is risky:

- a non-development deployment can start "successfully" with an ephemeral
  secret
- token validation and OAuth state signing become restart-unstable
- auth breakage appears later, not at startup

### Database URL fallback

Current owner:

- `backend-rs/src/db/connection.rs`

Current behavior:

- empty `cfg.database_url` falls back to `"sqlite::memory:"`

Why this is risky:

- a misconfigured non-development deployment can boot against an in-memory DB
- health may look green while all persistence is effectively non-durable
- this hides deployment/configuration failure instead of surfacing it

### Bootstrap integration

Current owner:

- `backend-rs/src/main.rs`

Current behavior:

- loads config
- initializes DB pool
- continues startup after logging warnings

This means the bootstrap path is the right place to enforce fail-fast policy,
but the policy definition itself should remain owned by config/bootstrap helper
code rather than scattered inline through `main.rs`.

## Target Contract

### 1. Non-development must fail fast

For non-development execution modes:

- empty `JWT_SECRET` must be a startup error
- empty `DATABASE_URL` must be a startup error
- startup should terminate with an explicit message

### 2. Development fallback, if retained, must be explicit

If the repository keeps local/dev convenience behavior:

- it must be gated by an explicit development-mode decision
- the selected fallback must be obvious from logs
- the logic should not silently activate in production-like deployments

### 3. One owner per bootstrap policy

The execution wave should converge on:

- one owner for environment-mode classification
- one owner for JWT secret validation/fallback policy
- one owner for DB URL validation/fallback policy

The goal is to avoid re-encoding the same policy separately in
`config.rs`, `connection.rs`, and `main.rs`.

## Key Open Design Choice

The first implementation task must decide how to classify development vs
non-development.

Candidate options:

1. Reuse `DEBUG`

- pros: already exists in `AppConfig`
- cons: debug-ness and environment trust policy are not exactly the same thing

2. Introduce an explicit environment/mode variable

- pros: clearer semantics
- cons: broader config surface area and rollout change

3. Hybrid policy

- existing `DEBUG` for immediate compatibility
- optional future dedicated mode variable if Phase 4 later needs stronger
  distinctions

Recommended initial direction:

- start with the narrowest compatible policy that can be implemented and
  tested quickly
- but structure the code so the "what mode are we in?" decision is centralized

## Preferred Execution Shape

### Preferred first step

Introduce a small config/bootstrap validation boundary that:

- evaluates environment mode once
- validates `JWT_SECRET`
- validates `DATABASE_URL`
- returns explicit startup errors instead of silent unsafe fallbacks

This can remain local to config/bootstrap code and does not need a broad new
framework.

### Avoid in first wave

- introducing a large environment abstraction
- rewriting all config loading semantics
- coupling this work to router/auth modules

## File-Level Design Boundaries

Primary implementation candidates:

- `backend-rs/src/config.rs`
- `backend-rs/src/db/connection.rs`
- `backend-rs/src/main.rs`

Possible adjacent helper target:

- a new local config/bootstrap helper module if it reduces duplication cleanly

## Validation Expectations

The execution task should validate:

- `cargo check`
- focused tests for any extracted config/bootstrap helpers
- explicit startup-path checks for:
  - missing `JWT_SECRET`
  - missing `DATABASE_URL`
  - allowed local/dev fallback path, if retained

## Rollout Notes

- this wave intentionally changes some startup outcomes from "warn and keep
  going" to "fail loudly"
- that is the desired effect in non-development
- logs and failure messages therefore matter as much as code correctness

## Start Gate

Do not start implementation until:

- this planning task is reviewed
- the parent Phase 4 planning task remains the source of truth for ordering
- the implementation task agrees not to mix in CORS, cookie, or public-path
  changes
