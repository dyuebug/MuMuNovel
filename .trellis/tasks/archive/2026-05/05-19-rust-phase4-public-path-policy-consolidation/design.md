# Design

## Scope

This task plans the Phase 4 public-path policy consolidation wave for the Rust
backend.

The goal is not to redesign the full auth architecture. It is to identify the
real owner of "which paths are public" in the current codebase, then define a
safe consolidation path so later implementation can reduce drift without
changing route visibility semantics by accident.

## Current Owner Discovery

### Primary owner

The current public/open route policy is primarily owned by:

- `backend-rs/src/middleware/auth.rs`

Specifically:

- `AuthLayer` applies auth globally from `backend-rs/src/api/router.rs`
- `AuthMiddleware::call()` checks the request path
- `is_public(path: &str)` contains the real allowlist/whitelist of paths that
  bypass auth

Current public policy examples in that function include:

- health/readiness endpoints
- docs/openapi endpoints
- `/assets` static paths
- auth endpoints such as login/callback/register/logout/config
- changelog endpoints

### Secondary related owner

`backend-rs/src/api/router.rs` is related, but it is not the primary owner of
auth bypass policy.

What `router.rs` does today:

- applies `AuthLayer::new(&cfg.jwt_secret)` to the API stack
- defines static serving and SPA fallback behavior
- returns JSON `404` for unmatched `/api/*` paths in the fallback closure

This means:

- open/protected API policy is mostly in `middleware/auth.rs`
- static/SPA fallback behavior is in `router.rs`
- Phase 4 must treat these as adjacent but distinct responsibilities

## Current Risk Shape

### Risk 1: policy drift

`is_public()` is a hand-maintained string whitelist.

Risk:

- new public endpoint additions may forget to update the list
- removed/renamed endpoints may leave stale allowlist entries behind
- auditability depends on reading one long boolean expression

### Risk 2: mixed concerns

`router.rs` owns route composition and static fallback, while `auth.rs`
indirectly owns which routes are exempt from auth.

Risk:

- route visibility policy is not expressed near the route definitions
- auth bypass and static file behavior can be confused during future changes

### Risk 3: unsafe refactor temptation

It would be easy to over-correct by inventing a large policy framework or
rewriting the full middleware/auth stack.

Risk:

- high blast radius
- accidental login breakage
- unexpected public/private route changes

## Target Contract

The first execution wave should achieve this contract:

1. One explicit owner for public-path auth bypass policy

- public/open path matching logic should live behind one auditable boundary
- route protection policy should not remain encoded as a long scattered string
  expression

2. No behavior change by default

- the initial consolidation should preserve which paths are public today
- changes to actual route visibility require explicit review

3. Static fallback stays separate

- SPA fallback and static asset serving remain router concerns
- auth bypass policy remains middleware/auth concern unless a stronger owner is
  intentionally introduced

4. Future additions become easier to review

- adding a new public endpoint should require touching one obvious place
- reviewers should be able to audit public exposure quickly

## Proposed Consolidation Strategy

### Preferred first step

Keep ownership in `middleware/auth.rs`, but replace the current long
`is_public()` boolean chain with a more explicit, auditable structure.

Likely options for implementation:

- a small constant allowlist plus prefix allowlist
- a matcher helper that clearly separates exact paths from prefix-based paths
- optional grouping/comments by route category

This is preferred because:

- it minimizes behavior change
- it keeps owner locality intact
- it avoids pulling route policy into unrelated files prematurely

### Not recommended for first wave

- moving public-path ownership into every route module
- redesigning auth middleware around route metadata
- combining this with cookie or CORS hardening in the same change

Those may be future improvements, but they are too broad for the first Phase 4
consolidation step.

## File-Level Design Boundaries

Primary implementation candidates:

- `backend-rs/src/middleware/auth.rs`

Secondary review candidates:

- `backend-rs/src/api/router.rs`
- auth-related route modules if audit uncovers mismatches between declared
  public behavior and the current allowlist

## Validation Expectations

The execution task should validate:

- `cargo check`
- focused unit tests for path matching behavior if matching logic is extracted
- targeted sanity review of:
  - health endpoints
  - auth login/callback endpoints
  - changelog endpoints
  - static assets prefix handling
  - a protected API route still requiring auth

## Rollout Notes

- The first implementation wave should be described as consolidation, not
  policy tightening.
- If audit reveals obviously unsafe public exposure, that should be raised as a
  separate reviewed change, not silently folded into consolidation.

## Start Gate

Do not start implementation until:

- this planning task is reviewed
- the auth-cookie and router/CORS planning direction is considered stable
- the implementation task explicitly commits to behavior-preserving
  consolidation rather than auth redesign
