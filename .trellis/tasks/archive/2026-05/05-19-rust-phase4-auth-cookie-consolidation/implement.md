# Implementation Plan

## Execution Rule

Do not start implementation until the planning artifacts in this task are
reviewed and approved.

## Ordered Checklist

1. Re-read this task's `prd.md` and `design.md`.
2. Re-read the Phase 4 parent plan:
   `05-19-rust-phase4-security-config-hardening`.
3. Reconfirm the current cookie-writing helpers and call sites in
   `backend-rs/src/api/auth.rs`.
4. Keep the first implementation wave cookie-local and behavior-preserving.
5. Add focused tests if helper extraction makes cookie assembly independently
   testable.
6. Run `cargo check` and targeted auth/cookie validation.
7. Stop if the task starts expanding into full auth redesign.

## Proposed Execution Waves

### Wave 1: Helper inventory freeze

Goal:

- confirm the current helper/call-site matrix before changing structure

Candidate scope:

- `backend-rs/src/api/auth.rs`

Primary targets:

- document which flows use which helper
- separate HttpOnly, non-HttpOnly, and clear-cookie cases
- identify repeated attribute strings that should be centralized

Validation:

- `cargo check --manifest-path backend-rs/Cargo.toml`
- grep/sanity review of all cookie-writing call sites

Stop rule:

- do not change behavior in the audit-only slice if uncertainty remains

### Wave 2: Local cookie builder consolidation

Goal:

- reduce helper drift while keeping security-relevant differences explicit

Candidate scope:

- `backend-rs/src/api/auth.rs`

Primary targets:

- consolidate shared attribute assembly
- preserve explicit control over HttpOnly vs non-HttpOnly
- preserve clear-cookie semantics

Validation:

- `cargo check --manifest-path backend-rs/Cargo.toml`
- focused helper tests if extracted

Stop rule:

- do not pull in unrelated auth flows or provider logic

### Wave 3: Parity sanity review

Goal:

- verify consolidation preserved externally observable cookie behavior

Candidate scope:

- auth route code
- any focused helper tests added

Primary targets:

- confirm login still sets expected cookies
- confirm logout still clears expected cookies
- confirm OAuth/session helper flows still use the intended cookie variants

Validation:

- `cargo check --manifest-path backend-rs/Cargo.toml`
- targeted auth test filter if available
- manual parity review notes if no focused tests exist

Stop rule:

- do not mix secure-policy tightening into the same slice unless explicitly
  reviewed

## Review Checklist

- Is there now one clearer owner for shared cookie attributes?
- Are HttpOnly vs non-HttpOnly differences still explicit?
- Are clear-cookie semantics still obvious?
- Did any auth route behavior change unintentionally?
- Did the task avoid expanding into broader auth redesign?

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
rg -n "set_cookie\\(|set_cookie_with_max_age\\(|set_cookie_non_httponly\\(|clear_cookie\\(|SET_COOKIE|SameSite|HttpOnly|Max-Age" backend-rs/src/api/auth.rs
```

If focused tests are added:

```powershell
cargo test auth --manifest-path "backend-rs/Cargo.toml"
```

## Planning Exit Criteria

This task is ready to start when:

- `prd.md` defines scope and constraints
- `design.md` identifies the current local owner and consolidation direction
- `implement.md` keeps the first implementation wave behavior-preserving and
  auth-local
