# Design

## Scope

This task continues the Rust chapter-generation refactor under
`docs/architecture/rust-strangler-refactor-plan-2026-05-17.zh-CN.md`
Phase 3: shrink internal boundaries in high-complexity chapter domains without
expanding user-visible behavior.

The design target is not a large rewrite. It is a sequence of small strangler
steps that:

- keep route handlers as HTTP/SSE boundaries only
- move default assembly and compatibility logic toward service-owned seams
- freeze task/runtime semantics with pure helpers and focused tests
- stop when a line enters diminishing returns

## Current State Summary

### Chapter generation

- Provider payload seams exist in:
  - `chapter_generation_prompt_context_provider_service.rs`
  - `chapter_generation_prompt_context_service.rs`
  - `chapter_generation_prompt_params_service.rs`
  - `chapter_generation_prompt_service.rs`
  - `chapter_generation_runtime_service.rs`
- Real callers already use the explicit `with_provider_payload` runtime entry.
- Default behavior is still placeholder-backed and intentionally unchanged.

### Chapter batch generation

- Route seam is already partially stabilized:
  - create/cancel/resume delegate to workflow-style services
  - active/status list and task payload assembly delegate to query/view helpers
  - runtime snapshot/checkpoint progression delegate to runtime helpers
  - route keeps transport wiring and `tokio::spawn`
- The recent low-risk improvements further tightened this seam:
  - owned-task lookup moved to a neutral read host
  - running-progress calculation moved into a pure helper with tests
  - default provider payload assembly for create/resume moved into workflow
    result boundaries
  - read-side stream semantics started consolidating into pure helpers

### Chapter quality

- Query/payload helpers have already been extracted far enough for this phase.
- Further splitting is low ROI and paused.

## Design Principles

1. Strict behavior preservation

- Keep HTTP payloads, SSE payload shapes, task status strings, checkpoint
  defaults, and runtime completion semantics unchanged unless explicitly
  reviewed.
- Moving a default value to another internal boundary is allowed only if the
  resulting observable behavior is unchanged.

2. Small-step strangler progression

- Prefer one seam-tightening slice at a time.
- Each slice should leave the codebase in a working, verifiable state.
- Do not batch multiple behavior-sensitive refactors into one edit set.

3. Service-owned semantics

- Route files should not own repeated fallback assembly, status mapping,
  checkpoint mutation, or background-task progression rules.
- These semantics should live in workflow/query/runtime/status helpers with
  narrow tests.

4. Diminishing-return stop rule

- If a domain line has already reached a stable seam and the next split would
  mostly move code around without reducing behavioral risk, stop and switch to
  a higher-signal seam.

## Planned Workstreams

### Workstream A: Chapter generation provider seam stabilization

Status: mostly complete for this round.

Purpose:

- keep explicit provider payload propagation from boundary to runtime
- preserve placeholder default behavior
- avoid route-local fallback reassembly

Remaining work:

- only take further steps here if a new caller still uses a compatibility
  wrapper or reintroduces route-local fallback logic

### Workstream B: Batch generation runtime/status semantics hardening

Status: active primary stream.

Purpose:

- freeze sensitive task semantics through pure helpers and focused tests
- continue shifting assembly logic from route/local branches into service-owned
  helpers

Expected slice types:

- runtime write-side progress/checkpoint pure helpers
- read-side status/message/event fallback pure helpers
- internal workflow result assembly moves that preserve defaults

Explicit non-goals:

- no large rewrite of task command semantics
- no DB schema change
- no provider source integration

### Workstream C: Chapter route seam reduction

Status: secondary stream.

Purpose:

- keep reducing `api/chapters.rs` into a router composition boundary
- ensure no new chapter-domain business logic flows back into route files

This stream can proceed in parallel only when edits do not overlap heavily with
Workstream B files.

### Workstream D: Chapter quality pause

Status: paused.

Reason:

- current extracted helpers already capture the major seam value
- further splitting would not materially reduce risk this phase

## Parallelization Strategy

Parallelism is allowed only across low-conflict workstreams.

### Safe parallel categories

- one thread modifies batch-generation read-side helpers/tests
- one thread modifies chapter route composition or isolated generation caller
  seams
- one thread performs read-only research on next candidate seam

### Unsafe parallel categories

- multiple threads editing the same batch-generation runtime/service file
- one thread changing task semantics while another changes stream semantics in
  the same contract without a shared checkpoint
- broad edits spanning route, runtime, and model layers in one batch

## Ordering / Dependencies

1. Finish planning artifacts and review.
2. Resume execution with the highest-signal low-risk seam in
   `chapter_batch_generation`.
3. Only after a seam slice is verified should the next workstream branch take
   ownership of adjacent files.
4. If route simplification depends on runtime semantics becoming stable, finish
   the semantics slice first.

## Compatibility Notes

- Preserve placeholder prompt-context provider defaults.
- Preserve task status vocabulary: `pending`, `running`, `completed`,
  `failed`, `cancelled`.
- Preserve checkpoint fallback progress/message semantics unless a later review
  explicitly changes them.
- Preserve SSE event categories and terminal handling.

## Validation Strategy

- `cargo check` after each slice using the shared target dir:
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo check`
- add focused unit tests when extracting pure helpers
- avoid broad integration churn unless a seam change touches request/response
  contracts

## Rollback Shape

- Because slices are small and behavior-preserving, rollback should happen by
  reverting the latest seam-tightening edit set only.
- Do not combine unrelated seam moves in a single execution batch.
