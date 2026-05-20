# Implementation Plan

## Execution Rule

Do not start implementation until the planning artifacts in this task are
reviewed and approved.

## Ordered Checklist

1. Re-read this task's `prd.md` and `design.md`.
2. Re-read the Phase 4 parent plan:
   `05-19-rust-phase4-security-config-hardening`.
3. Confirm the current owner is still `backend-rs/src/middleware/auth.rs`.
4. Keep the first change behavior-preserving.
5. Add focused tests if path matching is extracted into a helper.
6. Run `cargo check` and targeted auth/path sanity validation.
7. Stop if the task starts turning into auth architecture redesign.

## Proposed Execution Waves

### Wave 1: Owner audit freeze

Goal:

- confirm and document the current public-path owner before any code move

Candidate scope:

- `backend-rs/src/middleware/auth.rs`
- `backend-rs/src/api/router.rs`

Primary targets:

- verify all currently public paths come from the auth middleware boundary
- distinguish exact-path rules from prefix-path rules
- record any mismatch discovered between route expectations and whitelist

Validation:

- `cargo check --manifest-path backend-rs/Cargo.toml`
- grep/sanity review of all public path checks

Stop rule:

- do not change behavior in the audit-only slice if uncertainty remains

### Wave 2: Matcher consolidation

Goal:

- replace the long inline boolean expression with one explicit matcher shape

Candidate scope:

- `backend-rs/src/middleware/auth.rs`

Primary targets:

- convert `is_public()` into an auditable structure
- clearly separate exact matches from prefix matches
- keep route visibility behavior unchanged

Validation:

- `cargo check --manifest-path backend-rs/Cargo.toml`
- focused unit tests for public/protected path classification if helper logic
  is extracted

Stop rule:

- do not introduce new route metadata systems or broad abstractions

### Wave 3: Protected/public sanity review

Goal:

- verify consolidation did not accidentally widen or narrow access

Candidate scope:

- auth middleware
- route-level sanity probes if needed

Primary targets:

- confirm health endpoints stay public
- confirm auth endpoints stay public
- confirm `/assets` prefix remains public
- confirm at least one protected API route still requires auth

Validation:

- `cargo check --manifest-path backend-rs/Cargo.toml`
- targeted tests or manual sanity review notes for representative paths

Stop rule:

- do not combine with cookie-policy or CORS-policy implementation

## Review Checklist

- Did the owner remain explicit and local?
- Did exact-path vs prefix-path behavior stay readable?
- Did any route visibility behavior change unintentionally?
- Is SPA/static fallback still clearly separated from auth bypass policy?
- Did the change avoid mixing auth redesign into a consolidation task?

## Suggested Validation Commands

Use the shared target dir:

```powershell
$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'
```

Baseline:

```powershell
cargo check --manifest-path "backend-rs/Cargo.toml"
```

Triage:

```powershell
rustfmt --edition 2021 "<touched-files...>"
git diff --check -- "<touched-files...>"
rg -n "is_public|starts_with\\(|/api/auth|/assets|/health|/readyz|/livez" backend-rs/src
```

If focused tests are added:

```powershell
cargo test auth --manifest-path "backend-rs/Cargo.toml"
```

## Planning Exit Criteria

This task is ready to start when:

- `prd.md` defines scope and constraints
- `design.md` identifies the real owner and the preferred consolidation
  approach
- `implement.md` keeps the first implementation wave narrow and
  behavior-preserving
