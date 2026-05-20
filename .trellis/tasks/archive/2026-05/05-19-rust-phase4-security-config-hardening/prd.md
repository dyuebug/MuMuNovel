# Plan Rust Phase 4 security and config hardening

## Goal

Create the planning artifacts for Rust Strangler Phase 4 so the next execution
wave can harden security- and config-sensitive runtime behavior without mixing
that work into the current Phase 3 chapter-domain seam follow-up.

This task does not implement Phase 4 changes yet. It establishes the concrete
requirements, execution boundaries, and validation plan for the next task.

## Requirements

- Re-anchor the next Rust refactor wave to
  `docs/architecture/rust-strangler-refactor-plan-2026-05-17.zh-CN.md`
  Phase 4: security and config hardening.
- Translate the Phase 4 bullets into concrete code-owned work items for the
  current repository state.
- Base the plan on the real Rust code paths that currently own these concerns:
  - `backend-rs/src/config.rs`
  - `backend-rs/src/api/router.rs`
  - `backend-rs/src/api/auth.rs`
  - `backend-rs/src/db/connection.rs`
  - any directly adjacent bootstrap or middleware files discovered while
    planning
- Define what should be grouped together and what should remain separate in the
  execution phase, so the next implementation task does not mix security,
  router, auth, and dev-fallback changes in one risky batch.
- Preserve strangler compatibility and avoid accidental production breakage:
  - no silent auth lockout
  - no unexpected CORS deny-all
  - no deploy-time fallback to random secrets or in-memory SQLite in
    non-development environments
- Explicitly document rollout and failure modes for each hardening category.
- Keep the plan implementation-oriented: PRD, design, and implement should be
  detailed enough that the next task can start from them instead of rediscovering
  scope.

## Acceptance Criteria

- [ ] `prd.md` clearly defines the Phase 4 scope, constraints, and success
      conditions for the next execution wave.
- [ ] `design.md` identifies the concrete code boundaries, contracts, and
      rollout risks for:
      - JWT secret hardening
      - CORS configuration hardening
      - cookie-writing consolidation
      - public-path access policy cleanup
      - SQLite/dev fallback tightening
- [ ] `implement.md` breaks the next execution wave into ordered, low-risk
      slices with validation commands and stop rules.
- [ ] The planning artifacts clearly distinguish "plan now" from "implement
      later", so this task can remain in planning until reviewed.
- [ ] The plan is compatible with the current Phase 3 stop rule: do not keep
      expanding chapter-domain seam cleanup when Phase 4 planning is the
      higher-value next step.

## Constraints

- Do not implement the Phase 4 code changes in this task.
- Do not rewrite the global strangler architecture; keep the scope to Rust
  backend security/config hardening.
- Do not assume the Python backend is gone; the plan must still work in the
  current shared-db strangler state.
- Do not require frontend contract changes as part of the initial Phase 4
  execution wave unless later research proves they are unavoidable.

## Out Of Scope

- Continuing low-value Phase 3 alias/wrapper cleanup as a substitute for a new
  execution phase.
- Schema ownership or migration design changes that belong to Phase 2.
- Python API parity work that belongs to Phase 5.
- Immediate nginx or deploy pipeline redesign beyond what Phase 4 needs to
  validate Rust runtime behavior.

## Open Questions

- Should non-development detection rely on `DEBUG`, a dedicated environment
  mode variable, or both?
- Which current auth/public paths should remain open once public-path policy
  stops being hand-maintained as a scattered string whitelist?
- Should cookie hardening be done with one local helper inside `auth.rs` first,
  or should it immediately become a reusable middleware/shared utility?
- How strict can the first CORS hardening wave be without breaking existing
  desktop/web deployment flows?

## Notes

- Keep `prd.md` focused on requirements, constraints, and acceptance criteria.
- Lightweight tasks can remain PRD-only.
- For complex tasks, add `design.md` for technical design and `implement.md` for execution planning before `task.py start`.
