# Implementation Plan: R7 Autopilot Task and Minimal Coordinator

## Preconditions

- [x] R7 Tool Contract first slice is complete and validated.
- [x] User has explicitly authorized continued direct development.
- [x] Existing generic background task lifecycle, recovery policy contract, and task payload persistence limits have been inspected.

## Phase A — Contract Scope Extension

- [x] Extend the Tool Contract internal execution context with optional canonical task project scope.
- [x] Reject project ID mismatch between scope and strict Tool arguments before calling workflow service.
- [x] Add contract regression tests for scope match/mismatch and preserve existing direct-call behavior without a scope.

## Phase B — Coordinator and Task Executor

- [x] Add focused `autopilot_coordinator_service` with strict `NovelAutopilotTaskPayload` and safe error mapping.
- [x] Map only `TaskRecord.user_id`, `TaskRecord.project_id`, confirmation and raw arguments to the controlled dispatcher.
- [x] Add `novel_autopilot` generic background-task executor arm and reuse existing completion/failure lifecycle.
- [x] Register explicit `NonResumable` recovery policy.

## Phase C — Compatibility and Tests

- [x] Add frontend `BackgroundTaskType`/label presentation coverage without a control UI.
- [x] Add SQLite-backed confirmed success, confirmation rejection, scope mismatch, unauthorized, stale-CAS and error-redaction tests.
- [x] Add generic background task / recovery / production contract regression coverage.
- [x] Verify cancellation follows existing outer lifecycle and does not introduce a second terminal owner.

## Phase D — Validation

1. `cargo fmt --manifest-path backend-rs/Cargo.toml -- --check`
2. `cargo check --manifest-path backend-rs/Cargo.toml`
3. focused Tool Contract, coordinator, background task, recovery and production-contract tests
4. frontend type/lint validation where available
5. `cargo test --manifest-path backend-rs/Cargo.toml --quiet`
6. UTF-8 无 BOM、LF-only、trailing whitespace check for modified files

Use the existing test-process-only `rust-lld` + `debuginfo=0` + `/DEBUG:NONE` workaround if local MSVC hits
`LNK1318: PDB LIMIT (12)`; do not change product build configuration.

## Risky Files

```text
backend-rs/src/api/background_tasks.rs
backend-rs/src/tasks/recovery.rs
backend-rs/src/services/autopilot_tool_contract_service/
frontend/src/services/modules/backgroundTaskTypes.ts
frontend/src/store/backgroundTaskModel.ts
backend-rs/src/production_ci_contract_tests.rs
```

## Rollback Boundary

No migration or durable payload is introduced. Revert the new task type, executor arm, recovery entry, coordinator,
project-scope extension, and presentation type as one unit. Do not remove or alter existing workflow/task state owners.

## Validation Evidence (2026-07-16)

- [x] `cargo fmt --manifest-path backend-rs/Cargo.toml -- --check`
- [x] `cargo check --manifest-path backend-rs/Cargo.toml`
- [x] focused Rust tests: Tool Contract 7/7, Coordinator 3/3, background tasks 27/27,
  recovery 12/12, production contracts 16/16
- [x] `cargo test --manifest-path backend-rs/Cargo.toml -j 1 --quiet`
- [x] `npm --prefix frontend run lint` and `npm --prefix frontend run build`
- [x] modified R7 source text: UTF-8 without BOM, LF-only, no trailing whitespace

The local test process used `rust-lld`, `debuginfo=0`, and `/DEBUG:NONE` only to avoid the
known MSVC PDB linker limit; product build settings were not changed. The frontend lint
completed with pre-existing warnings outside the R7 task files.
