# Implementation Plan: R7 Autopilot Control API and Human Gate

## Preconditions

- [x] R7 Tool Contract and non-resumable minimal Coordinator slices are present.
- [x] User has authorized continued direct development.
- [x] Backend and shared Trellis guidance have been reviewed.

## Phase A — Route and Contract Discovery

- [x] Locate the current Rust project route module, claims extraction, project
  access check, background-task create owner, and workflow DTO/error patterns.
- [x] Confirm one existing task-creation owner can be reused without exposing a
  second generic payload entry point.

## Phase B — Minimal Control API

- [x] Add a strict project-scoped action request DTO with denial of unknown
  fields at the top level and arguments level.
- [x] Add the authenticated route and canonical route-project injection.
- [x] Verify project access, require explicit true confirmation, validate the
  one allowlisted Tool and its public workflow fields, then create the existing
  generic `novel_autopilot` task.
- [x] Keep handlers thin and delegate task lifecycle and execution to existing
  owners.

## Phase C — Regression Tests

- [x] Add focused API tests for confirmed creation and task ownership/scope.
- [x] Add rejection tests for confirmation, unknown/injected fields, unsupported
  Tool, invalid phase, and unauthorized actor.
- [x] Preserve or extend one execution-chain regression proving the later task
  still reaches the existing Coordinator/Tool Contract/workflow owner.

## Phase D — Validation

1. `cargo fmt --manifest-path backend-rs/Cargo.toml -- --check`
2. `cargo check --manifest-path backend-rs/Cargo.toml`
3. Focused API, background-task, Coordinator, Tool Contract, and workflow tests
4. `cargo test --manifest-path backend-rs/Cargo.toml -j 1 --quiet`
5. `npm --prefix frontend run lint` and `npm --prefix frontend run build` if a
   frontend file changes; otherwise record that no frontend source changed.
6. Verify modified text is UTF-8 without BOM, LF-only, and trailing-whitespace
   free.

Use the test-process-only `rust-lld`, `debuginfo=0`, and `/DEBUG:NONE` workaround
if the local MSVC linker hits the known PDB limit. Do not modify product build
configuration.

## Completion Evidence (2026-07-16)

- Implemented `POST /api/projects/:project_id/autopilot/actions` with authenticated
  Claims ownership, project access verification, strict request DTOs, canonical
  workflow phases, explicit confirmation, and server-only route scope injection.
- Reused `create_task_for_authenticated_user` and the generic task lifecycle; the
  route does not mutate workflow state directly.
- Fixed the generic lifecycle boundary: `novel_autopilot` preserves its strict
  invocation payload, while all other task types retain existing `project_id` and
  `user_id` runtime payload enrichment. Task actor and project authority remain in
  `TaskRecord`.
- Focused API test validates the full chain: API create -> generic task ->
  Coordinator -> Tool Contract -> `novel_workflow_service::transition`, including
  receipt schema version `autopilot-tool-contract/v1`.
- Passed: `cargo fmt --check`, `cargo check`, focused API 4/4, background tasks
  29/29, Coordinator 3/3, Tool Contract 7/7, workflow 17/17, and complete Rust
  suite 1779/1779. No frontend source changed, so frontend lint/build was not
  rerun for this backend-only slice.


## Risky Files

```text
backend-rs/src/api/projects.rs
backend-rs/src/api/background_tasks.rs
backend-rs/src/services/autopilot_coordinator_service.rs
backend-rs/src/services/autopilot_tool_contract_service/
backend-rs/src/services/novel_workflow_service.rs
```

## Explicit Non-Goals

Do not add pause/resume/steer controls, migrations, durable audit/checkpoint
storage, provider/MCP calls, prompt parsing, automatic retries, multi-step
coordination, generic arbitrary-tool API inputs, or a new Autopilot UI.
