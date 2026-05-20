# Design

## Scope

This follow-up task continues the Rust chapter-generation refactor as a new
execution checkpoint after the previous task was archived. The scope is to
finish one or more remaining low-risk seam-tightening slices, not to redesign
the subsystem.

The design remains anchored to
`docs/architecture/rust-strangler-refactor-plan-2026-05-17.zh-CN.md`
Phase 3:

- shrink internal boundaries
- preserve observable behavior
- stop when the next split has low signal or high compatibility risk

## Continuation Model

The archived task already captured the original plan and prior progress. This
follow-up task exists to:

- preserve the archive as a historical checkpoint
- re-establish a clean Trellis execution context for the next implementation
  wave
- record only the remaining work and decisions for this round

## Current Technical Direction

### Primary seam

`chapter_batch_generation` remains the highest-signal area because it still
contains behavior-sensitive runtime and read-side semantics that benefit from
service-owned helpers and focused tests.

Current checkpoint:

- completed one read-side semantics slice in
  `backend-rs/src/services/chapter_batch_generation_status_view_service.rs`
- stream `event_status` is now produced once by
  `resolve_batch_generation_stream_semantics()` and carried through
  `BatchGenerationStreamState`
- focused tests now cover additional terminal/unknown fallback cases

Target slice categories:

- read-side status or fallback normalization
- runtime checkpoint/progress helper extraction
- workflow-result assembly consolidation that preserves defaults

### Secondary seam

Chapter route seam compression remains valid, but only after the current batch
generation slice is stable or when route-only cleanup can be done without
overlapping behavior-sensitive files.

Current recommendation:

- do not prioritize route compression next
- first finish one more low-risk batch-generation semantics slice or a
  provider-payload ownership cleanup that does not overlap runtime-write files

Updated checkpoint:

- Wave 2 has started with two compatibility-safe provider seam slices:
  - `chapter_generation_runtime_service.rs` no longer exposes a default
    provider-payload wrapper; runtime callers now go through the explicit
    `*_with_provider_payload` boundary only
  - generation request/workflow preparation now shares one
    `prepare_generation_execution_config()` helper so `AIConfig` plus the
    default provider payload are assembled once in the access/preparation
    layer instead of being repeated across batch create, batch resume, and
    single-chapter request services
- Route handlers still only forward explicit prepared values; no new route
  fallback ownership was introduced
- Single-chapter background generation now also has a dedicated workflow
  boundary so the route no longer mixes request preparation with task-plan
  creation before dispatch
- Single-chapter stream generation now also has a dedicated workflow boundary
  so the route no longer mixes request preparation with stream construction
  before returning SSE transport state
- Batch resume now also delegates execution-plan dispatch selection to the
  dispatch service so the route no longer owns `ResumeExecutionPlan` branching
- `chapter_batch_generation_task_command_service.rs` now reuses shared batch
  status semantics helpers instead of carrying a local copy of task type /
  stage / execution-mode mapping, reducing semantic drift without touching the
  runtime write path
- explicit cancel now also reuses the existing cancelled runtime-checkpoint
  semantics so task status and persisted checkpoint state do not drift apart
  when a batch is cancelled outside the runtime loop
- resume now also owns a strict snapshot-reset boundary:
  - batch resume must reset persisted checkpoint progress and chapter pointer
    instead of merge-preserving the previous terminal runtime state
  - resume must also clear stale snapshot quality fields so a restored pending
    task does not continue to expose the prior terminal quality summary
  - single-chapter resume may preserve the current chapter pointer, but batch
    resume should restart from a clean pending checkpoint view
- runtime state ownership has been narrowed so
  `chapter_batch_generation_runtime_state_service.rs` owns runtime
  checkpoint/snapshot writes, while read-side task status semantics and payload
  construction remain in the status/quality/payload adapter services.
- manual-review quality gate parsing is a shared quality-status semantic.
  Both read-side terminal labels and task-command resume blocking should use
  the same `manual_review_label()` helper.
- Phase 5 governance assets are now strong enough to support narrow
  behavior-preserving Rust migration slices:
  - `phase5-p0` / `phase5-p0-fallback` / `phase5-p0-asymmetric` now give
    route-group owner evidence a stable execution lane
  - this follow-up no longer needs to choose between “governance only” and
    “code only”; the current direction is to keep governance assets current
    while continuing low-risk `backend-rs` seam tightening
- single-chapter generation request ownership is still a valid low-risk seam:
  - the route currently still assembles the standard single-chapter request
    shape locally while separately consuming compat-only fields
  - a service-owned request builder is a compatibility-safe next move because
    it reduces route-local request assembly without changing auth, workflow,
    payload, or runtime semantics

### Paused seam

`chapter_quality` remains paused unless direct dependency pressure appears.

## Design Principles

1. Preserve compatibility first

- Keep status vocabulary, checkpoint semantics, SSE event categories, and
  default provider behavior stable.
- Moving logic across boundaries is allowed only when the observable result
  remains unchanged.

2. Choose one seam at a time

- Each slice should be understandable and reviewable on its own.
- Avoid overlapping edits across runtime, API, and models unless the seam
  requires it.

3. Prefer service-owned contracts

- Route files should only orchestrate transport concerns.
- Repeated fallback assembly, progress calculation, status adaptation, and
  task semantics belong in focused service helpers.

4. Stop at diminishing returns

- If the next move only relocates code without reducing compatibility risk or
  clarifying ownership, stop and leave a checkpoint.

## File-Level Boundaries

Most likely files for this follow-up:

- `backend-rs/src/api/chapter_batch_generation.rs`
- `backend-rs/src/api/chapters.rs`
- `backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs`
- `backend-rs/src/services/chapter_batch_generation_status_view_service.rs`
- `backend-rs/src/services/chapter_batch_generation_task_command_service.rs`
- adjacent service/test files discovered during code inspection

Route files are allowed to change only to delegate existing logic more cleanly.
Behavior-sensitive ownership should move toward service modules.

## Validation Strategy

- Run `cargo check` after each completed slice with the shared target dir:
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo check`
- Add focused unit tests when extracting or tightening pure helpers.
- Prefer narrow regression protection in touched service files over broad test
  churn.

## Rollback Shape

- Roll back only the latest seam slice if validation fails.
- Do not mix unrelated refactor moves in the same execution batch.
