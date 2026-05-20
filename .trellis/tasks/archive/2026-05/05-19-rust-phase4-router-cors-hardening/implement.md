# Implementation Plan

## Execution Rule

Do not start implementation until the planning artifacts in this task are
reviewed and approved.

## Ordered Checklist

1. Re-read this task's `prd.md` and `design.md`.
2. Re-read the Phase 4 parent plan:
   `05-19-rust-phase4-security-config-hardening`.
3. Confirm current router behavior still ignores `cfg.cors_origins`.
4. Keep the first implementation wave router/CORS-only.
5. Add focused tests if origin parsing becomes independently testable.
6. Run `cargo check` and the targeted CORS/router validation set.
7. Stop if the task starts spilling into auth/cookie/public-path redesign.

## Proposed Execution Waves

### Wave 1: Config-to-router ownership connection

Goal:

- connect `CORS_ORIGINS` from config to router behavior in one explicit owner

Candidate scope:

- `backend-rs/src/api/router.rs`
- `backend-rs/src/config.rs`

Primary targets:

- stop ignoring `cfg.cors_origins`
- introduce one router-local application path for CORS policy

Validation:

- `cargo check --manifest-path backend-rs/Cargo.toml`
- focused helper tests if extracted

Stop rule:

- do not change unrelated route composition while wiring CORS ownership

### Wave 2: Explicit origin parsing

Goal:

- parse and apply wildcard vs explicit origins safely

Candidate scope:

- router/config files
- optional helper for parsing origin lists

Primary targets:

- support explicit origin list parsing
- distinguish wildcard policy from explicit allowlist policy
- keep behavior auditable and deterministic

Validation:

- `cargo check --manifest-path backend-rs/Cargo.toml`
- focused tests for parsing if helper logic is extracted

Stop rule:

- do not over-generalize into a large config framework

### Wave 3: Invalid-config and mode behavior hardening

Goal:

- ensure invalid non-development config fails clearly instead of silently
  staying permissive

Candidate scope:

- router/config integration

Primary targets:

- reject invalid non-development origin config
- make any development-broader behavior explicit
- align with the config/bootstrap child if shared mode policy exists

Validation:

- `cargo check --manifest-path backend-rs/Cargo.toml`
- targeted sanity checks for invalid config behavior

Stop rule:

- do not combine with auth cookie or public-path consolidation

## Review Checklist

- Does router runtime behavior now actually depend on `cfg.cors_origins`?
- Is wildcard handling explicit?
- Is explicit-origin handling auditable?
- Can invalid non-development config still silently downgrade to permissive?
- Did the task avoid mixing route-auth visibility concerns into CORS?

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
rg -n "cors_origins|CorsLayer|permissive\\(|very_permissive\\(|allow_origin|allow_methods|allow_headers" backend-rs/src
```

If focused tests are added:

```powershell
cargo test router --manifest-path "backend-rs/Cargo.toml"
```

## Planning Exit Criteria

This task is ready to start when:

- `prd.md` defines scope and constraints
- `design.md` identifies the current mismatch between config and runtime
  ownership
- `implement.md` keeps the first implementation wave narrow and router/CORS
  focused
