# Implementation Plan

## Execution Rule

Do not start implementation until the planning artifacts in this task are
reviewed and approved.

## Ordered Checklist

1. Re-read this task's `prd.md` and `design.md`.
2. Re-read the Phase 4 parent plan:
   `05-19-rust-phase4-security-config-hardening`.
3. Confirm the current startup fallback behavior in:
   - `backend-rs/src/config.rs`
   - `backend-rs/src/db/connection.rs`
   - `backend-rs/src/main.rs`
4. Implement one bootstrap/config wave only.
5. Add focused tests if validation logic becomes independently testable.
6. Run `cargo check` and the targeted startup/config validation set.
7. Stop if the task starts spilling into router/auth behavior.

## Proposed Execution Waves

### Wave 1: Mode and validation owner consolidation

Goal:

- define one owner boundary for development vs non-development bootstrap policy

Candidate scope:

- `backend-rs/src/config.rs`
- optional adjacent helper module

Primary targets:

- centralize environment-mode decision
- centralize JWT/database fallback policy inputs
- avoid duplicating mode checks across config/bootstrap files

Validation:

- `cargo check --manifest-path backend-rs/Cargo.toml`
- focused helper tests if extracted

Stop rule:

- do not yet change unrelated runtime config parsing

### Wave 2: JWT secret fail-fast hardening

Goal:

- remove silent random-secret fallback in non-development execution

Candidate scope:

- `backend-rs/src/config.rs`
- `backend-rs/src/main.rs`

Primary targets:

- fail explicitly when `JWT_SECRET` is missing in non-development
- keep any dev fallback behavior explicit and logged

Validation:

- `cargo check --manifest-path backend-rs/Cargo.toml`
- targeted startup/config-path checks for missing secret behavior

Stop rule:

- do not combine with cookie-policy cleanup

### Wave 3: Database URL fail-fast hardening

Goal:

- remove silent `sqlite::memory:` deployment fallback in non-development

Candidate scope:

- `backend-rs/src/db/connection.rs`
- `backend-rs/src/config.rs`
- `backend-rs/src/main.rs`

Primary targets:

- fail explicitly when `DATABASE_URL` is empty in non-development
- keep any local/dev fallback explicit if retained
- make logs/diagnostics identify chosen DB mode

Validation:

- `cargo check --manifest-path backend-rs/Cargo.toml`
- focused helper tests if extracted
- targeted startup/config-path checks for empty DB URL behavior

Stop rule:

- do not mix with schema/migration ownership changes

## Review Checklist

- Is the mode decision centralized instead of scattered?
- Can non-development still boot unsafely with a random JWT secret?
- Can non-development still boot unsafely with `sqlite::memory:`?
- Are startup failures explicit enough for operators?
- Did the task avoid expanding into router/CORS or auth behavior?

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
rg -n "JWT_SECRET|DATABASE_URL|sqlite::memory:|DEBUG|enable_startup_schema_sync" backend-rs/src
```

If focused tests are added:

```powershell
cargo test config --manifest-path "backend-rs/Cargo.toml"
```

## Planning Exit Criteria

This task is ready to start when:

- `prd.md` defines the startup/config scope and constraints
- `design.md` identifies the real fallback owners and the target contract
- `implement.md` keeps execution narrow and bootstrap-focused
