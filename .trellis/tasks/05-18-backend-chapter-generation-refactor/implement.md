# Implementation Plan

## Execution Rule

Do not resume implementation until the user reviews these artifacts and
explicitly approves execution for this planning task.

## Ordered Checklist

1. Reconfirm active seam and file ownership before each execution round.
   - Primary line: `chapter_batch_generation`
   - Secondary line: route seam reduction in `api/chapters.rs` or adjacent
     chapter generation callers

2. Advance one low-risk seam slice at a time in `chapter_batch_generation`.
   - Prefer pure helper extraction plus focused tests.
   - Prefer internal workflow/default assembly moves that preserve behavior.
   - Stop when the next edit would require changing task-write semantics across
     multiple services.

3. Validate after every slice.
   - Run:
     `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo check`
   - If the slice adds a pure helper, add/update unit tests in the same file
     where practical.

4. Use parallel work only for low-conflict streams.
   - Read-only research may run in parallel with main-thread implementation.
   - Implementation streams must have clear file ownership and no overlapping
     writes.

5. Keep chapter-quality work paused unless a verified dependency forces
   re-entry.

## Proposed Execution Waves

### Wave 1: Batch-generation semantics hardening

Goal:

- stabilize read/write semantics with pure helpers and tests

Candidate slices:

- runtime running-progress helper coverage
- success-checkpoint helper coverage
- stream fallback/message/event-status helper coverage
- resume-checkpoint helper only if it remains isolated and worthwhile

Validation:

- `cargo check`
- focused unit tests in touched service files

### Wave 2: Provider seam tightening

Goal:

- remove remaining route-local default provider assembly where safe

Candidate slices:

- internal workflow result payload ownership
- caller cleanup where real runtime entrypoints already exist

Validation:

- `cargo check`
- grep/sanity check to ensure no reintroduced route-local fallback in the same
  path

### Wave 3: Route seam compression

Goal:

- keep route files transport-only and prevent logic regression

Candidate slices:

- additional route delegation cleanup in `api/chapters.rs`
- shared helper extraction only where repetition is already visible

Validation:

- `cargo check`
- targeted request/response sanity checks if route payload shaping changes

## Parallel Workstream Proposal

These are planning-level workstreams for the next execution round.

### Workstream 1

- Area: `chapter_batch_generation` semantics hardening
- Ownership: batch-generation status/runtime helper files
- Dependency: none; this is the highest-priority line

### Workstream 2

- Area: chapter route seam reduction
- Ownership: `backend-rs/src/api/chapters.rs` and isolated route helpers
- Dependency: do not overlap with Workstream 1 files

### Workstream 3

- Area: read-only seam discovery
- Ownership: docs/spec inspection plus candidate-file analysis only
- Dependency: none

## Risky Files / Review Points

- `backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs`
- `backend-rs/src/services/chapter_batch_generation_status_view_service.rs`
- `backend-rs/src/services/chapter_batch_generation_task_command_service.rs`
- `backend-rs/src/api/chapter_batch_generation.rs`
- `backend-rs/src/api/chapters.rs`

Review before execution if any planned slice would:

- change response payload fields
- change SSE event kinds or terminal timing
- change task status/checkpoint defaults
- expand provider behavior beyond placeholder defaults

## Pre-Start Validation

Before `task.py start`, confirm:

- `prd.md` reflects current progress and strict behavior-preserving scope
- `design.md` captures workstreams and stop rules
- `implement.md` lists ordered slices and validation commands
- the user has reviewed and approved execution
