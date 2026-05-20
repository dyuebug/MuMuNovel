# Rust Phase 4 public path policy consolidation

## Goal

Implement the public-path policy consolidation wave of Rust Phase 4 so route
openness/protection is owned by one auditable boundary instead of scattered
string-whitelist checks.

## Requirements

- First identify the actual current owner of public/open route policy in the
  Rust backend.
- Scope the first execution wave to consolidation, not a full auth
  architecture rewrite.
- Base the work on the real files discovered during execution, expected to
  include:
  - `backend-rs/src/api/router.rs`
  - auth/middleware files adjacent to route protection
- Consolidate scattered string-whitelist logic into one explicit owner.
- Preserve current route visibility behavior unless an intentional change is
  reviewed and documented.
- Do not mix this task with unrelated chapter-domain refactors.

## Acceptance Criteria

- [ ] The current public/open route policy owner is explicitly identified.
- [ ] Scattered whitelist/string checks are consolidated or clearly reduced.
- [ ] Route openness remains auditable from one owner boundary.
- [ ] `cargo check --manifest-path "backend-rs/Cargo.toml"` passes.
- [ ] Targeted route/auth sanity checks are run for open vs protected paths.

## Constraints

- Do not redesign all middleware/auth layers unless a local consolidation is
  impossible.
- Do not mix broad cookie behavior changes into this task unless the ownership
  boundary is the same.
- Do not introduce new public endpoints as part of this work.

## Dependencies

- Parent planning task:
  `05-19-rust-phase4-security-config-hardening`
- Best executed after the router/CORS and auth-cookie children have clarified
  their respective ownership boundaries, unless audit shows it can proceed
  independently.

## Notes

- Keep `prd.md` focused on requirements, constraints, and acceptance criteria.
- Lightweight tasks can remain PRD-only.
- For complex tasks, add `design.md` and `implement.md` before `task.py start`
  if implementation details need to be persisted first.
