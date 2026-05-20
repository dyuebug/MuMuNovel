# Design

## Scope

This task plans the auth cookie consolidation wave of Rust Phase 4.

The goal is not to redesign the full authentication system. It is to make
cookie-writing policy inside the Rust auth API auditable, locally owned, and
less drift-prone.

This task stays narrow:

- cookie-writing logic and directly adjacent auth helper code only
- no OAuth/provider redesign
- no router/CORS implementation
- no public-path policy rewrite

## Current State

### Current owner

The current cookie-writing owner is already relatively local:

- `backend-rs/src/api/auth.rs`

That is good news: the first execution wave does not need a broad ownership
move across modules.

### Current cookie helpers

The file currently has multiple helpers:

- `set_cookie()`
- `set_cookie_with_max_age()`
- `set_cookie_non_httponly()`
- `clear_cookie()`

These helpers are then used across login/logout/oauth/session refresh flows.

### Current drift shape

The helpers already reduce some duplication, but policy is still partly spread
across:

- helper function choice
- hard-coded format strings
- call sites that choose different max-age and HttpOnly combinations

Risk:

- future changes to `SameSite`, `Path`, or secure policy can drift across
  helpers
- policy review requires reasoning across multiple formatting functions
- new cookie cases may copy one existing helper while forgetting another
  attribute choice

## Target Contract

1. One explicit cookie-construction boundary

- shared cookie attribute construction should be owned by one local boundary
- HttpOnly vs non-HttpOnly should remain explicit, not implicit

2. Behavior-preserving first wave

- login, logout, LinuxDo OAuth callback, and refresh/session flows should keep
  their current observable cookie behavior unless an intentional change is
  reviewed

3. Future hardening should become easier

- if Phase 4 later tightens secure attributes, reviewers should only need to
  inspect one obvious owner path

## Preferred Execution Shape

### Preferred first step

Consolidate the cookie string-building policy behind one local builder/helper
shape in `auth.rs`.

Likely form:

- a local cookie spec/option struct or
- one lower-level cookie formatter with explicit flags for:
  - name
  - value
  - max age
  - http_only

This keeps ownership local while reducing policy scattering.

### Keep explicit

The implementation must preserve explicit differences between:

- HttpOnly auth cookies
- non-HttpOnly frontend-visible session timing cookies
- clear-cookie behavior

That means consolidation should not collapse these into a vague "one function
does everything" API that hides security-relevant differences.

## Not Recommended in First Wave

- moving cookie logic out into a broad shared crate/module unless the local
  boundary proves insufficient
- changing OAuth/session semantics while consolidating helpers
- mixing cookie refactor with JWT/bootstrap or CORS changes

## File-Level Design Boundaries

Primary implementation candidate:

- `backend-rs/src/api/auth.rs`

Possible adjacent helper target:

- a very small auth-local helper/module only if it materially improves clarity

## Validation Expectations

The execution task should validate:

- `cargo check`
- focused tests if cookie assembly becomes independently testable
- manual parity review of representative flows:
  - local login
  - logout
  - OAuth callback success path
  - session refresh/first-login marker behavior

## Rollout Notes

- the first implementation wave should be framed as policy consolidation, not
  auth behavior change
- if secure-cookie policy changes are desired later, they should be called out
  explicitly rather than smuggled into consolidation

## Start Gate

Do not start implementation until:

- this planning task is reviewed
- the config/bootstrap child has stabilized overall Phase 4 environment policy
- the implementation task commits to cookie-local consolidation rather than
  auth redesign
