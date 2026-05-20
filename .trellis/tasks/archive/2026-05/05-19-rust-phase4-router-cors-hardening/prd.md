# Rust Phase 4 router cors hardening

## Goal

Implement the router/CORS hardening wave of Rust Phase 4 so the declared
`CORS_ORIGINS` configuration actually controls runtime CORS behavior instead of
being ignored behind permissive defaults.

## Requirements

- Scope this task to router/CORS behavior only.
- Base the work on:
  - `backend-rs/src/api/router.rs`
  - `backend-rs/src/config.rs`
  - any direct helper extracted for origin parsing/application
- Make `CORS_ORIGINS` a real runtime input for non-development behavior.
- Distinguish development ergonomics from non-development enforcement clearly.
- Invalid non-development CORS configuration must fail clearly instead of
  silently downgrading to permissive behavior.
- Do not mix this task with cookie consolidation unless CORS implementation
  directly forces a shared boundary.

## Acceptance Criteria

- [ ] Router CORS behavior is driven by `CORS_ORIGINS` in non-development
      execution.
- [ ] Development behavior, if broader, is explicit and bounded.
- [ ] Invalid origin config produces visible failure or diagnostics.
- [ ] `cargo check --manifest-path "backend-rs/Cargo.toml"` passes.
- [ ] Focused helper tests are added if origin parsing logic is extracted.

## Constraints

- Do not redesign the whole auth/public access model in this task.
- Do not mix JWT/bootstrap hardening into this task unless a tiny shared config
  helper is unavoidable.
- Do not change unrelated route ownership.

## Dependencies

- Parent planning task:
  `05-19-rust-phase4-security-config-hardening`
- Prefer executing after or alongside the config/bootstrap child if both share
  environment-mode policy.

## Notes

- Keep `prd.md` focused on requirements, constraints, and acceptance criteria.
- Lightweight tasks can remain PRD-only.
- For complex tasks, add `design.md` and `implement.md` before `task.py start`
  if implementation details need to be persisted first.
