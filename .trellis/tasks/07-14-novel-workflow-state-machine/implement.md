# R3 Implementation Plan

## Preconditions

- [x] G0 is GO and R3 is the next unblocked roadmap item.
- [x] Existing phase/write-entry/backend/frontend evidence has been inspected.
- [x] No blocking product decision remains; user previously authorized direct continuation.
- [x] No database Schema change is required by this implementation plan.
- [x] Current diffs were reviewed before edits; unrelated worktree changes were not overwritten.

## Ordered Implementation

### Phase A — Backend Domain Owner

- [x] Add `novel_workflow_service.rs` with nine-phase enum, aliases, transition table, state view,
  conflict/domain errors and schema version.
- [x] Add exhaustive unit tests for parsing, canonicalization, allowed-transition matrix,
  rollback, same-phase idempotency and suggested next phase.
- [x] Register the service module without changing unrelated service owners.

### Phase B — Persistence and Compatibility Entrypoints

- [x] Implement owned GET and conditional transition persistence using `expected_phase`.
- [x] Route legacy Project PUT status changes through the same owner while preserving metadata-only updates.
- [x] Change project creation defaults from legacy `planning` to canonical `foundation`.
- [x] Route wizard completion and wizard cleanup status writes through internal owner helpers without changing
  wizard/background execution state semantics.
- [x] Normalize project import phase aliases and reject unknown values; route book import `writing` through the
  same owner; keep export transport compatible.
- [x] Add tests proving all write entrypoints produce canonical phases and cannot bypass transition rules.

### Phase C — Rust API

- [x] Add workflow state GET and transition POST DTOs/handlers/routes to the Project API.
- [x] Map not-found, illegal transition, stale expected phase, unknown persisted phase and internal errors.
- [x] Emit bounded structured audit events for real changes.
- [x] Add route/serde/owner contract tests and database-backed transition/concurrency coverage supported by the
  current harness.

### Phase D — Frontend Feature

- [x] Add shared `NovelWorkflowPhase` and workflow API response/request types.
- [x] Add project API methods for GET state and POST transition.
- [x] Add project workflow hook and presentation component under `features/projects/workflow`.
- [x] Integrate into `ProjectDetail` persistent summary area and synchronize the existing Zustand project cache.
- [x] Ensure conflict refresh, pending disable, high-impact rollback confirmation and responsive layout.
- [x] Remove progress-derived completed display where it conflicts with the authoritative phase, using only
  minimal reviewed hunks in already-dirty list/bookshelf files.

### Phase E — Verification and Documentation

- [x] `cargo fmt --manifest-path backend-rs/Cargo.toml -- --check`
- [x] targeted Rust workflow/project/import tests
- [x] `cargo test --manifest-path backend-rs/Cargo.toml --quiet`
- [x] `cargo check --manifest-path backend-rs/Cargo.toml`
- [x] Execute `cargo clippy --manifest-path backend-rs/Cargo.toml --all-targets --all-features -- -D warnings`;
  the repository-wide command remains non-green because of pre-existing baseline findings, while the R3-owned
  new files have zero hits.
- [x] `npm run build --prefix frontend`
- [x] `npm run lint --prefix frontend`
- [x] focused Playwright API-mock workflow test
- [x] verify scoped `git diff` does not overwrite unrelated changes
- [x] update `implement.md` with evidence and update Roadmap R3/G1 ordering status

## Verification Evidence — 2026-07-14

| Scope | Command | Result |
|---|---|---|
| Rust formatting | `cargo fmt --manifest-path backend-rs/Cargo.toml -- --check` | PASS |
| Workflow owner | `cargo test --manifest-path backend-rs/Cargo.toml novel_workflow_service::tests --quiet` | PASS, 17/17 |
| Project service | `cargo test --manifest-path backend-rs/Cargo.toml services::project_service::tests --quiet` | PASS, 6/6 |
| Project API/import | `cargo test --manifest-path backend-rs/Cargo.toml api::projects --quiet` | PASS, 67/67 |
| Book import | `cargo test --manifest-path backend-rs/Cargo.toml services::book_import_service::tests --quiet` | PASS, 10/10 |
| Rust compile check | `cargo check --manifest-path backend-rs/Cargo.toml` | PASS; four existing dead-code warning groups remain outside R3 |
| Full Rust suite | `cargo test --manifest-path backend-rs/Cargo.toml --quiet` | PASS, 1646/1646 |
| Strict Clippy audit | `cargo clippy --manifest-path backend-rs/Cargo.toml --all-targets --all-features -- -D warnings` | EXECUTED, exit 101; current log contains 197 `error:` lines and reports 191 previous errors for the test target; zero hits in `novel_workflow_service.rs` and `import_workflow_owner.rs` |
| Frontend build | `npm run build --prefix frontend` | PASS, including service-facade and visible-text validation |
| Frontend lint | `npm run lint --prefix frontend` | PASS, 0 errors / 33 existing warnings |
| Workflow E2E | `npm run e2e --prefix frontend -- e2e/project-workflow-state.spec.ts` | PASS, Chromium 1/1 |

Strict Clippy evidence is stored for the current session at
`%TEMP%/mumu-r3-clippy-current.log`. The repository baseline must be handled by a separate scoped cleanup task;
R3 does not add global `#[allow]` suppressions or broaden shared CRUD refactors solely to hide those findings.

## Contract and Scope Decisions

- `projects.status` is the only persisted novel-level workflow phase fact.
- `wizard_status`/`wizard_step`, background `TaskStatus`, word-count metrics and chapter runtime/checkpoints keep
  their existing independent responsibilities.
- No migration, new workflow table or second phase field is introduced.
- Word count never derives or auto-writes the `completed` phase.
- Legacy `planning`, `draft`, `active` and `revising` values are normalized through the owner; unknown persisted
  phases fail explicitly instead of being silently guessed.
- The default SeaORM/mock/SQLite-compatible CAS coverage proves the owner contract in the available harness.
  A separate opt-in real-PostgreSQL concurrency test now reuses the same assertion helper and requires an
  explicitly supplied fresh isolated `MUMU_R3_POSTGRES_URL`; it is intentionally `#[ignore]` in the default suite.
- R3 does not define Story Packet, Generation Intent, role model policy, business checkpoint or Coordinator
  contracts; those remain R4 and later roadmap responsibilities.

## Test Matrix

1. [x] Every canonical phase serializes/deserializes exactly.
2. [x] Legacy `planning/draft/revising/active` normalize deterministically.
3. [x] Unknown input and unknown stored phase fail explicitly.
4. [x] Legal forward and rollback transitions pass; all other pairs fail.
5. [x] Same-phase requests are idempotent and do not change `updated_at`.
6. [x] Missing/foreign project returns 404 without leaking existence.
7. [x] Stale `expected_phase` returns 409 and preserves current phase.
8. [x] Requests with the same expected phase are protected by the conditional-update/CAS owner contract;
   `postgres_concurrent_transitions_with_same_expected_phase_change_at_most_once` additionally verifies the
   same assertion against a fresh isolated PostgreSQL database when explicitly enabled.
9. [x] Legacy PUT cannot write arbitrary status but metadata-only updates still work.
10. [x] Wizard completion/reset, project import and book import produce canonical phases.
11. [x] Background task status changes never mutate `projects.status`.
12. [x] Frontend renders server phase/allowed transitions and refreshes on conflict.
13. [x] A 100% word-count project remains in its stored workflow phase unless explicitly transitioned.

## Risky Files / Rollback Points

- `backend-rs/src/services/project_service.rs`: shared CRUD owner; status handling remains isolated.
- `backend-rs/src/api/projects.rs`: large route file; R3 adds focused handlers and tests only.
- `backend-rs/src/api/projects/import_workflow_owner.rs`: import validation and partial-failure behavior are
  preserved; R3 does not expand into an import transaction refactor.
- `backend-rs/src/services/wizard_service.rs` and `book_import_service.rs`: existing shared worktree changes were
  preserved; R3 uses focused owner calls.
- `frontend/src/pages/ProjectList.tsx` and `BookshelfPage.tsx`: already-dirty files received only the reviewed
  authoritative-status correction.
- The worktree contains many unrelated uncommitted changes. No delete, reset, commit, push, archive or database
  migration operation is part of R3 closure.

## Completion Decision

R3 functional implementation and its local quality evidence are complete. The next unblocked roadmap item is
R4 Story Packet / Generation Intent. G1 remains pending until R4, R5, R6 and G1-Cancel satisfy their own scoped
acceptance evidence; R7 remains blocked by G1.
