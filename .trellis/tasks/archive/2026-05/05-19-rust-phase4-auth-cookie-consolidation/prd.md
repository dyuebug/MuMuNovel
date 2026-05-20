# Rust Phase 4 auth cookie consolidation

## Goal

Implement the auth cookie consolidation wave of Rust Phase 4 so cookie-writing
policy is owned by one explicit boundary instead of multiple hand-built string
formatters.

## Requirements

- Scope this task to cookie-writing and directly adjacent auth helper logic.
- Base the work on:
  - `backend-rs/src/api/auth.rs`
  - any directly adjacent helper extracted for cookie assembly
- Consolidate shared cookie attribute construction:
  - `Path`
  - `HttpOnly` vs non-`HttpOnly`
  - `SameSite`
  - `Max-Age`
- Keep behavior compatible unless a reviewed policy change is intentional.
- Make future secure/samesite tightening happen in one obvious owner location.
- Do not mix this task with broad OAuth/provider logic changes.

## Acceptance Criteria

- [ ] Cookie-writing logic is consolidated behind one explicit owner boundary.
- [ ] Existing login/OAuth cookie behavior remains compatible unless an
      intentional change is documented.
- [ ] Shared attributes no longer require editing multiple ad hoc formatters.
- [ ] `cargo check --manifest-path "backend-rs/Cargo.toml"` passes.
- [ ] Focused helper tests are added if cookie assembly becomes independently
      testable.

## Constraints

- Do not redesign the full authentication system.
- Do not mix public-path access cleanup into this task unless auth code proves
  they are inseparable.
- Do not mix router/CORS implementation into this task.

## Dependencies

- Parent planning task:
  `05-19-rust-phase4-security-config-hardening`
- Can execute independently from the router/CORS child once startup/config
  policy is stable enough.

## Notes

- Keep `prd.md` focused on requirements, constraints, and acceptance criteria.
- Lightweight tasks can remain PRD-only.
- For complex tasks, add `design.md` and `implement.md` before `task.py start`
  if implementation details need to be persisted first.
