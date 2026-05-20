# Design

## Scope

This task plans the router/CORS hardening wave of Rust Phase 4.

The purpose is to make the declared `CORS_ORIGINS` configuration actually own
runtime CORS behavior in the Rust router, instead of leaving the router on
permissive defaults regardless of configuration.

This task stays narrow:

- router/CORS only
- no cookie refactor
- no public-path policy rewrite
- no JWT/bootstrap fail-fast implementation inside this task unless a tiny
  shared environment-mode helper is unavoidable

## Current State

### Config owner

Current config anchor:

- `backend-rs/src/config.rs`

Current behavior:

- `AppConfig` includes `cors_origins: String`
- `load()` reads `CORS_ORIGINS` with default `"*"`

That means the config surface already claims CORS is configurable.

### Runtime owner

Current runtime anchor:

- `backend-rs/src/api/router.rs`

Current behavior:

- router builds `let cors = if cfg.debug { CorsLayer::permissive() } else { CorsLayer::very_permissive() };`
- `cfg.cors_origins` is not used

This means:

- the runtime source of truth is currently not configuration
- production-like behavior is broader than the config surface suggests

## Main Risk Shape

### Risk 1: configuration drift

The code exposes `CORS_ORIGINS`, but router behavior ignores it.

Risk:

- operators think origin policy is configured when it is not
- review of deployment config gives a false sense of restriction

### Risk 2: over-tightening breakage

A naive switch from permissive to strict parsing can break:

- desktop/local dev flows
- browser clients with slightly mismatched origin formatting
- deployments that currently rely on permissive behavior

### Risk 3: mixed responsibility with other Phase 4 tasks

CORS policy overlaps conceptually with:

- config/bootstrap environment policy
- public/open route behavior

But it should still have one clear execution owner:

- router applies policy
- config provides policy input

## Target Contract

1. `CORS_ORIGINS` becomes the real source of truth for non-development runtime
   policy

- router should derive origin behavior from parsed config

2. Development ergonomics remain explicit

- local/dev behavior may remain broader if necessary
- but that broad behavior must be an explicit mode decision, not an accidental
  default

3. Invalid non-development config fails clearly

- invalid origin strings should not silently downgrade to permissive behavior
- startup or router build should surface the problem

4. Policy ownership remains simple

- config owns raw input
- router owns layer construction
- parsing helper may exist, but should stay local and auditable

## Preferred Execution Shape

### Preferred first step

Introduce a small parsing/application boundary for CORS that:

- parses `cfg.cors_origins`
- distinguishes wildcard from explicit origin lists
- applies explicit behavior for development vs non-development mode

This can remain local to router/config code and does not require a broad new
framework.

### Avoid in first wave

- redesigning all environment-mode handling
- mixing cookie/auth policy changes into the same patch
- conflating CORS policy with public-path auth bypass policy

## File-Level Design Boundaries

Primary implementation candidates:

- `backend-rs/src/api/router.rs`
- `backend-rs/src/config.rs`

Possible adjacent helper target:

- a small local helper for origin parsing / router layer assembly

## Open Design Choice

The implementation task may need a shared notion of development vs
non-development.

Recommended direction:

- reuse or align with the result of the config/bootstrap child if it already
  establishes a mode boundary
- avoid inventing a second, conflicting environment policy locally in router

## Validation Expectations

The execution task should validate:

- `cargo check`
- focused tests if origin parsing/helper logic is extracted
- sanity checks for:
  - wildcard origin behavior
  - explicit origin list behavior
  - invalid origin config handling
  - local/dev path if broader behavior remains

## Rollout Notes

- this wave may intentionally turn some hidden permissive behavior into
  explicit configuration requirements
- diagnostics therefore matter: if config is invalid, the failure should point
  to `CORS_ORIGINS`

## Start Gate

Do not start implementation until:

- this planning task is reviewed
- the config/bootstrap child is at least directionally stable on environment
  mode policy
- the implementation task commits to router/CORS scope only
