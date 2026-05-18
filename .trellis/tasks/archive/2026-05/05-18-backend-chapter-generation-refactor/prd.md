# Refactor backend chapter generation flow

## Goal

Continue the in-progress `backend-rs` refactor that splits the monolithic
chapter generation and chapter route logic into focused API and service
modules while preserving the existing runtime behavior expected by current
clients and task flows.

## Confirmed Facts

- The current workspace has extensive uncommitted changes concentrated in
  `backend-rs/src/api/`, `backend-rs/src/services/`, `backend-rs/src/models/`,
  `backend-rs/src/main.rs`, and `backend-rs/src/config.rs`.
- `backend-rs/src/api/chapters.rs` is being reduced from a large monolithic
  route file into a thin router composition layer that delegates to
  `chapter_analysis_routes`, `chapter_crud_routes`, and
  `chapter_regeneration_routes`.
- `backend-rs/src/api/chapter_batch_generation.rs` is being refactored from a
  route-plus-business-logic file into a thinner transport layer that delegates
  to dedicated services for create/query/cancel/stream/runtime behavior.
- `backend-rs/src/services/chapter_generation_service.rs` has been converted
  into a facade/re-export for runtime generation behavior, and
  `backend-rs/src/services/mod.rs` now exposes many focused service modules.
- The active architecture plan for this area is
  `docs/architecture/rust-strangler-refactor-plan-2026-05-17.zh-CN.md`
  Phase 3, which prioritizes Rust internal boundary shrinkage over adding more
  endpoints.
- `chapter_generation` has already advanced through the provider seam path:
  runtime now exposes `with_provider_payload` execution, and real callers in
  single-generation and batch-generation paths use the explicit provider
  payload entrypoint while preserving default behavior.
- `chapter_batch_generation` has already completed several low-risk seam
  reductions:
  - owned task loading moved into
    `chapter_batch_generation_owned_task_query_service.rs`
  - runtime write-side progress logic now has pure helper coverage in
    `chapter_batch_generation_runtime_state_service.rs`
  - default provider payload assembly for create/resume now lives in workflow
    result objects instead of route-local assembly
  - stream/view read-side semantics have started moving toward shared helpers
    with testable fallback and event-status behavior
- `chapter_quality` has already been split enough to hit diminishing returns
  for this round and is intentionally paused.
- The refactor touches API, service, model, and config boundaries, so it is a
  complex task and will require `design.md` and `implement.md` before
  implementation resumes.
- Repeated user direction in this session establishes a strict preference for:
  small-step strangler refactors, parallel low-conflict workstreams, no broad
  behavior changes, and verification after each slice.

## Requirements

- Keep HTTP and SSE route handlers thin; business logic should live in focused
  service modules.
- Preserve existing request/response compatibility unless a deliberate
  behavior change is explicitly approved.
- Treat runtime task semantics as compatibility-sensitive contracts. This
  includes status, checkpoint, progress fallback, stream event shape, cancel,
  resume, and terminal-state behavior.
- Preserve task runtime semantics for chapter generation, batch generation,
  regeneration, cancellation, polling, and streaming flows.
- Keep ownership/access validation explicit at the route or access-service
  boundary.
- Avoid reintroducing logic into monolithic route or service files after the
  split.
- Prefer low-risk refactor slices that either:
  - move default assembly to a more appropriate internal boundary without
    changing defaults, or
  - extract pure helper logic and add focused tests around existing behavior.
- Parallel development should be modeled as independent workstreams with clear
  file ownership and dependency notes, not as overlapping broad edits in the
  same files.
- Validate cross-layer impact on models, runtime payloads, and frontend/API
  consumers before calling the refactor complete.

## Acceptance Criteria

- [ ] Chapter-related routes in `backend-rs/src/api/` are reduced to transport
      orchestration and delegate domain behavior to focused services.
- [ ] Generation and regeneration workflows preserve existing externally
      observable API behavior unless a documented exception is approved.
- [ ] Batch-generation status, polling, cancel, resume, and streaming flows
      still use shared service-owned contracts rather than route-local payload
      assembly.
- [ ] Default provider-payload assembly for chapter-generation flows is owned
      by service/workflow boundaries rather than repeated route-local fallback
      code.
- [ ] Critical batch-generation task semantics have focused regression
      protection around pure helpers or stable read-side adapters.
- [ ] Any required model/config changes remain consistent with runtime task
      semantics and existing consumers.
- [ ] A validation plan exists for the refactored backend-rs chapter flow,
      including targeted checks or tests for the most critical endpoints and
      runtime paths.
- [ ] The next execution round can be split into independently verifiable,
      low-conflict workstreams with explicit ordering where needed.

## Out Of Scope

- Introducing new user-facing generation features unrelated to the current
  refactor.
- Broad frontend redesign or unrelated Python backend changes.
- Intentional API contract changes without explicit review.
- Connecting real prompt-context provider sources in this round.
- Unifying every task-write semantic across batch-generation services in a
  single large rewrite.
- Resuming `chapter_quality` decomposition beyond the currently extracted
  payload/query helpers.

## Open Questions

- No product-scope blocker remains from repository evidence and current user
  direction.
- The task is active in Trellis `in_progress` state. Remaining work is
  execution completion, scoped commit, then `finish-work` archival/journal
  steps.
