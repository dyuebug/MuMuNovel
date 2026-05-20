# Implementation Plan

## Execution Rule

Do not start implementation until the planning artifacts in this task are
reviewed and approved.

## Ordered Checklist

1. Re-read `prd.md`, `design.md`, and the Phase 4 section of
   `docs/architecture/rust-strangler-refactor-plan-2026-05-17.zh-CN.md`.
2. Confirm the active execution wave and keep it narrow.
3. Load backend specs before editing code.
4. Implement one Phase 4 wave only.
5. Add focused tests where config parsing or helper extraction becomes
   independently testable.
6. Run `cargo check` plus the targeted validation set for the touched area.
7. Reassess whether the next Phase 4 wave should continue in the same task or
   become a follow-up.

## Proposed Execution Waves

### Wave 1: Config/bootstrap fail-fast hardening

Goal:

- stop unsafe non-development fallback behavior at startup

Candidate scope:

- `backend-rs/src/config.rs`
- `backend-rs/src/db/connection.rs`
- `backend-rs/src/main.rs`

Primary targets:

- require `JWT_SECRET` in non-development execution modes
- tighten empty `DATABASE_URL` fallback behavior
- make startup logging explicit about the selected security/config mode

Validation:

- `cargo check --manifest-path backend-rs/Cargo.toml`
- focused config/helper tests if added
- startup/config-path sanity checks for missing secret / empty DB URL behavior

Stop rule:

- do not spill into router/CORS or auth cookie refactors in the same slice

### Wave 2: Router/CORS hardening

Goal:

- make runtime CORS behavior match declared configuration

Candidate scope:

- `backend-rs/src/api/router.rs`
- `backend-rs/src/config.rs`
- any direct helper added for CORS parsing

Primary targets:

- parse and apply `CORS_ORIGINS`
- distinguish development permissive behavior from non-development policy
- fail clearly on invalid non-development CORS config

Validation:

- `cargo check --manifest-path backend-rs/Cargo.toml`
- focused helper tests for origin parsing if extracted
- router behavior sanity review for expected local/dev origins

Stop rule:

- do not mix cookie-writing cleanup into this wave unless router changes
  directly require it

### Wave 3: Auth cookie consolidation

Goal:

- make cookie policy auditable and reduce hand-built attribute drift

Candidate scope:

- `backend-rs/src/api/auth.rs`
- any adjacent auth utility introduced for cookie assembly

Primary targets:

- consolidate cookie string construction behind one local owner
- keep HttpOnly vs non-HttpOnly differences explicit
- prepare for secure-policy tightening without changing unrelated auth flow

Validation:

- `cargo check --manifest-path backend-rs/Cargo.toml`
- focused auth helper tests if extracted
- manual review of `Set-Cookie` attribute parity for existing flows

Stop rule:

- do not combine with broad OAuth/provider behavior changes

### Wave 4: Public-path policy consolidation

Goal:

- remove scattered route-visibility string policy once the real owner is
  identified

Candidate scope:

- auth/middleware/router files discovered during execution

Primary targets:

- identify the current public-path owner
- consolidate string-whitelist checks into one explicit boundary

Validation:

- `cargo check --manifest-path backend-rs/Cargo.toml`
- targeted route/auth sanity checks on open vs protected endpoints

Stop rule:

- do not redesign the full auth architecture if a local consolidation is
  enough for Phase 4

## Code/Review Checklist for the Next Task

- If changing startup validation, did you preserve a deliberate local/dev
  workflow?
- If changing JWT secret handling, did you avoid silent random fallback in
  non-development?
- If changing CORS, did you verify `CORS_ORIGINS` is truly the source of
  truth?
- If changing cookies, did you keep the current login/OAuth flows behaviorally
  compatible unless explicitly intended?
- If tightening DB fallback behavior, did you make failure diagnostics obvious?

## Suggested Validation Commands

Use the shared target dir:

```powershell
$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'
```

Baseline validation:

```powershell
cargo check --manifest-path "backend-rs/Cargo.toml"
```

If auth/router/config helpers gain focused tests:

```powershell
cargo test auth --manifest-path "backend-rs/Cargo.toml"
cargo test router --manifest-path "backend-rs/Cargo.toml"
cargo test config --manifest-path "backend-rs/Cargo.toml"
```

Triage commands during implementation:

```powershell
rustfmt --edition 2021 "<touched-files...>"
git diff --check -- "<touched-files...>"
rg -n "JWT_SECRET|CORS_ORIGINS|sqlite::memory:|Set-Cookie|SameSite|HttpOnly" backend-rs/src
```

## Planning Exit Criteria

This planning task is ready for review when:

- `prd.md` defines the next phase requirements and constraints
- `design.md` maps the work to concrete owners and rollout risks
- `implement.md` splits Phase 4 into narrow execution waves with validation
  and stop rules
