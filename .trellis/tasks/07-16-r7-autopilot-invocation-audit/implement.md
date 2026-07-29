# Implementation Plan: R7 Autopilot Durable Invocation Audit

## Preconditions

- [x] R7 strict Tool contract, project owner route and one-shot `novel_autopilot` task exist.
- [x] Workflow Panel launch is complete; Autopilot remains NonResumable.
- [x] Current PostgreSQL schema owner is Rust migration-executor + Alembic revision catalog.

## Phase A — Schema and Audit Owner

- [x] Add the v1 SeaORM entity and service module with typed status/record/read projections.
- [x] Add the PostgreSQL Alembic table/index revision and matching Rust migration catalog entry/head.
- [x] Build canonical SHA-256 input digests and allowlisted input/result/error projections; reject malformed strict payloads without persistence.

## Phase B — Lifecycle Integration

- [x] Create queued audit before `novel_autopilot` task registry insert/spawn; abort creation if it cannot persist.
- [x] Refactor only the workflow transition call boundary needed to execute its CAS update and audit success update inside one transaction.
- [x] Record running/failed terminal facts without leaking raw arguments; preserve generic TaskRegistry presentation and non-resumability.

## Phase C — Read Contract and Tests

- [x] Add a project-owner-scoped read route with a minimal typed response; do not add UI controls.
- [x] Add focused SQLite service/API tests: creation, success atomicity, error redaction, malformed payload, owner scope, migration metadata presence.
- [x] Run Rust format/check/focused tests, route regression tests, migration metadata tests, diff/text hygiene, then update Trellis evidence and roadmap.

## Risky Files

```text
backend-rs/src/api/background_tasks.rs
backend-rs/src/api/autopilot.rs
backend-rs/src/services/autopilot_coordinator_service.rs
backend-rs/src/services/novel_workflow_service.rs
backend-rs/src/services/schema_migration_metadata_service.rs
backend/alembic/postgres/versions/<new-revision>.py
```

## Explicit Non-Goals

No Pause/Resume/Steer, no checkpoint/recovery owner, no TaskRegistry durability rewrite,
no raw prompt/payload persistence, no Provider/MCP calls, no automatic retry and no unattended
whole-book or multi-volume generation.

## Verification Evidence (2026-07-16)

- `cargo fmt --manifest-path backend-rs/Cargo.toml --check` passed.
- `cargo check --manifest-path backend-rs/Cargo.toml -j 1` passed. The repository still emits pre-existing unused/dead-code warnings outside this task's ownership.
- `RUSTFLAGS='-C link-arg=/DEBUG:NONE' cargo test --manifest-path backend-rs/Cargo.toml -j 1 autopilot -- --nocapture` passed: 25 tests, including the real `cancel_task` handler path from a running `novel_autopilot` task to the redacted `cancelled_by_user` audit terminal state.
- `RUSTFLAGS='-C link-arg=/DEBUG:NONE' cargo test --manifest-path backend-rs/Cargo.toml -j 1 schema_migration_metadata_service -- --nocapture` passed: 34 tests.
- Python AST parsing passed for the Alembic revision and migrator metadata model; `git -C backend-rs diff --check` passed.

On this Windows/MSVC workstation, the default test link invocation can hit `LNK1318` while generating PDB data. The focused test binaries pass with `/DEBUG:NONE`; this is a local linker limitation, not a Rust compile or assertion failure.

## Route Status

This task's durable-audit scope is implemented and verified, but the broader R7 route remains `in_progress`. A durable audit does not grant recovery or replay; `novel_autopilot` remains `NonResumable`, and G2 remains a prerequisite for any unattended whole-book, multi-volume, or multi-step autonomy work.
