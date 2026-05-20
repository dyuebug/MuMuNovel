# Rust Phase 4 config bootstrap guardrails

## Goal

Implement the first execution wave of Rust Phase 4 by hardening startup-time
config and bootstrap behavior, so non-development environments can no longer
silently fall back to unsafe defaults such as a random JWT secret or
`sqlite::memory:`.

## Requirements

- Scope this task to startup/config/bootstrap behavior only.
- Base the work on the current Rust code paths:
  - `backend-rs/src/config.rs`
  - `backend-rs/src/db/connection.rs`
  - `backend-rs/src/main.rs`
- Harden `JWT_SECRET` handling:
  - non-development execution must not silently generate a random secret
  - if development fallback remains, it must be explicit and auditable
- Harden empty `DATABASE_URL` handling:
  - non-development execution must not silently fall back to
    `sqlite::memory:`
  - if a local/dev fallback remains, its activation rule must be explicit
- Keep diagnostics clear:
  - startup failure reason must be obvious
  - startup logs must make selected mode/fallback behavior visible
- Do not mix this task with CORS, cookie, or public-path changes.

## Acceptance Criteria

- [ ] `JWT_SECRET` policy is explicit and safe for non-development execution.
- [ ] Empty `DATABASE_URL` no longer silently produces unsafe deployment
      behavior in non-development execution.
- [ ] Any remaining development fallback behavior is deliberate and clearly
      bounded.
- [ ] `cargo check --manifest-path "backend-rs/Cargo.toml"` passes.
- [ ] Focused tests are added if config parsing or helper extraction becomes
      independently testable.

## Constraints

- Do not change auth route behavior beyond what startup/config hardening
  requires.
- Do not implement router/CORS logic in this task.
- Do not reopen Phase 2 schema ownership work.

## Dependencies

- Parent planning task:
  `05-19-rust-phase4-security-config-hardening`
- This child should execute before the router/CORS child if both depend on a
  shared notion of development vs non-development mode.

## Notes

- Keep `prd.md` focused on requirements, constraints, and acceptance criteria.
- Lightweight tasks can remain PRD-only.
- For complex tasks, add `design.md` and `implement.md` before `task.py start`
  if implementation details need to be persisted first.
