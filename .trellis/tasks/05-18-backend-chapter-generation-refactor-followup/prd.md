# Continue backend chapter generation refactor

## Goal

Continue the archived `05-18-backend-chapter-generation-refactor` work as a
new execution task, focusing on Rust backend migration work that keeps
chapter-generation and batch-generation behavior stable while moving faster
than repeated micro-slices.

The goal of this follow-up is not to reopen the whole refactor. It is to
finish the highest-signal remaining module migration packages in `backend-rs`
so the chapter generation flow is closer to thin transport routes plus
service-owned runtime semantics. As of the 2026-06-06 acceleration re-plan,
whole-file, whole-function-group, and whole-module packages are the default
planning unit. Small slices are still allowed only as a risk-control technique
inside an active package; they are no longer valid standalone progress units.

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

- Prefer module-level Rust migration packages in `backend-rs` when the module
  already has enough owner evidence, tests, and rollback boundaries to move
  as a coherent unit.
- Keep small-step refactors available for high-risk compatibility boundaries,
  but do not let micro-slicing become the default when a whole module can be
  migrated and verified together.
- Preserve HTTP payloads, SSE payload shapes, task lifecycle semantics, and
  default provider behavior unless an explicit exception is reviewed.
- Keep route handlers thin and continue moving business logic to focused
  service modules.
- Prioritize `chapter_generation`, `chapter_batch_generation`, and adjacent
  `chapters` / workflow modules where a full module package can retire Python
  fallback semantics or make cutover readiness measurable.
- Each module package must be independently verifiable with focused tests,
  `cargo check`, and the relevant strangler/gateway smoke or manifest
  validation when route ownership changes.
- Maintainability is part of acceptance: code should remain readable,
  cohesive, and robust; add short comments only where they clarify non-obvious
  runtime, checkpoint, fallback, or rollback behavior.
- Framework and control-flow adjustments are allowed when they reduce
  migration drag and improve ownership clarity, provided the user-facing
  behavior and rollback path remain explicit.
- Do not modify archived task contents; this task is the continuation record.
- Do not expand scope into unrelated Python backend or frontend behavior.

## Acceleration Re-plan: Whole-Module Migration Packages

The migration is now optimized for completing real Python-to-Rust ownership
packages instead of accumulating more micro-seams. A new implementation round
must first choose one package and then migrate the whole file, function group,
or module boundary that belongs to that package.

2026-06-07 Rust-first reset:

The next execution rounds must start from `backend-rs` owner work, not from
Python compatibility shell cleanup. Python edits are allowed only as companion
fallback shrink, route wiring, or test patch-surface updates after the Rust
owner and validation boundary are explicit. A round that mostly moves Python
compat helpers without adding or tightening Rust owner evidence should not be
reported as primary migration progress.

Default package order after the reset:

1. `chapter_single_generation` whole module package:
   migrate prepare, write, stream, runtime, snapshot, task-model, and quality
   owners as coherent single-chapter generation files.
2. `chapter_generation` shared owner package:
   move shared lower-level generation owners out of Python compatibility shells
   and batch-named Rust files when batch/single/resume flows all consume the
   same semantics.
3. `chapter_batch_generation` whole module package:
   migrate read, write, resume, cancel, runtime, status, stream, and task-view
   owners as one batch-generation capability family.
4. `chapters` compatibility shell package:
   shrink Python delegation only after the matching Rust owner, route parity,
   smoke evidence, and rollback knob are explicit.
5. `schema / migration owner` package:
   move startup/schema assumptions into Rust migration readiness once route
   packages expose table or field ownership pressure.

Each package must record:

- Python source map: route files, service helpers, schemas, and fallback shells.
- Rust target map: route files, service owners, models, tests, and smoke probes.
- Behavior contract: HTTP payloads, SSE payloads, task lifecycle, checkpoint
  shape, provider defaults, error shells, and fallback semantics.
- Implementation boundary: which whole files or function groups move together,
  and which compatibility shell remains intentionally frozen.
- Validation boundary: focused tests, `cargo check`, route-group smoke or
  manifest validation when route ownership changes.
- Rollback boundary: feature flags, gateway routing, fallback shell, migration
  assumption, or deployment probe that makes rollback observable.

Stop rule: do not pick a standalone "next seam" unless it is explicitly part
of the selected package and helps that package reach cutover readiness. If the
next edit only shortens a helper without retiring Python ownership, fallback
dependency, smoke gap, rollback ambiguity, or schema assumption, it should be
deferred.

Rust-first acceptance rule: before any new Python fallback shrink is counted,
the selected package must identify the Rust route/service owner, focused Rust
tests or `cargo check` command, and the exact Python fallback branch that will
be frozen, repointed, or removed only after that Rust owner is validated.

## Acceptance Criteria

- [x] A Rust migration package or risk-controlled slice is identified and
      completed without reopening archived task metadata.
- [x] The chosen package preserves externally observable behavior for chapter
      generation or batch generation flows.
- [x] Touched route files remain transport-oriented and avoid reintroducing
      route-local business logic or fallback assembly.
- [x] Touched service files own the moved semantics through focused helpers,
      workflow boundaries, read-side adapters, or module-level owners.
- [x] Validation is run for the completed package, including `cargo check`,
      focused tests for changed behavior, and route-group smoke/manifest
      validation when route ownership changes.
- [x] The task leaves a clear checkpoint for any remaining package work,
      fallback shrink readiness, and rollback boundary.
- [ ] New execution rounds select and finish package-level units by default,
      with micro-slices used only as package-internal review checkpoints.
- [ ] The chosen package includes Python source map, Rust target map, behavior
      contract, validation boundary, and rollback boundary before implementation
      starts.

## Out Of Scope

- Rewriting the entire chapter-generation architecture in one unreviewable
  pass.
- Intentional API contract changes.
- New prompt-provider capabilities beyond the current placeholder-preserving
  defaults.
- Resuming paused `chapter_quality` decomposition unless a direct dependency
  is discovered during implementation.
- Broad cleanup unrelated to the active module package.

## Open Questions

- Wave 1 checkpoint:
  `chapter_batch_generation_status_view_service.rs` now carries resolved
  `event_status` through `BatchGenerationStreamState`, and
  `build_batch_generation_progress_event()` no longer recomputes it locally.
  Focused tests now cover failed/cancelled/unknown status fallbacks plus
  existing running/completed cases.
- Remaining sequencing question:
  the next coding round should choose one package instead of one seam:
  continue package A (`chapter_generation` shared owners) if preserving the
  current owner-lift chain is highest value, or switch to package B
  (`chapter_single_generation`) if visible whole-file migration progress is
  preferred.
