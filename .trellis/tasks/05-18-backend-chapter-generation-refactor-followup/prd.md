# Continue backend chapter generation refactor

## Goal

Continue the archived `05-18-backend-chapter-generation-refactor` work as a
new execution task, focusing only on the remaining low-risk Rust backend seam
reductions that keep chapter-generation and batch-generation behavior stable.

The goal of this follow-up is not to reopen the whole refactor. It is to
finish the highest-signal remaining slices in `backend-rs` so the chapter
generation flow is closer to thin transport routes plus service-owned runtime
semantics.

## Confirmed Facts

- The previous task was archived at
  `.trellis/tasks/archive/2026-05/05-18-backend-chapter-generation-refactor`.
- Its planning artifacts already established a strict behavior-preserving
  scope and identified `chapter_batch_generation` as the highest-value
  remaining seam.
- The repository currently has broad uncommitted changes, including changes
  under `.trellis/`, `.agents/`, `.codex/`, and Rust backend files, so this
  follow-up must avoid reverting unrelated edits.
- The Rust refactor direction is still aligned with
  `docs/architecture/rust-strangler-refactor-plan-2026-05-17.zh-CN.md`
  Phase 3: reduce internal boundary size without expanding user-facing
  behavior.
- The previous task already completed part of the provider-payload seam and
  part of the batch-generation service extraction.
- No new product requirement was introduced by the user. The request is to
  continue the existing refactor task.

## Requirements

- Restrict this round to small-step strangler refactors in `backend-rs`.
- Preserve HTTP payloads, SSE payload shapes, task lifecycle semantics, and
  default provider behavior unless an explicit exception is reviewed.
- Keep route handlers thin and continue moving business logic to focused
  service modules.
- Prioritize `chapter_batch_generation` runtime/read-side semantics over broad
  route churn.
- Only take slices that are independently verifiable with targeted tests or
  `cargo check`.
- Do not modify archived task contents; this task is the continuation record.
- Do not expand scope into unrelated Python backend or frontend behavior.

## Acceptance Criteria

- [x] A new execution slice is identified and completed in the Rust
      chapter-generation refactor without reopening archived task metadata.
- [x] The chosen slice preserves externally observable behavior for chapter
      generation or batch generation flows.
- [x] Touched route files remain transport-oriented and avoid reintroducing
      route-local business logic or fallback assembly.
- [x] Touched service files own the moved semantics through focused helpers,
      workflow boundaries, or read-side adapters.
- [x] Validation is run for the completed slice, including `cargo check` and
      focused tests when pure helpers are added or changed.
- [x] The task leaves a clear checkpoint for any remaining refactor work if
      the next seam reaches diminishing returns.

## Out Of Scope

- Rewriting the entire chapter-generation architecture in one pass.
- Intentional API contract changes.
- New prompt-provider capabilities beyond the current placeholder-preserving
  defaults.
- Resuming paused `chapter_quality` decomposition unless a direct dependency
  is discovered during implementation.
- Broad cleanup unrelated to the active seam.

## Open Questions

- Wave 1 checkpoint:
  `chapter_batch_generation_status_view_service.rs` now carries resolved
  `event_status` through `BatchGenerationStreamState`, and
  `build_batch_generation_progress_event()` no longer recomputes it locally.
  Focused tests now cover failed/cancelled/unknown status fallbacks plus
  existing running/completed cases.
- Remaining sequencing question:
  should the next slice stay in read-side stream/status consolidation
  (`chapter_batch_generation_status_stream_service.rs` and adjacent helpers),
  or switch to provider seam tightening before touching route compression?
