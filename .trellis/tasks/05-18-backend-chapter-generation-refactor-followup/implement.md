# Implementation Plan

## Execution Rule

Do not start implementation until the planning artifacts in this task are
reviewed and approved.

## Ordered Checklist

1. Inspect the current `backend-rs` and Python backend state, then select one
   whole-file, whole-function-group, or whole-module migration package for this
   round.
2. Load the relevant backend spec indexes before editing code.
3. Record the package map before implementation:
   Python source files, Rust target files, behavior contract, validation
   commands, and rollback/cutover evidence.
4. Migrate the selected package as a coherent unit. Whole files and whole
   function groups should move together when they belong to the same behavior
   owner.
5. Use micro-slices only as internal review checkpoints inside the selected
   package; do not report them as standalone migration completion.
6. Remove, freeze, or repoint legacy wrappers only after the Rust owner and
   fallback behavior are explicit.
7. Add or update focused tests for changed service behavior, payload shape,
   task lifecycle, checkpoint, SSE, provider default, or error shell semantics.
8. Run validation with `cargo check`, targeted Rust tests, and route-group
   smoke/manifest checks when transport ownership or fallback behavior changes.
9. Leave a package checkpoint that states completed owner scope, remaining
   Python shell, rollback boundary, and the next package entrypoint.

## Latest Checkpoint

- 2026-06-06 whole-module acceleration re-plan checkpoint:
  the task execution model has been changed from standalone seam selection to
  package-first migration. From this checkpoint forward, a new coding round
  must select one package and keep the work inside that package until a whole
  file, function group, or module capability has moved or been explicitly
  paused.

  Default package order is now:
  - package A: `chapter_generation` shared lower-level owners
  - package B: `chapter_single_generation` prepare/write/stream/runtime module
  - package C: `chapter_batch_generation` read/write/resume/status/runtime module
  - package D: `chapters` compatibility shell and route delegation shrink
  - package E: `schema / migration owner`

  Package planning must include:
  - Python source map
  - Rust target map
  - preserved HTTP / SSE / task lifecycle / checkpoint / provider behavior
  - focused tests and `cargo check`
  - route-group smoke or manifest validation when route ownership changes
  - rollback/cutover notes

  Stop rule update:
  do not pick another tiny "next seam" unless that seam is part of the active
  package and directly helps finish owner, fallback, smoke, rollback, or schema
  readiness for that package.

- 2026-06-06 chapter-generation shared quality runtime-context persisted-source owner-lift checkpoint:
  this slice stayed on the same `chapter_generation` Phase 5 shared-owner
  lane immediately after the shared chapter-access owner lift had already
  removed one more batch-named lower-level entrypoint from the surrounding
  single / generation / resume production chains. Before this change, the
  shared snapshot persistence owner had already been lifted into:
  - `backend-rs/src/services/chapter_generation_snapshot_persistence_service.rs`

  but that shared owner still depended on one neighboring batch-named
  persisted-source quality context helper:
  - `chapter_batch_generation_quality_runtime_context_service::resolve_batch_quality_runtime_context_from_persisted_sources(...)`

  that dependency was no longer a real batch-only domain dependency. It
  represented one shared lower-level owner that still happened to live under a
  batch-named file:
  - `persisted quality columns + summary state -> quality runtime context`

  instead of preserving the batch file as the de facto shared persisted-source
  quality owner, this slice narrowed the shared snapshot persistence owner to
  consume the already-existing chapter-generation-scoped quality owner
  directly:
  - `chapter_generation_snapshot_persistence_service.rs`
    now calls:
    - `resolve_generation_quality_runtime_context_from_persisted_sources("batch", ...)`
  - `chapter_generation_quality_runtime_context_service.rs`
    now carries an explicit batch-scope regression test proving the shared
    persisted-source owner preserves batch ordering / summary-state behavior:
    - `should_resolve_generation_quality_runtime_context_from_persisted_summary_only_for_batch_scope`

  this is a real Phase 5 migration step because Rust now owns a clearer
  `shared snapshot persistence -> shared generation quality runtime context`
  chain instead of leaving the chapter-generation shared persistence lane
  attached to a batch file name for the same lower-level persisted quality
  semantics.

  Focused validation passed with:
  `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/shared-generation-quality-context-owner"`
  `cargo test chapter_generation_quality_runtime_context_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/shared-generation-quality-context-owner" -- --nocapture`
  `cargo test chapter_generation_snapshot_persistence_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/shared-generation-quality-context-owner" -- --nocapture`

- 2026-06-06 chapter-generation shared chapter-access owner-lift checkpoint:
  this slice stayed on the same `chapter_single_generation` Phase 5 module
  package immediately after the shared snapshot persistence owner lift had
  already made the lower-level write chain explicit. Before this change, the
  surrounding single / generation / batch resume production lanes had already
  stopped depending on batch-named snapshot query, recovery, and persistence
  entrypoints directly, but they still shared one neighboring batch-file
  access entrypoint:
  - `chapter_batch_generation_access_service.rs`
  - `load_accessible_chapter_for_generation(...)`
  - `load_accessible_chapters_for_generation(...)`

  that dependency was no longer a real `single -> batch` or
  `generation -> batch` domain dependency. It represented one shared
  lower-level owner that still happened to live under a batch-named file:
  - `chapter id -> owned / accessible generation chapter`
  - `chapter ids -> owned / accessible generation chapters`

  instead of adding another single-only forwarding facade or preserving the
  batch file as the de facto shared access entrypoint, this slice moved the
  true shared lower-level owner out of the batch file boundary:
  - `backend-rs/src/services/chapter_generation_access_service.rs`
    now owns:
    - `LoadAccessibleChapterForGenerationError`
    - `load_accessible_chapter_for_generation(...)`
    - `load_accessible_chapters_for_generation(...)`

  the neighboring consumers were then narrowed to consume that shared owner
  directly:
  - `chapter_generation_runtime_service.rs`
  - `chapter_batch_generation_resume_task_command_service.rs`
  - `chapter_single_generation_prepare_service.rs`
  - `chapter_single_generation_stream_workflow_service.rs`
  - `chapter_generation_error_mapper.rs`
  - `services/mod.rs` now exports
    `chapter_generation_access_service` and no longer exports the old
    `chapter_batch_generation_access_service`
  - `chapter_batch_generation_access_service.rs` has been removed because it
    no longer owned an independent compatibility boundary

  this is a real Phase 5 migration step because Rust now owns a clearer
  `shared chapter access -> batch/single/generation module owners` chain
  instead of leaving multiple non-batch production lanes attached to a batch
  file name for the same lower-level access semantics.

  Focused validation passed with:
  `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/shared-generation-access-owner"`
  `cargo test chapter_generation_runtime_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/shared-generation-access-owner" -- --nocapture`
  `cargo test chapter_batch_generation_resume_task_command_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/shared-generation-access-owner" -- --nocapture`
  `cargo test chapter_single_generation_prepare_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/shared-generation-access-owner" -- --nocapture`
  `cargo test chapter_single_generation_stream_workflow_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/shared-generation-access-owner" -- --nocapture`

- 2026-06-06 chapter-generation shared snapshot persistence owner-lift checkpoint:
  this slice stayed on the same `chapter_single_generation` Phase 5 module
  package immediately after the shared snapshot-query / task-recovery owner
  lift had already made the lower-level read/recover chain explicit. Before
  this change, the single-generation startup snapshot / runtime lane had
  already stopped reopening batch-named query and recovery helpers directly,
  but its runtime snapshot write path still depended on one neighboring
  batch-file entrypoint:
  - `chapter_batch_generation_snapshot_service::upsert_batch_generation_runtime_snapshot(...)`

  that call was no longer a real `single -> batch` domain dependency. It
  represented one shared lower-level owner that still happened to live under a
  batch-named file:
  - `task id + runtime state -> snapshot merge / replace persistence`

  instead of adding another single-only forwarding helper or preserving the
  batch file as the de facto persistence entrypoint, this slice moved the true
  shared lower-level owner out of the batch file boundary:
  - `backend-rs/src/services/chapter_generation_snapshot_persistence_service.rs`
    now owns:
    - `ChapterGenerationSnapshotWriteMode`
    - `merge_chapter_generation_runtime_state(...)`
    - `persist_chapter_generation_runtime_snapshot(...)`
    - `upsert_chapter_generation_runtime_snapshot(...)`
    - shared quality-column sync / backfill helpers used during snapshot
      persistence

  the neighboring consumers were then narrowed to consume that shared owner
  directly:
  - `chapter_batch_generation_snapshot_service.rs`
    now keeps batch-local queued/resume snapshot plan ownership and batch
    public APIs, but delegates lower-level merge / replace persistence into
    the shared owner
  - `chapter_single_generation_snapshot_service.rs`
    now delegates directly into the shared persistence owner instead of
    routing back through the batch snapshot file

  this is a real Phase 5 migration step because Rust now owns a clearer
  `shared snapshot persistence -> batch/single module owners` chain instead of
  leaving the single-generation production lane attached to a batch file name
  for the same lower-level write semantics.

  Focused validation passed with:
  `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-shared-snapshot-persistence-owner"`
  `cargo test chapter_generation_snapshot_persistence_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-shared-snapshot-persistence-owner" -- --nocapture`
  `cargo test chapter_batch_generation_snapshot_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-shared-snapshot-persistence-owner" -- --nocapture`
  `cargo test chapter_single_generation_snapshot_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-shared-snapshot-persistence-owner" -- --nocapture`
  `cargo test chapter_batch_generation --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-shared-snapshot-persistence-owner" -- --nocapture`
  `cargo test chapter_single_generation --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-shared-snapshot-persistence-owner" -- --nocapture`

- 2026-06-06 chapter-generation shared snapshot-query / task-recovery owner-lift checkpoint:
  this slice stayed on the same `chapter_single_generation` Phase 5 module
  package immediately after the single-generation task-model seam had already
  been closed, but it deliberately avoided creating another fake
  single-generation wrapper. Before this change, the
  `chapter_single_generation_existing_background_query_service.rs` lane had
  already pulled existing-background query/read payload ownership back into a
  dedicated single-generation file, yet it still depended directly on two
  neighboring batch-file entrypoints for lower-level shared production work:
  - `recover_batch_generation_task_if_needed(...)`
  - `load_batch_generation_snapshot_map(...)`

  those calls were no longer a real `single -> batch` domain dependency. They
  represented one shared lower-level owner that still happened to live under
  batch-named files:
  - `task row -> timeout auto-recovery`
  - `task ids -> snapshot query materialization`

  instead of adding another single-only forwarding helper, this slice moved
  the true shared lower-level owner out of the batch file boundary:
  - `backend-rs/src/services/chapter_generation_task_recovery_service.rs`
    now owns:
    - `resolve_generation_task_auto_recovery_error(...)`
    - `recover_generation_task_if_needed(...)`
  - `backend-rs/src/services/chapter_generation_snapshot_query_service.rs`
    now owns:
    - `load_chapter_generation_snapshot(...)`
    - `load_chapter_generation_snapshot_map(...)`

  the neighboring consumers were then narrowed to consume that shared owner
  directly:
  - `chapter_single_generation_existing_background_query_service.rs`
  - `chapter_batch_generation_owned_task_query_service.rs`
  - `chapter_batch_generation_read_context_service.rs`
  - `chapter_batch_generation_runtime_state_service.rs`
  - `chapter_batch_generation_snapshot_service.rs`

  this is a real Phase 5 migration step because Rust now owns a clearer
  `shared lower-level sources -> batch/single module owners` chain instead of
  leaving single-generation production lanes directly attached to batch file
  names that no longer matched the true ownership split.

  Focused validation passed with:
  `cargo test chapter_generation_task_recovery_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-shared-query-recovery-owner" -- --nocapture`
  `cargo test chapter_batch_generation_read_context_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-shared-query-recovery-owner" -- --nocapture`
  `cargo test chapter_batch_generation_owned_task_query_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-shared-query-recovery-owner" -- --nocapture`
  `cargo test chapter_single_generation_existing_background_query_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-shared-query-recovery-owner" -- --nocapture`
  `cargo test chapter_batch_generation --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-shared-query-recovery-owner" -- --nocapture`
  `cargo test chapter_single_generation --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-shared-query-recovery-owner" -- --nocapture`
  `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-shared-query-recovery-owner"`

- 2026-06-06 single-generation task-model owner-collapse checkpoint:
  this slice stayed on the same `chapter_single_generation` Phase 5 module
  package and closed the next real owner gap immediately after the
  single-generation snapshot persistence / merge lane had already been pulled
  back from the neighboring batch snapshot owner chain. Before this change,
  the single-generation background create / runtime mutation lane already
  owned:
  - chapter-scoped startup snapshot planning
  - single-generation background response payload projection
  - single runtime checkpoint stage projection
  - single runtime lifecycle persistence branches

  but the same lane still reopened one neighboring batch task-model owner
  shape directly:
  - batch-style task persistence seed semantics
  - local `ModelFieldUpdate` / `TaskTimestampUpdate`
  - local `SingleGenerationTaskStage`
  - single background task `ActiveModel` assembly from chapter target fields

  those pieces no longer represented a real batch-shared compatibility
  boundary for the chapter-scoped branch. They only replayed:
  - `single background target -> task insert seed / active model`
  - `single runtime stage -> task row mutation contract`

  `backend-rs/src/services/chapter_single_generation_task_model_service.rs`
  now narrows that boundary once more so the single-generation module itself
  carries the single task-model contract:
  - `SingleGenerationTaskPersistenceSeed`
  - `build_single_generation_background_task_persistence_seed(...)`
  - `build_single_generation_background_task_active_model(...)`
  - `ModelFieldUpdate`
  - `TaskTimestampUpdate`
  - `SingleGenerationTaskStage`
  - `SingleGenerationTaskStage::persist_for_task(...)`
  - `SingleGenerationTaskStage::apply_to_active_model(...)`

  `backend-rs/src/services/chapter_single_generation_prepare_service.rs`
  now consumes that focused single task-seed owner instead of reopening
  batch task persistence seed semantics, and
  `backend-rs/src/services/chapter_single_generation_runtime_state_service.rs`
  now consumes the same single task-stage owner instead of keeping one more
  file-local copy of the task mutation contract.

  This is a real Phase 5 migration step because Rust now owns one tighter
  single-generation background task insert -> runtime task mutation chain
  rather than preserving direct batch task-model reopen hops inside
  chapter-scoped production owners:
  - `single launch target -> single task-model owner -> runtime lane`

  Focused validation passed with:
  `cargo test chapter_single_generation_task_model_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-task-model-owner-collapse" -- --nocapture`
  `cargo test chapter_single_generation_prepare_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-task-model-owner-collapse" -- --nocapture`
  `cargo test chapter_single_generation_runtime_state_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-task-model-owner-collapse" -- --nocapture`
  `cargo test chapter_single_generation --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-task-model-owner-collapse" -- --nocapture`
  `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-task-model-owner-collapse"`

- 2026-06-06 single-generation snapshot persistence / merge owner-collapse checkpoint:
  this slice stayed on the same `chapter_single_generation` Phase 5 module
  package and closed the next real owner gap immediately after the
  single-generation existing-background query file had already been pulled
  into a dedicated single-generation owner file. Before this change, the
  single-generation startup snapshot / runtime lane already owned:
  - chapter-scoped startup snapshot planning
  - single runtime checkpoint stage projection
  - single runtime lifecycle persistence branches

  but the same lane still reopened two neighboring batch snapshot helpers
  directly:
  - `project_merged_batch_generation_runtime_state(...)`
  - `upsert_batch_generation_runtime_snapshot(...)`

  those calls no longer represented a real batch-shared compatibility
  boundary for the chapter-scoped branch. They only replayed:
  - `pending checkpoint + runtime seed -> merged runtime state`
  - `single runtime checkpoint -> persisted snapshot merge`

  `backend-rs/src/services/chapter_single_generation_snapshot_service.rs`
  now narrows that boundary once more so the single-generation module itself
  carries the single snapshot persistence/merge contract:
  - `merge_single_generation_runtime_state(...)`
  - `upsert_single_generation_runtime_snapshot(...)`
  - `SingleGenerationStartupSnapshotPlan::persist(...)` now routes through
    that local owner

  `backend-rs/src/services/chapter_single_generation_runtime_state_service.rs`
  now consumes only that focused single snapshot owner and no longer reopens
  batch snapshot helpers directly from the runtime lane.

  This is a real Phase 5 migration step because Rust now owns one tighter
  single-generation startup snapshot -> runtime checkpoint persistence chain
  rather than preserving direct batch helper reopen hops inside chapter-scoped
  production owners:
  - `single startup snapshot owner -> single snapshot merge/persist owner -> runtime lane`

  Focused validation passed with:
  `cargo test chapter_single_generation_snapshot_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-snapshot-persistence-owner-collapse" -- --nocapture`
  `cargo test chapter_single_generation_runtime_state_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-snapshot-persistence-owner-collapse" -- --nocapture`
  `cargo test chapter_single_generation --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-snapshot-persistence-owner-collapse" -- --nocapture`
  `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-snapshot-persistence-owner-collapse"`

- 2026-06-06 single-generation existing-background query file-collapse checkpoint:
  this slice stayed on the same `chapter_single_generation` Phase 5 module
  package and closed the next real owner gap immediately after the
  single-generation startup snapshot owner had already been pulled back from
  the neighboring batch snapshot file. Before this change, the
  single-generation background write lane already owned:
  - existing-task short-circuit branch selection
  - restored background launch preparation
  - persist-and-dispatch workflow entry

  but `backend-rs/src/services/chapter_single_generation_write_workflow_service.rs`
  still carried one full neighboring read/query owner chain inline:
  - active single-generation task query
  - recovered existing-background read-state loading
  - existing-background payload projection

  that group no longer represented a real write-workflow-local compatibility
  boundary. It only replayed:
  - `load active tasks -> recover snapshot -> find chapter -> build compat payload`

  `backend-rs/src/services/chapter_single_generation_existing_background_query_service.rs`
  now narrows that boundary once more so the single-generation module itself
  carries the existing-background read/query owner chain in one dedicated file:
  - `SingleGenerationExistingBackgroundTaskContext`
  - `load_owned_single_generation_existing_background_task_payload(...)`
  - `into_single_generation_existing_background_task_payload(...)`

  `backend-rs/src/services/chapter_single_generation_write_workflow_service.rs`
  now consumes only that focused query owner and keeps the workflow-entry
  branch decision plus launch path, instead of also owning the full
  existing-background task query/read-state/payload chain inline.

  This is a real Phase 5 migration step because Rust now owns one tighter
  single-generation existing-background query -> payload -> write-workflow
  chain rather than preserving a mixed write/query file that adds no new
  transport translation, no independent error contract, and no branch-local
  workflow semantics:
  - `existing-background query owner -> existing payload branch -> workflow entry`

  Focused validation passed with:
  `cargo test chapter_single_generation_existing_background_query_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-existing-background-query-file-collapse" -- --nocapture`
  `cargo test chapter_single_generation_write_workflow_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-existing-background-query-file-collapse" -- --nocapture`
  `cargo test chapter_single_generation --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-existing-background-query-file-collapse" -- --nocapture`
  `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-existing-background-query-file-collapse"`

- 2026-06-06 single-generation startup snapshot owner-collapse checkpoint:
  this slice stayed on the same `chapter_single_generation` Phase 5 module
  package and closed the next real owner gap immediately after the
  single-generation quality-status seam had already been pulled back from the
  neighboring batch owner chain. Before this change, the single-generation
  restored-launch / background-write lane already owned:
  - restored runtime-state materialization
  - startup snapshot planning inputs
  - background response payload assembly
  - runtime launch input projection

  but the chapter-scoped startup snapshot owner itself still lived in one
  neighboring batch snapshot file:
  - `chapter_batch_generation_snapshot_service::SingleGenerationStartupSnapshotPlan`

  that owner no longer represented a real batch-shared compatibility boundary
  for the chapter-scoped branch. It only replayed:
  - `pending checkpoint + runtime seed -> startup snapshot runtime state`
  - `startup snapshot -> quality/runtime restore payloads`
  - `startup snapshot -> snapshot persistence`

  `backend-rs/src/services/chapter_single_generation_snapshot_service.rs`
  now narrows that boundary once more so the single-generation module itself
  carries the remaining chapter-scoped startup snapshot contract:
  - `SingleGenerationStartupSnapshotPlan`
  - `SingleGenerationStartupSnapshotPlan::from_pending_checkpoint(...)`
  - `runtime_state()`
  - `quality_runtime_context()`
  - `latest_quality_metrics()`
  - `quality_metrics_history()`
  - `quality_metrics_summary_state()`
  - `quality_metrics_summary()`
  - `active_story_repair_payload()`
  - `quality_history_context()`
  - `persist(...)`

  `backend-rs/src/services/chapter_single_generation_prepare_service.rs`
  and
  `backend-rs/src/services/chapter_single_generation_write_workflow_service.rs`
  now consume that local owner chain directly, while
  `backend-rs/src/services/chapter_batch_generation_snapshot_service.rs`
  keeps only the remaining batch-shared snapshot owners
  (`queued`, `resume`, and persistence helpers).

  This is a real Phase 5 migration step because Rust now owns one tighter
  single-generation restored-launch -> startup snapshot -> write/runtime
  chain rather than reopening a batch snapshot owner that adds no batch/single
  branching, no transport translation, and no independent error contract:
  - `pending checkpoint + runtime seed -> single startup snapshot owner`

  Focused validation passed with:
  `cargo test chapter_single_generation_snapshot_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-startup-snapshot-owner-collapse" -- --nocapture`
  `cargo test chapter_single_generation_prepare_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-startup-snapshot-owner-collapse" -- --nocapture`
  `cargo test chapter_single_generation_write_workflow_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-startup-snapshot-owner-collapse" -- --nocapture`
  `cargo test chapter_single_generation --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-startup-snapshot-owner-collapse" -- --nocapture`
  `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-startup-snapshot-owner-collapse"`

- 2026-06-06 single-generation quality-status owner-collapse checkpoint:
  this slice stayed on the same `chapter_single_generation` Phase 5 module
  package and closed the next real owner gap immediately after the
  single-generation background payload base contract had already been pulled
  back from neighboring batch payload/status helpers. Before this change, the
  single-generation module already owned:
  - background create payload base projection
  - existing-background payload projection
  - single-generation runtime manual-review persistence

  but those two quality-facing lanes still depended on one neighboring batch
  quality-status semantic shell:
  - `BatchGenerationQualityStatusContext`
  - `manual_review_label_from_quality_context(...)`

  those helpers no longer owned a real single-generation-local compatibility
  boundary for the chapter-scoped branch. They only replayed:
  - `snapshot/runtime state -> chapter quality status context`
  - `quality payload -> manual-review label`

  `backend-rs/src/services/chapter_single_generation_quality_status_service.rs`
  now narrows that boundary once more so the single-generation module itself
  carries the remaining chapter-scoped quality-status contract:
  - `SingleGenerationQualityStatusContext`
  - `SingleGenerationQualityStatusContext::from_snapshot_and_runtime_state(...)`
  - `SingleGenerationQualityStatusContext::insert_into_payload(...)`
  - `manual_review_label_from_single_generation_quality_context(...)`

  `backend-rs/src/services/chapter_single_generation_write_workflow_service.rs`
  now consumes that local owner chain directly for existing-background quality
  payload projection, and
  `backend-rs/src/services/chapter_single_generation_runtime_state_service.rs`
  now consumes the same local owner for runtime manual-review label
  resolution.

  This is a real Phase 5 migration step because Rust now owns one tighter
  single-generation quality-status chain rather than reopening batch
  quality-status semantics that add no transport translation, no chapter/batch
  branching, and no independent error contract:
  - `chapter quality sources -> single quality status -> payload/manual-review`

  Focused validation passed with:
  `cargo test chapter_single_generation_quality_status_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-quality-status-owner-collapse" -- --nocapture`
  `cargo test chapter_single_generation_write_workflow_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-quality-status-owner-collapse" -- --nocapture`
  `cargo test chapter_single_generation_runtime_state_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-quality-status-owner-collapse" -- --nocapture`
  `cargo test chapter_single_generation --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-quality-status-owner-collapse" -- --nocapture`
  `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-quality-status-owner-collapse"`

- 2026-06-06 single-generation background payload base owner-collapse checkpoint:
  this slice stayed on the same `chapter_single_generation` Phase 5 module
  package and closed the next real owner gap immediately after the
  single-generation-specific existing-background read-context chain had
  already been pulled back from the neighboring batch read owner. Before this
  change, the single-generation background write lane already owned:
  - active-task query selection
  - existing-task read-state/context loading
  - existing-task short-circuit payload projection
  - background create response payload projection

  but those two payload lanes still depended on neighboring batch payload and
  status semantics for the remaining payload base contract:
  - `build_batch_generation_task_view_payload_from_task_state(...)`
  - `estimated_task_minutes(...)`
  - `active_batch_generation_statuses()`

  those helpers no longer owned a real single-generation-local compatibility
  boundary for the background branch. They only replayed:
  - `single task state -> task-view payload base`
  - `single task state -> active statuses`
  - `single task estimate`

  `backend-rs/src/services/chapter_single_generation_prepare_service.rs`
  now narrows that boundary once more so the single-generation module itself
  carries the remaining runtime/task payload base contract:
  - `estimated_single_generation_task_minutes(...)`
  - `single_generation_pending_stage_code()`
  - `single_generation_active_task_statuses()`
  - `build_single_generation_runtime_payload_base(...)`
  - `build_single_generation_task_view_payload_from_task_state(...)`

  `backend-rs/src/services/chapter_single_generation_write_workflow_service.rs`
  now consumes that local owner chain directly for existing-background
  payloads, instead of reopening batch payload/status helpers for the final
  base fields.

  This is a real Phase 5 migration step because Rust now owns one tighter
  single-generation background payload base chain rather than reopening batch
  task-view/status semantics that add no transport translation, no semantic
  branching, and no independent error contract:
  - `single task state -> runtime payload base -> create/existing payload`

  Focused validation passed with:
  `cargo test chapter_single_generation_prepare_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-background-payload-base-owner-collapse" -- --nocapture`
  `cargo test chapter_single_generation_write_workflow_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-background-payload-base-owner-collapse" -- --nocapture`
  `cargo test chapter_single_generation --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-background-payload-base-owner-collapse" -- --nocapture`
  `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-background-payload-base-owner-collapse"`

- 2026-06-06 single-generation existing-background read-context owner-collapse checkpoint:
  this slice stayed on the same `chapter_single_generation` Phase 5 module
  package and closed the next real owner gap immediately after the
  single-generation-specific existing-background payload variant had already
  been removed from the batch payload base. Before this change, the
  single-generation background write lane already owned:
  - active-task query selection
  - chapter match filtering
  - existing-task short-circuit payload projection

  but it still depended on one neighboring batch read-context owner chain:
  - `BatchGenerationReadContext`
  - `load_active_batch_generation_read_contexts_for_tasks(...)`
  - `batch_generation_task_contains_chapter(...)`

  those helpers no longer owned a real single-generation-local compatibility
  boundary for the existing-background branch. They only replayed:
  - `recover active tasks -> load snapshots -> build read context -> match chapter`

  `backend-rs/src/services/chapter_single_generation_write_workflow_service.rs`
  now narrows that boundary once more so the single background write owner
  itself carries the remaining single-generation-local existing-task read-state
  chain:
  - `SingleGenerationExistingBackgroundTaskContext`
  - `load_active_single_generation_existing_background_task_contexts(...)`
  - `single_generation_existing_background_task_contains_chapter(...)`

  `backend-rs/src/services/chapter_batch_generation_read_context_service.rs`
  now keeps only the remaining batch shared read-context owner chains
  (`active list`, `active project`, `status`) and no longer participates in
  the single-generation existing-background read-context contract except for
  the lower-level shared recovery primitive that still remains reusable.

  This is a real Phase 5 migration step because Rust now owns one tighter
  single-generation background write owner chain rather than reopening a batch
  read-context owner hop that adds no transport translation, no semantic
  branching, and no independent error contract:
  - `recover tasks -> load snapshots -> build single-generation read state -> match chapter`

  Focused validation passed with:
  `cargo test chapter_single_generation_write_workflow_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-existing-background-read-context-owner-collapse" -- --nocapture`
  `cargo test chapter_batch_generation_read_context_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-existing-background-read-context-owner-collapse" -- --nocapture`
  `cargo test chapter_single_generation --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-existing-background-read-context-owner-collapse" -- --nocapture`
  `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-existing-background-read-context-owner-collapse"`

- 2026-06-06 single-generation existing-background payload variant-collapse checkpoint:
  this slice stayed on the same `chapter_single_generation` Phase 5 module
  package and closed the next real owner gap immediately after the
  single-generation-specific existing-background payload projection had
  already been pulled back from the batch read-context layer. Before this
  change, the single-generation background write lane already owned:
  - active-task query selection
  - chapter match filtering
  - existing-task short-circuit payload consumption

  but it still depended on one neighboring batch payload-base variant:
  - `BatchGenerationTaskViewPayloadVariant::SingleGenerationExistingBackgroundTask`

  that variant no longer owned a real batch shared compatibility boundary for
  the single-generation branch. It only replayed:
  - `shared task-view payload base -> single-generation existing payload fields`

  `backend-rs/src/services/chapter_single_generation_write_workflow_service.rs`
  now narrows that boundary once more so the single background write owner
  itself carries the remaining single-generation-specific payload assembly:
  - `into_single_generation_existing_background_task_payload(...)`

  `backend-rs/src/services/chapter_batch_generation_task_payload_base_service.rs`
  now keeps only the remaining batch shared task-view variants
  (`active list`, `active project`, `status`) and no longer exposes a
  single-generation-specific existing-background variant.

  This is a real Phase 5 migration step because Rust now owns one tighter
  single-generation background write owner chain rather than reopening a batch
  payload-base variant that adds no transport translation, no semantic
  branching, and no independent error contract:
  - `shared payload base -> single-generation existing payload projection`

  Focused validation passed with:
  `cargo test chapter_single_generation_write_workflow_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-existing-background-payload-variant-collapse" -- --nocapture`
  `cargo test chapter_batch_generation_task_payload_base_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-existing-background-payload-variant-collapse" -- --nocapture`
  `cargo test chapter_single_generation --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-existing-background-payload-variant-collapse" -- --nocapture`
  `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-existing-background-payload-variant-collapse"`

- 2026-06-06 single-generation existing-background payload owner-collapse checkpoint:
  this slice stayed on the same `chapter_single_generation` Phase 5 module
  package and closed the next real owner gap immediately after the
  existing-background query entrypoint had already been pulled back from the
  batch task-view query layer. Before this change, the single-generation
  background write lane already owned:
  - target loading
  - active-task query selection
  - existing-task short-circuit branch selection

  but it still reopened one neighboring batch read-context projection seam:
  - `BatchGenerationReadContext::into_single_generation_existing_background_task_payload()`
  - `load_existing_single_generation_background_task_payload_for_tasks(...)`

  those helpers no longer owned a real batch compatibility boundary for the
  single-generation branch. They only replayed:
  - `active read contexts -> find chapter task -> existing background payload`

  `backend-rs/src/services/chapter_single_generation_write_workflow_service.rs`
  now narrows that boundary again so the single background write owner itself
  carries the remaining existing-task payload projection chain:
  - `into_single_generation_existing_background_task_payload(...)`
  - `load_owned_single_generation_existing_background_task_payload(...)`

  `backend-rs/src/services/chapter_batch_generation_read_context_service.rs`
  now keeps only the remaining batch shared read-context payload owners
  (`active list`, `active project`, `status`) and no longer exposes the
  single-generation-specific existing-background projection contract.

  This is a real Phase 5 migration step because Rust now owns one tighter
  single-generation background write owner chain rather than reopening a batch
  read-context projection hop that adds no transport translation, no semantic
  branching, and no independent error contract:
  - `load active tasks -> match chapter -> existing payload short-circuit`

  Focused validation passed with:
  `cargo test chapter_single_generation_write_workflow_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-existing-background-payload-owner-collapse" -- --nocapture`
  `cargo test chapter_batch_generation_read_context_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-existing-background-payload-owner-collapse" -- --nocapture`
  `cargo test chapter_single_generation --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-existing-background-payload-owner-collapse" -- --nocapture`
  `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-existing-background-payload-owner-collapse"`

- 2026-06-06 single-generation existing-background query owner-collapse checkpoint:
  this slice stayed on the same `chapter_single_generation` Phase 5 module
  package and closed the next real owner gap after the runtime-checkpoint file
  had already been collapsed. Before this change, the single-generation
  background write lane already owned:
  - target loading
  - existing-task short-circuit branch selection
  - prepared background launch / persist-and-dispatch

  but it still reopened one neighboring batch query entrypoint:
  - `chapter_batch_generation_task_view_query_service::load_existing_single_generation_background_task_payload(...)`

  that entrypoint no longer owned a real compatibility boundary for the
  single-generation branch. It only replayed:
  - `load active project tasks -> existing background payload projection`

  `backend-rs/src/services/chapter_single_generation_write_workflow_service.rs`
  now narrows that boundary so the single background write owner itself
  carries the remaining existing-task query chain:
  - `load_active_single_generation_background_tasks(...)`
  - `load_owned_single_generation_existing_background_task_payload(...)`

  `backend-rs/src/services/chapter_batch_generation_task_view_query_service.rs`
  no longer exposes the single-background existing-task entrypoint, and now
  keeps only the remaining batch active-task-list / active-project query
  lanes.

  This is a real Phase 5 migration step because Rust now owns one tighter
  single-generation write owner chain rather than reopening a batch
  task-view query hop that adds no transport translation, no semantic
  branching, and no independent error contract:
  - `target load -> existing task query -> existing payload short-circuit`

  Focused validation passed with:
  `cargo test chapter_single_generation_write_workflow_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-existing-background-query-owner-collapse" -- --nocapture`
  `cargo test chapter_batch_generation_task_view_query_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-existing-background-query-owner-collapse" -- --nocapture`
  `cargo test chapter_single_generation --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-existing-background-query-owner-collapse" -- --nocapture`
  `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-existing-background-query-owner-collapse"`

- 2026-06-06 single-generation runtime-checkpoint file-collapse checkpoint:
  this slice stayed on the same `chapter_single_generation` Phase 5 module
  package and closed the next real owner gap after the stream success owner
  chain had already been narrowed. Before this change, the single-generation
  runtime lane already owned:
  - task-stage mutation semantics
  - runtime preparation / finalization snapshot writes
  - all remaining checkpoint projection call sites

  but it still kept one extra neighboring service file:
  - `chapter_single_generation_runtime_checkpoint_service.rs`

  that file no longer owned a real compatibility boundary. It only replayed:
  - `SingleGenerationSnapshotStage -> checkpoint payload`

  `backend-rs/src/services/chapter_single_generation_runtime_state_service.rs`
  now narrows that boundary so the runtime owner itself carries the remaining
  checkpoint projection chain:
  - `SingleGenerationSnapshotStage`
  - `build_single_generation_runtime_checkpoint_for_stage(...)`

  `backend-rs/src/services/chapter_single_generation_prepare_service.rs`
  now consumes that runtime owner directly for pending checkpoint
  materialization, and the neighboring checkpoint service file has been
  removed instead of preserving one more compatibility-shaped module hop
  around the same runtime-owned checkpoint contract.

  This is a real Phase 5 migration step because Rust now owns one tighter
  single-generation runtime file-local owner chain rather than preserving a
  dead checkpoint module that adds no transport translation, no semantic
  branching, and no independent error contract:
  - `snapshot stage -> checkpoint payload -> runtime snapshot persistence`

  Focused validation passed with:
  `cargo test chapter_single_generation_runtime_state_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-runtime-checkpoint-file-collapse" -- --nocapture`
  `cargo test chapter_single_generation_prepare_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-runtime-checkpoint-file-collapse" -- --nocapture`
  `cargo test chapter_single_generation --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-runtime-checkpoint-file-collapse" -- --nocapture`
  `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-runtime-checkpoint-file-collapse"`

- 2026-06-06 single-generation stream success owner-collapse checkpoint:
  this slice stayed on the same `chapter_single_generation` Phase 5 module
  package and closed the next real owner gap after the single-generation
  prepare/runtime chain had already been narrowed. Before this change, the
  single stream success lane already let one explicit analysis owner carry:
  - follow-up analysis execution
  - latest-history quality sync trigger
  - quality metrics / quality gate / analysis-started projection
  - terminal response payload projection

  but it still kept one extra neighboring owner shell:
  - `SingleGenerationStreamCompletionProjection`

  that struct no longer owned a real compatibility boundary. It only replayed:
  - `analysis outcome -> completion message`
  - `analysis outcome -> ordered success event payloads`
  - `analysis outcome -> success SSE emission`

  `backend-rs/src/services/chapter_single_generation_stream_workflow_service.rs`
  now narrows that boundary so the analysis owner itself carries the full
  success terminal chain:
  - `SingleGenerationStreamAnalysisOutcome::completion_message(...)`
  - `SingleGenerationStreamAnalysisOutcome::ordered_success_event_payloads(...)`
  - `SingleGenerationStreamAnalysisOutcome::emit_success(...)`

  the neighboring completion owner has been removed instead of preserving one
  more compatibility-shaped hop around the same analysis-owned
  `complete -> quality events -> result -> analysis-started -> done`
  emission contract.

  This is a real Phase 5 migration step because Rust now owns one tighter
  single stream success owner chain rather than preserving a dead projection
  shell that adds no transport translation, no semantic branching, and no
  independent error contract:
  - `generated result -> follow-up analysis -> terminal projection/emission`

  Focused validation passed with:
  `cargo test chapter_single_generation_stream_workflow_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-stream-analysis-owner-collapse" -- --nocapture`
  `cargo test chapter_single_generation --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-stream-analysis-owner-collapse" -- --nocapture`
  `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-stream-analysis-owner-collapse"`

- 2026-06-06 single-generation workflow-wrapper collapse checkpoint:
  this slice stayed on the same `chapter_single_generation` Phase 5 module
  package and closed the next real owner gap after the single route edge had
  already been narrowed and after the neighboring background/stream public
  starts had already converged toward explicit Rust owners. Before this
  change, the single-generation production lane still kept four local wrapper
  hops:
  - `SingleGenerationBackgroundWorkflowRouteStart`
  - `SingleGenerationBackgroundWorkflowStart`
  - `SingleGenerationStreamWorkflowRouteStart`
  - `SingleGenerationStreamWorkflowStart::start(...)`

  those wrappers no longer owned a real compatibility boundary. They only
  replayed:
  - `route payload -> request normalization`
  - `prepare -> persist_and_dispatch`
  - `prepare -> lifecycle.spawn`

  `backend-rs/src/services/chapter_single_generation_write_workflow_service.rs`
  now narrows that boundary so the background owner chain itself carries the
  remaining production path:
  - `start_owned_single_generation_background_write_workflow_from_route_payload(...)`
  - `start_owned_single_generation_background_write_workflow(...)`
  - `SingleGenerationBackgroundWorkflowEntry::start(...)`

  `backend-rs/src/services/chapter_single_generation_stream_workflow_service.rs`
  now narrows that boundary so the stream owner chain itself carries the
  remaining production path:
  - `create_single_generation_stream_workflow_from_route_payload(...)`
  - `create_single_generation_stream_workflow(...)`
  - `SingleGenerationStreamWorkflowStart::prepare(...).spawn(...)`

  the removed wrappers were deleted instead of preserving one more
  compatibility-shaped hop around the same owner-ready request normalization,
  prepare, and launch contracts.

  This is a real Phase 5 migration step because Rust now owns one tighter
  single-generation background/stream file-local production chain rather than
  preserving dead wrapper seams that add no transport translation, no branch
  selection, and no independent error contract:
  - background:
    `route payload -> request -> workflow entry start -> persist/dispatch`
  - stream:
    `route payload -> request -> workflow prepare -> lifecycle.spawn`

  Focused validation passed with:
  `cargo test chapter_single_generation_write_workflow_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-workflow-wrapper-collapse" -- --nocapture`
  `cargo test chapter_single_generation_stream_workflow_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-workflow-wrapper-collapse" -- --nocapture`
  `cargo test chapter_single_generation --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-workflow-wrapper-collapse" -- --nocapture`
  `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-workflow-wrapper-collapse"`

- 2026-06-06 single-generation prepare/runtime owner collapse checkpoint:
  this slice stayed on the same `chapter_single_generation` Phase 5 module
  package and closed the next real owner gap after the single-generation
  workflow wrappers had already been removed. Before this change, the same
  production owner chain still kept two local forwarding seams:
  - `prepare_validated_single_chapter_generation_request_from_target(...)`
    plus `prepare_validated_from_target(...)`
  - `execute_single_generation_runtime_generation(...)`

  those helpers no longer owned a real compatibility boundary. They only
  replayed:
  - `validated request/target -> restored-launch materialization`
  - `runtime launch input -> generate/persist chapter content`

  `backend-rs/src/services/chapter_single_generation_prepare_service.rs`
  now narrows that boundary so
  `PreparedSingleChapterGenerationRestoredRuntimeLaunch::prepare_from_target(...)`
  itself carries the full validated prepare chain:
  - request-bound validation
  - request-runtime-state assembly
  - restored runtime-state loading
  - startup snapshot / runtime launch materialization

  `backend-rs/src/services/chapter_single_generation_runtime_state_service.rs`
  now narrows that boundary so `SingleGenerationRuntimeLaunchInput` itself
  carries the direct execute entry:
  - `SingleGenerationRuntimeLaunchInput::execute_generation(...)`

  both runtime lifecycle and stream lifecycle now consume that owner method
  directly instead of reopening the same launch-input handoff through a free
  helper.

  This is a real Phase 5 migration step because Rust now owns one tighter
  single-generation prepare/runtime owner chain rather than preserving dead
  helper seams that add no transport translation, no branch selection, and no
  independent error contract:
  - `validated prepare -> restored launch -> runtime launch input -> execute generation`

  Focused validation passed with:
  `cargo test chapter_single_generation_prepare_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-prepare-runtime-owner-collapse" -- --nocapture`
  `cargo test chapter_single_generation_runtime_state_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-prepare-runtime-owner-collapse" -- --nocapture`
  `cargo test chapter_single_generation --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-prepare-runtime-owner-collapse" -- --nocapture`
  `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-prepare-runtime-owner-collapse"`

- 2026-06-06 batch status-query file-collapse checkpoint:
  this slice stayed on the same `chapter_batch_generation` Phase 5 module
  package and closed the next real owner gap after the batch owned read-state
  and status payload projection contracts had already converged. Before this
  change, the batch status-query module already owned:
  - shared owned task read-state loading
  - quality-context materialization from task + snapshot sources
  - final status payload projection

  but it still kept one extra neighboring service file:
  - `chapter_batch_generation_status_task_query_service.rs`

  that file no longer owned a real compatibility boundary. It only replayed:
  - `owned read-state -> status payload projection`

  `backend-rs/src/services/chapter_batch_generation_read_context_service.rs`
  now narrows that boundary so the read-context owner itself carries the
  remaining production chain:
  - `build_batch_generation_status_task_payload_with_quality_context(...)`
  - `build_batch_generation_status_task_payload_from_task_and_snapshot_projection(...)`
  - `load_owned_batch_generation_status_payload(...)`

  the neighboring status-task query file has been removed instead of
  preserving one more compatibility-shaped module hop around the same
  read-context owner-ready read-state and status-payload contract.

  This is a real Phase 5 migration step because Rust now owns one tighter
  batch status-query file-local owner chain rather than preserving a dead
  module seam that adds no route translation, no semantic branching, and no
  independent error contract:
  - `owned read-state -> status payload`

  Focused validation passed with:
  `rustfmt --edition 2021 "backend-rs/src/services/chapter_batch_generation_read_context_service.rs" "backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs" "backend-rs/src/services/chapter_batch_generation_task_view_query_service.rs" "backend-rs/src/api/chapter_batch_generation.rs"`
  `cargo test chapter_batch_generation_read_context_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-status-file-collapse" -- --nocapture`
  `cargo test chapter_batch_generation --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-status-file-collapse" -- --nocapture`
  `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-status-file-collapse"`

- 2026-06-05 batch stream-state file-collapse checkpoint:
  this slice stayed on the same `chapter_batch_generation` Phase 5 module
  package and closed the next real owner gap after the batch status/stream
  read-state projection had already been collapsed. Before this change, the
  batch stream module already owned:
  - shared owned task read-state loading
  - stream-state semantics projection
  - status-stream polling / cursor advance
  - SSE event emission and close behavior

  but it still kept one extra neighboring service file:
  - `chapter_batch_generation_stream_state_query_service.rs`

  that file no longer owned a real compatibility boundary. It only replayed:
  - `owned read-state -> stream state projection`

  `backend-rs/src/services/chapter_batch_generation_status_stream_service.rs`
  now narrows that boundary so the status-stream owner itself carries the
  remaining production chain:
  - `build_batch_generation_stream_state_from_task_and_snapshot(...)`
  - `load_owned_batch_generation_stream_state(...)`

  the neighboring stream-state query file has been removed instead of
  preserving one more compatibility-shaped module hop around the same
  status-stream owner-ready read-state and projection contract.

  This is a real Phase 5 migration step because Rust now owns one tighter
  batch stream file-local owner chain rather than preserving a dead module
  seam that adds no transport translation, no branch selection, and no
  independent error contract:
  - `owned read-state -> stream state -> poll / emit`

  Focused validation passed with:
  `rustfmt --edition 2021 "backend-rs/src/services/chapter_batch_generation_status_stream_service.rs"`
  `cargo test chapter_batch_generation_status_stream_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-stream-state-file-collapse" -- --nocapture`
  `cargo test chapter_batch_generation --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-stream-state-file-collapse" -- --nocapture`
  `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-stream-state-file-collapse"`

- 2026-06-05 batch cancel service file-collapse checkpoint:
  this slice stayed on the same `chapter_batch_generation` Phase 5 module
  package and closed the next real owner gap after batch cancel had already
  joined the shared write-workflow lane and after the cancel workflow-start
  shell had already been removed. Before this change, the batch module
  already owned:
  - shared owned task + snapshot sources
  - terminal status gating
  - cancelled persistence-plan preparation
  - cancel workflow launch / write-workflow start

  but it still kept one extra neighboring service file:
  - `chapter_batch_generation_cancel_service.rs`

  that file no longer owned a real compatibility boundary. It only replayed:
  - `validate cancellable status`
  - `prepare cancelled persistence plan`
  - `prepare cancel workflow launch`

  `backend-rs/src/services/chapter_batch_generation_write_workflow_service.rs`
  now narrows that boundary so the batch write-workflow owner itself carries
  the remaining cancel production chain:
  - `prepare_cancel_batch_generation_persistence_plan_from_owned_sources(...)`
  - `prepare_owned_batch_generation_cancel_workflow(...)`
  - `PreparedBatchGenerationCancelWorkflowLaunch::prepare(...)`

  the neighboring cancel service file has been removed instead of preserving
  one more compatibility-shaped module hop around the same owner-ready
  sources and persistence contract.

  This is a real Phase 5 migration step because Rust now owns one tighter
  batch cancel file-local write chain rather than preserving a dead module
  seam that adds no transport translation, no branch selection, and no error
  contract that was independent from the write-workflow owner:
  - `public cancel start -> owned cancel prepare -> cancelled persistence`

  Focused validation passed with:
  `rustfmt --edition 2021 "backend-rs/src/services/chapter_batch_generation_write_workflow_service.rs"`
  `cargo test chapter_batch_generation_write_workflow_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-cancel-file-collapse" -- --nocapture`
  `cargo test chapter_batch_generation --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-cancel-file-collapse" -- --nocapture`
  `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-cancel-file-collapse"`

- 2026-06-05 batch create workflow-entry collapse checkpoint:
  this slice stayed on the same `chapter_batch_generation` Phase 5 module
  package and closed the next real owner gap after the batch write-workflow
  start shells had already been removed. Before this change, the batch create
  lane already owned:
  - create workflow launch preparation
  - create persistence-plan materialization
  - public write-workflow start

  but it still kept one extra local wrapper shell:
  - `PreparedBatchGenerationCreateWorkflowEntry`

  that wrapper no longer owned a real compatibility boundary. It only
  replayed:
  - `prepare persistence plan`
  - `persist_and_dispatch(...)`

  `backend-rs/src/services/chapter_batch_generation_write_workflow_service.rs`
  now narrows that boundary so the create persistence-plan owner itself
  carries the final start handoff:
  - `BatchGenerationCreateLaunchPersistencePlan::prepare(...)`
  - `BatchGenerationCreateLaunchPersistencePlan::start(...)`

  the outer public create write-workflow entry now hands off directly to that
  owner instead of reopening one more empty workflow-entry hop.

  This is a real Phase 5 migration step because Rust now owns one tighter
  batch create write-lane chain rather than preserving a compatibility-shaped
  wrapper that adds no validation, no branch selection, and no error
  translation:
  - `public start -> create persistence-plan owner -> persist-and-dispatch`

  Focused validation passed with:
  `rustfmt --edition 2021 "backend-rs/src/services/chapter_batch_generation_write_workflow_service.rs"`
  `cargo test chapter_batch_generation_write_workflow_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-create-entry-collapse" -- --nocapture`
  `cargo test chapter_batch_generation --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-create-entry-collapse" -- --nocapture`
  `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-create-entry-collapse"`

- 2026-06-05 batch write-workflow start-collapse checkpoint:
  this slice stayed on the same `chapter_batch_generation` Phase 5 module
  package and closed the next real owner gap after batch cancel had already
  joined the shared write-workflow lane. Before this change, the batch module
  already routed create / resume / cancel through write-workflow owners, but
  each lane still kept one extra local wrapper shell:

  - create:
    `PreparedBatchGenerationCreateWorkflowStart`
  - resume:
    `PreparedBatchGenerationResumeWorkflowStart`
  - cancel:
    `PreparedBatchGenerationCancelWorkflowStart`

  those wrappers no longer owned a real compatibility boundary. They only
  replayed:
  - `prepare(...)`
  - `persist_and_dispatch(...)`

  `backend-rs/src/services/chapter_batch_generation_write_workflow_service.rs`
  now narrows that boundary so the neighboring production owners themselves
  carry the final start handoff:
  - create:
    `PreparedBatchGenerationCreateWorkflowEntry::start(...)`
  - resume:
    `PreparedBatchGenerationResumeWorkflowLaunch::start(...)`
  - cancel:
    `PreparedBatchGenerationCancelWorkflowLaunch::start(...)`

  the outer public write-workflow entries now hand off directly to those
  adjacent owners instead of reopening one more empty workflow-start hop.

  This is a real Phase 5 migration step because Rust now owns one tighter
  batch write-lane start chain rather than preserving a compatibility-shaped
  wrapper that adds no validation, no branch selection, and no error
  translation:
  - create:
    `public start -> workflow entry -> persist-and-dispatch`
  - resume:
    `public start -> workflow launch -> persist-and-dispatch`
  - cancel:
    `public start -> workflow launch -> persist`

  Focused validation passed with:
  `rustfmt --edition 2021 "backend-rs/src/services/chapter_batch_generation_cancel_service.rs" "backend-rs/src/services/chapter_batch_generation_write_workflow_service.rs"`
  `cargo test chapter_batch_generation_write_workflow_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-workflow-start-owner-collapse" -- --nocapture`
  `cargo test chapter_batch_generation_cancel_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-workflow-start-owner-collapse" -- --nocapture`
  `cargo test chapter_batch_generation --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-workflow-start-owner-collapse" -- --nocapture`

- 2026-06-05 batch cancel write-workflow owner checkpoint:
  this slice stayed on the same `chapter_batch_generation` Phase 5 module
  package and closed the next real owner gap on the batch command lane after
  create/resume had already been routed through the batch write-workflow
  public-start boundary. Before this change, the batch module already had:
  - create -> write workflow
  - resume -> write workflow

  but cancel was still a route-local special case:
  - route -> `cancel_owned_batch_generation_task(...)`

  `backend-rs/src/services/chapter_batch_generation_cancel_service.rs`
  now narrows back to the lower-level command owner responsibilities only:
  - owned task + snapshot source loading
  - terminal status gating
  - cancelled persistence-plan preparation

  `backend-rs/src/services/chapter_batch_generation_write_workflow_service.rs`
  now also owns:
  - `PreparedBatchGenerationCancelWorkflowLaunch`
  - `PreparedBatchGenerationCancelWorkflowStart`
  - `cancel_owned_batch_generation_write_workflow(...)`

  `backend-rs/src/api/chapter_batch_generation.rs`
  now delegates the cancel route to that batch write-workflow public-start
  owner instead of calling the lower-level cancel service directly.

  This is a real Phase 5 migration step because Rust now owns one more full
  batch command public-start chain rather than only another shared helper:
  the batch command lane now has one consistent owner shape across:
  `create / resume / cancel`

  Focused validation passed with:
  `cargo test chapter_batch_generation_cancel_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-cancel-write-workflow-owner-v2" -- --nocapture`
  `cargo test chapter_batch_generation_write_workflow_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-cancel-write-workflow-owner-v2" -- --nocapture`
  `cargo test chapter_batch_generation --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-cancel-write-workflow-owner-v2" -- --nocapture`
  `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-cancel-write-workflow-owner-v2"`

- 2026-06-05 batch owned task-sources owner checkpoint:
  this slice stayed on the same `chapter_batch_generation` Phase 5 module
  package and closed the next real owner gap between the newer shared
  read-state owner and the still-duplicated command-side task source loading.
  Before this change, the batch module had already narrowed the read/query
  lane with one shared owned `task -> recover -> snapshot` owner, but the
  neighboring command lanes still reopened a parallel lower-level handoff:

  - cancel:
    `load_owned_task(...) -> load_batch_generation_snapshot(...)`
  - resume:
    `load_owned_task(...) -> load_batch_generation_snapshot(...)`

  `backend-rs/src/services/chapter_batch_generation_owned_task_query_service.rs`
  now keeps two explicit layers instead of forcing one owner to cover both
  read and command semantics:
  - `OwnedBatchGenerationTaskSources`
  - `load_owned_batch_generation_task_sources(...)`
  - `OwnedBatchGenerationTaskReadState`
  - `load_owned_batch_generation_task_read_state(...)`

  owner responsibilities now stay separated and audit-friendly:
  - sources owner:
    - owned task lookup
    - snapshot loading
    - no recovery side effects
  - read-state owner:
    - consume the shared sources owner
    - apply `recover_batch_generation_task_if_needed(...)`
    - keep read/query semantics unchanged

  `backend-rs/src/services/chapter_batch_generation_cancel_service.rs`
  now consumes the shared sources owner directly instead of replaying local
  task + snapshot loading before cancel persistence planning.

  `backend-rs/src/services/chapter_batch_generation_resume_task_command_service.rs`
  now also consumes the shared sources owner directly instead of replaying
  local task + snapshot loading before resume launch preparation.

  This is a real Phase 5 migration step because Rust now owns one more full
  lower-level command-side source chain rather than only another helper move:
  cancel/resume now hand off directly from one shared owned task-sources owner
  while read-side recovery remains isolated in the higher read-state owner.

  Focused validation passed with:
  `cargo test chapter_batch_generation_owned_task_query_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-owned-sources-owner" -- --nocapture`
  `cargo test chapter_batch_generation_cancel_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-owned-sources-owner" -- --nocapture`
  `cargo test chapter_batch_generation_resume_task_command_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-owned-sources-owner" -- --nocapture`
  `cargo test chapter_batch_generation --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-owned-sources-owner" -- --nocapture`
  `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-owned-sources-owner"`

- 2026-06-05 继续迁移 checkpoint:
  this round first had to recover from a workspace-corruption incident instead
  of immediately starting a new seam. Two files were left as `0 bytes` after a
  prior write/format failure:
  - `.trellis/tasks/05-18-backend-chapter-generation-refactor-followup/implement.md`
  - `backend-rs/src/services/chapter_single_generation_runtime_state_service.rs`

  Recovery rule for this task going forward:
  - if a source file is corrupted by workspace/disk failure, restore the
    smallest compilable baseline first
  - re-verify `cargo check` / focused tests before continuing the next seam
  - do not mix corruption recovery with broader refactor changes in one
    unverified step

- 2026-06-05 single runtime lifecycle owner checkpoint:
  this slice continued the same `chapter_single_generation` Phase 5 module
  migration by closing the next real owner gap behind single background write
  workflow public-start. Before this change, the single runtime lane already
  owned most of the semantics, but the outer dispatch path still reopened a
  separate runtime driver shell:

  - `dispatch_single_chapter_generation_runtime(...)`
    `-> execute_single_generation_runtime(...)`

  `backend-rs/src/services/chapter_single_generation_runtime_state_service.rs`
  now narrows that runtime boundary so one explicit lifecycle owner sequences
  the whole single runtime lane:
  - `SingleGenerationRuntimeLifecyclePlan::from_runtime_launch(...)`
  - owner responsibilities now stay together:
    - persist preparing state
    - execute chapter generation
    - run follow-up analysis
    - route completed / manual-review / failed terminal persistence
  - `dispatch_single_chapter_generation_runtime(...)` now hands launch input
    directly to the lifecycle owner instead of reopening a separate
    `execute_single_generation_runtime(...)` wrapper chain
  - the completed path no longer keeps duplicate terminal persistence branches
    split by `enable_analysis`; the lifecycle owner now owns one shared
    completed persistence path after analysis gating resolves

  This is a real Phase 5 migration step because Rust now owns one more full
  runtime execution chain rather than only another helper relocation:
  background/resume runtime launch now hands off directly to a single
  lifecycle owner that executes:
  `prepare -> execute -> analysis -> terminal persist`

  Focused validation passed with:
  `cargo test chapter_single_generation_runtime_state_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-runtime-public-start-owner" -- --nocapture`
  `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-runtime-public-start-owner"`

- 2026-06-05 single stream workflow public-start owner checkpoint:
  this slice continued the same `chapter_single_generation` Phase 5 module
  migration by closing the next real owner gap beside single runtime
  lifecycle. Before this change, the stream lane already owned the real
  stream lifecycle, but the outer public stream entry still reopened one more
  handoff chain:

  - `PreparedSingleChapterGenerationRestoredRuntimeLaunch::prepare(...)`
  - `into_runtime_launch_input()`
  - `SingleGenerationStreamLifecyclePlan::from_runtime_launch(...).spawn(...)`

  `backend-rs/src/services/chapter_single_generation_stream_workflow_service.rs`
  now narrows that stream-entry boundary so one explicit workflow-start owner
  carries the public handoff:
  - `SingleGenerationStreamWorkflowStart::prepare(...)`
  - `SingleGenerationStreamWorkflowStart::start(...)`
  - owner responsibilities now stay together:
    - prepare restored runtime launch
    - convert to lifecycle input once
    - hand off to stream lifecycle spawn
  - `create_single_generation_stream_workflow(...)` now calls the workflow-start
    owner directly instead of replaying
    `prepare -> into_runtime_launch_input -> lifecycle.spawn`
    inline at the outer free function entry

  This is a real Phase 5 migration step because Rust now owns one more full
  stream-entry chain rather than only another helper relocation:
  the public stream entry now hands off directly to one workflow-start owner
  that executes:
  `prepare restored runtime launch -> lifecycle.spawn`

  Focused validation passed with:
  `cargo test chapter_single_generation_stream_workflow_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-stream-workflow-start-owner" -- --nocapture`
  `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-stream-workflow-start-owner"`

- 2026-06-05 resume restored-state owner checkpoint:
  this slice continued the same `chapter_batch_generation` / `resume-runtime`
  Phase 5 package by closing the next real owner gap after restored runtime
  projection had already been narrowed. Before this change, the restored
  resume lane already owned:
  - restored `request_runtime_state`
  - restored `runtime_state_seed`

  but the neighboring command lane still reopened the same handoff through:
  - `into_launch_parts()`
  - local `request_runtime_state -> runtime_input` replay

  `backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs`
  now narrows that boundary so the restored-state owner itself materializes
  the final launch contract:
  - `RestoredResumeRuntimeStateProjection::prepare_batch_runtime_launch(...)`
  - `RestoredResumeRuntimeStateProjection::prepare_single_chapter_runtime_launch(...)`
  - owner responsibilities now stay together:
    - restored request-runtime projection
    - restored runtime-state seed handoff
    - batch/single runtime launch materialization
  - `backend-rs/src/services/chapter_batch_generation_resume_task_command_service.rs`
    now calls those owner methods directly instead of reopening
    `into_launch_parts()` and replaying launch assembly in the command branch

  This is a real Phase 5 migration step because Rust now owns one more full
  restored-state -> launch chain rather than only another helper relocation:
  the batch/single resume lane now hands off directly from restored runtime
  projection to launch-ready owner materialization.

  Focused validation passed with:
  `cargo test chapter_batch_generation_resume_task_command_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/resume-restored-state-owner" -- --nocapture`
  `cargo test chapter_batch_generation_runtime_state_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/resume-restored-state-owner" -- --nocapture`
  `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/resume-restored-state-owner"`

- 2026-06-05 single restored-launch direct materialization owner checkpoint:
  this slice continued the same `chapter_single_generation` Phase 5 module
  package by closing the next real owner gap after the restored-launch owner
  had already taken over prepare entry, stream public-start, and background
  launch-parts assembly. Before this change, the restored-launch lane already
  owned:
  - startup snapshot planning
  - runtime launch input
  - background launch-parts projection

  but neighboring production lanes still reopened the same handoff through:
  - `prepare(...).into_runtime_launch_input()`
  - `prepare_from_target(...).into_background_launch_parts(task_id)`

  `backend-rs/src/services/chapter_single_generation_prepare_service.rs`
  now narrows that boundary so the restored-launch owner itself materializes
  the final production-ready contract:
  - `PreparedSingleChapterGenerationRestoredRuntimeLaunch::prepare_runtime_launch_input(...)`
  - `PreparedSingleChapterGenerationRestoredRuntimeLaunch::prepare_background_launch_parts_from_target(...)`
  - owner responsibilities now stay together:
    - restored-launch preparation
    - startup snapshot planning
    - runtime launch materialization
    - background launch-parts materialization
  - `chapter_single_generation_stream_workflow_service.rs` now consumes the
    direct runtime-launch owner method instead of reopening
    `prepare(...).into_runtime_launch_input()`
  - `chapter_single_generation_write_workflow_service.rs` now consumes the
    direct background-launch owner method instead of reopening
    `prepare_from_target(...).into_background_launch_parts(task_id)`

  This is a real Phase 5 migration step because Rust now owns one more full
  restored-launch -> production materialization chain rather than only another
  helper relocation: the stream/background neighboring lanes now hand off
  directly from restored-launch preparation to owner-materialized launch
  products.

  Focused validation passed with:
  `cargo test chapter_single_generation_prepare_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-restored-launch-owner" -- --nocapture`
  `cargo test chapter_single_generation_write_workflow_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-restored-launch-owner" -- --nocapture`
  `cargo test chapter_single_generation_stream_workflow_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-restored-launch-owner" -- --nocapture`
  `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-restored-launch-owner"`

- 2026-06-05 batch create direct persistence-plan owner checkpoint:
  this slice continued the same `chapter_batch_generation` Phase 5 package by
  closing the next real owner gap on the batch create write lane after the
  workflow-launch owner had already taken over startup snapshot planning,
  runtime launch assembly, and response/task-seed projection. Before this
  change, the create lane already owned:
  - normalized chapter targets
  - startup snapshot plan
  - runtime input
  - create response payload and task seed

  but the neighboring workflow-entry lane still reopened the same handoff
  through:
  - `PreparedBatchGenerationCreateWorkflowLaunch::prepare(...)`
  - local `.into_persistence_plan(...)`

  `backend-rs/src/services/chapter_batch_generation_write_workflow_service.rs`
  now narrows that boundary so the workflow-launch owner itself materializes
  the final persistence-plan contract:
  - `PreparedBatchGenerationCreateWorkflowLaunch::prepare_persistence_plan(...)`
  - owner responsibilities now stay together:
    - create launch preparation
    - startup snapshot planning
    - runtime launch assembly
    - task-seed / response payload assembly
    - persistence-plan materialization
  - `PreparedBatchGenerationCreateWorkflowEntry::prepare(...)` now consumes
    that owner method directly instead of reopening
    `prepare(...).into_persistence_plan(...)` in the workflow-entry branch

  This is a real Phase 5 migration step because Rust now owns one more full
  create launch -> persistence-plan chain rather than only another helper
  relocation: the batch create write lane now hands off directly from the
  workflow-launch owner to a persistence-ready owner contract.

  Focused validation passed with:
  `cargo test chapter_batch_generation_write_workflow_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-create-persistence-owner" -- --nocapture`
  `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-create-persistence-owner"`

- 2026-06-05 batch runtime public-start owner checkpoint:
  this slice stayed on the same `chapter_batch_generation` Phase 5 runtime
  package and closed the next real owner gap at the outer batch runtime entry.
  Before this change, the batch runtime lane already owned the real lifecycle
  sequencing through `BatchGenerationRuntimeLifecyclePlan`, but the public
  entry still reopened one more wrapper chain:

  - `dispatch_batch_generation_runtime(...)`
  - `execute_batch_generation_runtime(...)`
  - `BatchGenerationRuntimeDriver::new(...).execute(...)`
  - `BatchGenerationRuntimeLifecyclePlan::execute(...)`

  `backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs`
  now narrows that boundary so the lifecycle owner itself carries the public
  handoff:
  - `BatchGenerationRuntimeLifecyclePlan::start(...)`
  - owner responsibilities now stay together:
    - public runtime launch handoff
    - preparing persistence
    - chapter iteration / step progression
    - lifecycle stop / continue routing
  - batch runtime dispatch now hands execution input directly to that
    lifecycle owner instead of reopening a separate
    `execute_batch_generation_runtime(...) -> runtime driver` wrapper chain
  - the now-unused `execute_batch_generation_runtime(...)` wrapper was removed
    after verification so this seam does not leave a new dead compatibility
    shell behind

  This is a real Phase 5 migration step because Rust now owns one more full
  batch runtime public-start -> lifecycle chain rather than only another
  helper relocation: the outer runtime entry now hands off directly to one
  lifecycle owner that executes:
  `public start -> persist preparing -> iterate chapter steps -> stop/continue`

  Focused validation passed with:
  `cargo test chapter_batch_generation_runtime_state_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-runtime-public-start-owner" -- --nocapture`
  `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-runtime-public-start-owner"`

- 2026-06-05 batch runtime post-analysis direct-owner checkpoint:
  this slice stayed on the same `chapter_batch_generation` Phase 5 runtime
  package and closed the next real owner gap inside the success lane after
  batch runtime public-start had already been narrowed. Before this change,
  the batch runtime lane already owned:
  - follow-up analysis through `BatchGenerationFollowUpAnalysisPlan`
  - terminal routing through `BatchGenerationPostAnalysisTerminalPlan`

  but the success lane still reopened the same handoff through:
  - local `run_follow_up_analysis(...)`
  - local `resolve_analysis_outcome(...)`

  `backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs`
  now narrows that boundary so the success chain itself directly consumes the
  explicit owners:
  - `PreparedBatchGenerationStepExecution::execute_success_chain(...)`
  - owner responsibilities now stay together:
    - post-write guard resolution
    - follow-up analysis owner handoff
    - post-analysis terminal owner handoff
  - the batch runtime success lane now hands the analysis result directly to
    `BatchGenerationPostAnalysisTerminalPlan`
    instead of reopening a separate
    `run_follow_up_analysis(...) -> resolve_analysis_outcome(...)` wrapper chain

  This is a real Phase 5 migration step because Rust now owns one more full
  batch runtime success -> post-analysis-terminal chain rather than only
  another helper relocation: the success lane now hands off directly from
  follow-up analysis to terminal owner routing on the production path.

  Focused validation passed with:
  `cargo test chapter_batch_generation_runtime_state_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-runtime-post-analysis-owner" -- --nocapture`
  `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-runtime-post-analysis-owner"`

- 2026-06-05 batch runtime analysis-attempt direct-preparation checkpoint:
  this slice stayed on the same `chapter_batch_generation` Phase 5 runtime
  package and closed the next real owner gap inside the follow-up analysis
  attempt lane. Before this change, the batch runtime lane already owned:
  - analysis-attempt state through `BatchGenerationAnalysisAttemptPlan`
  - completion/retry resolution through
    `BatchGenerationAnalysisAttemptResolutionPlan`

  but the attempt lane still reopened the same handoff through:
  - one outer `persist_started(...)` branch
  - one local `execute_prepared_or_fallback(...)` wrapper

  `backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs`
  now narrows that boundary so the analysis-attempt owner itself directly
  carries the whole production attempt:
  - `BatchGenerationAnalysisAttemptPlan::execute(...)`
  - owner responsibilities now stay together:
    - prepared-analysis selection
    - started-snapshot persistence
    - prepared vs fallback execution
    - resolution owner handoff
  - the batch runtime analysis-attempt lane no longer reopens a separate
    `execute_prepared_or_fallback(...)` wrapper or split started-persist
    branches around the same production attempt

  This is a real Phase 5 migration step because Rust now owns one more full
  batch runtime analysis-attempt chain rather than only another helper
  relocation: the follow-up analysis lane now stays on one owner from
  prepared/fallback attempt preparation through completion/retry resolution.

  Focused validation passed with:
  `cargo test chapter_batch_generation_runtime_state_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-runtime-analysis-attempt-next-owner" -- --nocapture`
  `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-runtime-analysis-attempt-next-owner"`

- 2026-06-05 batch runtime terminal quality-gate direct-owner checkpoint:
  this slice stayed on the same `chapter_batch_generation` Phase 5 runtime
  package and closed the next real owner gap inside the post-analysis
  terminal lane. Before this change, the batch runtime lane already owned:
  - post-analysis terminal state through
    `BatchGenerationPostAnalysisTerminalPlan`
  - quality-gate retry/manual-review routing through
    `BatchGenerationQualityGateRoutingPlan`

  but the terminal success lane still reopened the same handoff through:
  - a separate `BatchGenerationQualityGateResolutionPlan`
  - local success-path handoff from terminal owner to that neighbor

  `backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs`
  now narrows that boundary so the post-analysis terminal owner itself
  carries the quality-gate resolution chain:
  - `BatchGenerationPostAnalysisTerminalPlan::resolve_quality_gate_outcome(...)`
  - owner responsibilities now stay together:
    - post-analysis terminal success routing
    - retry-budget source loading
    - quality-gate terminal semantics resolution
    - retry/manual-review routing handoff
  - the batch runtime terminal lane no longer materializes a separate
    `BatchGenerationQualityGateResolutionPlan` for the same production path

  This is a real Phase 5 migration step because Rust now owns one more full
  batch runtime post-analysis-terminal -> quality-gate chain rather than only
  another helper relocation: the terminal lane now stays on one owner from
  analysis-success outcome through quality-gate retry/manual-review routing.

  Focused validation passed with:
  `cargo test chapter_batch_generation_runtime_state_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-runtime-quality-gate-terminal-owner" -- --nocapture`
  `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-runtime-quality-gate-terminal-owner"`

- 2026-06-05 batch runtime lifecycle-step direct-owner checkpoint:
  this slice stayed on the same `chapter_batch_generation` Phase 5 runtime
  package and closed the next real owner gap inside the lifecycle -> step
  lane. Before this change, the batch runtime lane already owned:
  - step preparation through `PreparedBatchGenerationStepExecution::prepare(...)`
  - prepared-step execution through
    `PreparedBatchGenerationStepExecution::execute(...)`

  but the lifecycle lane still reopened the same handoff through:
  - local `preparation_retry_count` orchestration inside
    `BatchGenerationRuntimeLifecyclePlan::execute_step(...)`
  - local retry-carry reconstruction before handing back to the prepared-step
    owner

  `backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs`
  now narrows that boundary so the prepared-step owner itself carries the
  retry-aware step entry:
  - `PreparedBatchGenerationStepExecution::start(...)`
  - owner responsibilities now stay together:
    - step preparation
    - retry carry reuse after preparation-level retry
    - prepared-step execution handoff
  - `BatchGenerationRuntimeLifecyclePlan::execute(...)` now hands each chapter
    step directly to that owner instead of reopening
    `prepare -> carry retry -> execute` inside a neighboring lifecycle helper
  - the now-unused `execute_step(...)` lifecycle wrapper was removed after
    verification so this seam does not leave a new dead forwarding shell

  This is a real Phase 5 migration step because Rust now owns one more full
  batch runtime lifecycle-step chain rather than only another helper
  relocation: the lifecycle lane now hands off directly from chapter
  iteration to one retry-aware step owner that executes:
  `prepare step -> reuse retry carry -> execute prepared step`

  Focused validation passed with:
  `rustfmt --edition 2021 backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs`
  `cargo test chapter_batch_generation_runtime_state_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-runtime-lifecycle-step-owner" -- --nocapture`
  `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-runtime-lifecycle-step-owner"`

- 2026-06-05 batch runtime analysis-attempt direct-resolution checkpoint:
  this slice stayed on the same `chapter_batch_generation` Phase 5 runtime
  package and closed the next real owner gap inside the follow-up analysis
  attempt lane after the direct-preparation seam was already in place. Before
  this change, the batch runtime lane already owned:
  - analysis-attempt state through `BatchGenerationAnalysisAttemptPlan`
  - prepared/fallback execution inside the same attempt owner

  but the attempt lane still reopened the same handoff through:
  - a separate `BatchGenerationAnalysisAttemptResolutionPlan`
  - local result handoff from the attempt owner to that neighbor

  `backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs`
  now narrows that boundary so the analysis-attempt owner itself also carries
  the final completion/retry resolution chain:
  - `BatchGenerationAnalysisAttemptPlan::resolve_result(...)`
  - owner responsibilities now stay together:
    - analysis-started snapshot persistence
    - prepared vs fallback analysis execution
    - completion snapshot persistence or retry routing
  - the batch runtime analysis-attempt lane no longer materializes a separate
    `BatchGenerationAnalysisAttemptResolutionPlan` for the same production
    path

  This is a real Phase 5 migration step because Rust now owns one more full
  batch runtime analysis-attempt -> completion/retry chain rather than only
  another helper relocation: the follow-up analysis lane now stays on one
  owner from attempt execution through final completion/retry routing.

  Focused validation passed with:
  `rustfmt --edition 2021 backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs`
  `cargo test chapter_batch_generation_runtime_state_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-runtime-analysis-attempt-direct-resolution-owner" -- --nocapture`
  `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-runtime-analysis-attempt-direct-resolution-owner"`

- 2026-06-05 batch runtime step-generation-attempt direct-owner checkpoint:
  this slice stayed on the same `chapter_batch_generation` Phase 5 runtime
  package and closed the next real owner gap inside the prepared-step lane.
  Before this change, the batch runtime lane already owned:
  - retry-aware step entry through `PreparedBatchGenerationStepExecution::start(...)`
  - prepared-step execution through `PreparedBatchGenerationStepExecution::execute(...)`

  but the prepared-step lane still reopened the same handoff through:
  - a local `execute_generation_attempt(...)` wrapper
  - a local `execute_success_chain(...)` wrapper

  `backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs`
  now narrows that boundary so the prepared-step owner itself directly carries
  the full generation-attempt chain:
  - `PreparedBatchGenerationStepExecution::execute(...)`
  - owner responsibilities now stay together:
    - chapter-started persistence
    - prerequisite gating
    - attempt-input preparation
    - generation execution
    - post-write guard
    - follow-up analysis
    - terminal routing
  - the batch runtime prepared-step lane no longer materializes separate
    `execute_generation_attempt(...)` or `execute_success_chain(...)` wrappers
    for the same production path

  This is a real Phase 5 migration step because Rust now owns one more full
  batch runtime step-generation-attempt chain rather than only another helper
  relocation: the prepared-step lane now stays on one owner from step retry
  entry through final post-analysis terminal routing.

  Focused validation passed with:
  `rustfmt --edition 2021 backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs`
  `cargo test chapter_batch_generation_runtime_state_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-runtime-step-generation-attempt-direct-owner" -- --nocapture`
  `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-runtime-step-generation-attempt-direct-owner"`

- 2026-06-05 batch runtime attempt-input direct-generation checkpoint:
  this slice stayed on the same `chapter_batch_generation` Phase 5 runtime
  package and closed the next real owner gap inside the generation-attempt
  input lane after the prepared-step owner had already absorbed the broader
  step-generation-attempt chain. Before this change, the batch runtime lane
  already owned:
  - compat restore through `BatchGenerationAttemptInputPlan::prepare(...)`
  - prompt-override materialization
  - provider-payload preparation

  but the prepared-step lane still reopened the same production handoff
  through:
  - local `BatchGenerationAttemptInputPlan::prepare(...)`
  - one outer `generate_and_persist_chapter_content_with_provider_payload(...)`
    call replay

  `backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs`
  now narrows that boundary so the attempt-input owner itself directly carries
  the full input-to-generation chain:
  - `BatchGenerationAttemptInputPlan::execute(...)`
  - owner responsibilities now stay together:
    - compat restore
    - prompt-override materialization
    - provider-payload preparation
    - generation execution call
  - the prepared-step lane no longer reopens
    `prepare attempt input -> local generate_and_persist...` after the
    attempt-input owner is already explicit

  This is a real Phase 5 migration step because Rust now owns one more full
  batch runtime attempt-input -> generation execution chain rather than only
  another helper relocation: the generation-attempt lane now stays on one
  owner from restored compat/provider assembly through the actual generation
  runtime call.

- 2026-06-05 single runtime direct-generation-analysis checkpoint:
  this slice switched to the adjacent `chapter_single_generation` Phase 5
  runtime package after the current batch-runtime lane reached diminishing
  returns for high-signal seams. Before this change, the single runtime lane
  already owned:
  - lifecycle public-start through `SingleGenerationRuntimeLifecyclePlan`
  - preparation persistence
  - terminal completed / failed / manual-review persistence semantics

  but the lifecycle body still reopened the same production chain through:
  - `execute_owned_single_chapter_generation(...)`
  - `run_single_generation_follow_up_analysis(...)`
  - `maybe_fail_single_generation_for_quality_gate_manual_review(...)`

  `backend-rs/src/services/chapter_single_generation_runtime_state_service.rs`
  now narrows that boundary so the lifecycle owner itself directly carries the
  full runtime execute-to-terminal chain:
  - `SingleGenerationRuntimeLifecyclePlan::execute_generation(...)`
  - `SingleGenerationRuntimeLifecyclePlan::run_follow_up_analysis(...)`
  - `SingleGenerationRuntimeLifecyclePlan::persist_manual_review_generation(...)`
  - owner responsibilities now stay together:
    - generation execution call
    - follow-up analysis routing
    - manual-review terminal persistence
    - completed / failed terminal persistence
  - the single runtime lane no longer reopens those free helpers beside the
    lifecycle owner for one-call production handoff replay

  This is a real Phase 5 migration step because Rust now owns one more full
  single runtime generation execution -> follow-up analysis -> terminal
  persistence chain rather than only another helper relocation: the runtime
  lifecycle now stays on one owner from launch input through quality-gate
  routing and final terminal persistence.

- 2026-06-05 single background launch-parts persistence owner checkpoint:
  this slice stayed on the same `chapter_single_generation` Phase 5 write
  package and closed the next real owner gap inside the background launch
  lane after restored-launch materialization and workflow-start ownership had
  already been narrowed. Before this change, the single background write lane
  already owned:
  - existing-task payload short-circuit
  - restored background launch-parts materialization
  - task-seed / startup snapshot / response payload / runtime-input ownership

  but the final production write path still reopened the same handoff
  through:
  - `persist_and_dispatch_background_launch_parts(...)`

  `backend-rs/src/services/chapter_single_generation_prepare_service.rs`
  now narrows that boundary so the launch-parts owner itself directly carries
  the final persistence/disptach chain:
  - `PreparedSingleGenerationBackgroundLaunchParts::persist_and_dispatch(...)`
  - owner responsibilities now stay together:
    - task insert active-model materialization
    - startup snapshot persistence
    - runtime dispatch
    - response payload return
  - `chapter_single_generation_write_workflow_service.rs` no longer keeps a
    neighboring free helper that reopens the same final persistence/dispatch
    chain after the launch-parts owner is already fully materialized

  This is a real Phase 5 migration step because Rust now owns one more full
  single-background launch-parts -> persistence/disptach chain rather than
  only another helper relocation: the background write lane now stays on one
  owner from prepared launch-parts through persisted task creation and runtime
  dispatch.

  Focused validation to run:
  `cargo test chapter_single_generation_prepare_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-background-launch-parts-persistence-owner" -- --nocapture`
  `cargo test chapter_single_generation_write_workflow_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-background-launch-parts-persistence-owner" -- --nocapture`
  `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-background-launch-parts-persistence-owner"`

- 2026-06-05 single stream success analysis-projection owner checkpoint:
  this slice stayed on the same `chapter_single_generation` Phase 5 stream
  package and closed the next real owner gap inside the successful stream
  lane after stream public-start and success emission ordering had already
  been narrowed. Before this change, the single stream lane already owned:
  - generated result handoff
  - completion projection owner
  - ordered SSE success emission plan

  but the success chain still reopened the same production handoff through:
  - `run_single_generation_stream_follow_up_analysis(...)`
  - `build_single_generation_stream_quality_metrics_event(...)`
  - `build_single_generation_stream_quality_gate_event(...)`
  - `build_single_generation_stream_analysis_started_event(...)`
  - `build_single_generation_stream_result_payload(...)`

  `backend-rs/src/services/chapter_single_generation_stream_workflow_service.rs`
  now narrows that boundary so the stream analysis owner itself directly
  carries the active success analysis/projection chain:
  - `SingleGenerationStreamAnalysisOutcome::from_generated_result(...)`
  - `SingleGenerationStreamAnalysisOutcome::run_follow_up_analysis(...)`
  - `SingleGenerationStreamAnalysisOutcome::quality_metrics_event(...)`
  - `SingleGenerationStreamAnalysisOutcome::quality_gate_event(...)`
  - `SingleGenerationStreamAnalysisOutcome::analysis_started_event(...)`
  - `SingleGenerationStreamAnalysisOutcome::response_payload(...)`
  - owner responsibilities now stay together:
    - follow-up analysis execution
    - latest-history quality sync trigger
    - quality event projection
    - analysis-started event projection
    - terminal response payload projection
  - `SingleGenerationStreamCompletionProjection` no longer reopens those free
    helpers beside the analysis/completion owners for one-call production
    handoff replay

  This is a real Phase 5 migration step because Rust now owns one more full
  single stream success analysis -> payload/event projection chain rather than
  only another helper relocation: the success lane now stays on one owner from
  generated result through follow-up analysis and terminal SSE payload
  assembly.

  Focused validation to run:
  `cargo test chapter_single_generation_stream_workflow_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-stream-success-analysis-projection-owner" -- --nocapture`
  `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-stream-success-analysis-projection-owner"`

- 2026-06-05 single route workflow-start owner checkpoint:
  this slice stayed on the same `chapter_single_generation` Phase 5 module
  package and closed the next real owner gap at the HTTP route edge after the
  background/stream workflow public-start owners had already been narrowed.
  Before this change, the single route lane already owned only transport
  semantics, but both route entries still reopened the same local handoff
  through:
  - `build_single_chapter_generation_request_from_route_payload(...)`
  - `start_owned_single_generation_background_write_workflow(...)`
  - `create_single_generation_stream_workflow(...)`

  `backend-rs/src/api/chapter_generation_routes.rs` now narrows that
  boundary so the neighboring background/stream workflow owners themselves
  carry the route-payload normalization handoff:
  - `start_owned_single_generation_background_write_workflow_from_route_payload(...)`
  - `create_single_generation_stream_workflow_from_route_payload(...)`
  - owner responsibilities now stay together:
    - route-payload normalization
    - workflow public-start handoff
    - background/stream entry sequencing
  - the HTTP route no longer rebuilds `SingleChapterGenerationRequest`
    locally before calling the neighboring workflows

  This is a real Phase 5 migration step because Rust now owns one more full
  single route payload -> workflow-start chain rather than only another helper
  relocation: the single background/stream route edge now hands off directly
  from transport payload to the workflow owners that execute the generation
  lanes.

  Focused validation to run:
  `cargo test chapter_single_generation_write_workflow_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-route-workflow-start-owner" -- --nocapture`
  `cargo test chapter_single_generation_stream_workflow_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-route-workflow-start-owner" -- --nocapture`
  `cargo test chapter_generation_routes --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-route-workflow-start-owner" -- --nocapture`
  `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-route-workflow-start-owner"`

- 2026-06-05 batch route workflow-start owner checkpoint:
  this slice switched back to the adjacent `chapter_batch_generation`
  Phase 5 module package after the single-generation route edge had already
  been narrowed. Before this change, the batch create route lane already
  owned only transport semantics, but the create entry still reopened the same
  local handoff through:
  - `build_batch_generation_create_workflow_request_from_route_payload(...)`
  - `start_owned_batch_generation_write_workflow(...)`

  `backend-rs/src/api/chapter_batch_generation.rs` and
  `backend-rs/src/services/chapter_batch_generation_write_workflow_service.rs`
  now narrow that boundary so the neighboring batch create workflow owner
  itself carries the route-payload normalization handoff:
  - `build_batch_generation_create_workflow_request_from_route_payload(...)`
  - `start_owned_batch_generation_write_workflow_from_route_payload(...)`
  - owner responsibilities now stay together:
    - route-payload normalization
    - workflow public-start handoff
    - batch create entry sequencing
  - the HTTP route no longer rebuilds
    `BatchGenerationCreateWorkflowRequest` locally before calling the
    neighboring write workflow

  This is a real Phase 5 migration step because Rust now owns one more full
  batch route payload -> workflow-start chain rather than only another helper
  relocation: the batch create route edge now hands off directly from
  transport payload to the workflow owner that executes the create lane.

  Focused validation passed with:
  `rustfmt --edition 2021 "backend-rs/src/api/chapter_batch_generation.rs" "backend-rs/src/services/chapter_batch_generation_write_workflow_service.rs"`
  `cargo test chapter_batch_generation_write_workflow_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-route-workflow-start-owner" -- --nocapture`
  `cargo test chapter_batch_generation --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-route-workflow-start-owner" -- --nocapture`
  `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-route-workflow-start-owner"`

- 2026-06-05 batch create route-start collapse checkpoint:
  this slice stayed on the same adjacent `chapter_batch_generation`
  Phase 5 module package after the batch create route workflow-start owner
  had already been narrowed. Before this change, the batch create route lane
  had already moved route-payload normalization into the write-workflow
  boundary, but `chapter_batch_generation_write_workflow_service.rs` still
  kept one extra local wrapper shell:
  - `BatchGenerationCreateWorkflowRouteStart`

  that wrapper no longer owned a real compatibility boundary. It only
  replayed:
  - `route payload -> workflow request`
  - `start_owned_batch_generation_write_workflow(...)`

  `backend-rs/src/services/chapter_batch_generation_write_workflow_service.rs`
  now narrows that boundary so the route-payload builder and write-workflow
  public-start owner themselves carry the final create handoff:
  - `build_batch_generation_create_workflow_request_from_route_payload(...)`
  - `start_owned_batch_generation_write_workflow_from_route_payload(...)`
  - `start_owned_batch_generation_write_workflow(...)`

  the outer create route entry now hands off directly through that tighter
  owner chain instead of reopening one more empty route-start hop beside the
  already-materialized write-workflow owner.

  This is a real Phase 5 migration step because Rust now owns one tighter
  batch create route edge rather than preserving a compatibility-shaped
  wrapper that adds no validation, no branch selection, and no error
  translation:
  - `route payload -> workflow request builder -> write-workflow owner`

  Focused validation passed with:
  `cargo test chapter_batch_generation_write_workflow_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-create-route-start-collapse" -- --nocapture`
  `cargo test chapter_batch_generation --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-create-route-start-collapse" -- --nocapture`
  `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-create-route-start-collapse"`

- 2026-06-05 batch active-task-list route-query owner checkpoint:
  this slice stayed on the adjacent `chapter_batch_generation`
  Phase 5 module package after the batch-create route edge had already been
  narrowed. Before this change, the batch active-task-list route lane already
  owned only transport semantics, but the query entry still reopened the same
  local handoff through:
  - `build_active_batch_generation_task_list_query_request(...)`
  - `load_active_batch_generation_task_list(...)`

  `backend-rs/src/api/chapter_batch_generation.rs`,
  `backend-rs/src/services/chapter_batch_generation_task_view_query_service.rs`,
  and `backend-rs/src/api/chapter_batch_generation_error_mapper.rs`
  now narrow that boundary so the neighboring active-task query owner itself
  carries the route-query normalization and shared error handoff:
  - `ActiveBatchGenerationTaskListRouteQueryError`
  - `ActiveBatchGenerationTaskListRouteQuery`
  - `build_active_batch_generation_task_list_query_request_from_route_query(...)`
  - `load_active_user_batch_generation_task_list_view_from_route_query(...)`
  - `map_active_batch_generation_task_list_route_error(...)`
  - owner responsibilities now stay together:
    - route-query normalization
    - active-task query public-start handoff
    - request/query error ownership
    - active-task-list view sequencing
  - the HTTP route no longer rebuilds
    `ActiveBatchGenerationTaskListQueryRequest` locally before calling the
    neighboring query workflow

  This is a real Phase 5 migration step because Rust now owns one more full
  batch route query -> active-task-list owner chain rather than only another
  helper relocation: the batch active-task-list route edge now hands off
  directly from transport query parameters to the query owner that executes
  the active-task read lane.

  Focused validation passed with:
  `rustfmt --edition 2021 "backend-rs/src/services/chapter_batch_generation_task_view_query_service.rs" "backend-rs/src/api/chapter_batch_generation_error_mapper.rs" "backend-rs/src/api/chapter_batch_generation.rs"`
  `cargo test chapter_batch_generation_task_view_query_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-active-task-list-route-owner" -- --nocapture`
  `cargo test chapter_batch_generation_error_mapper --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-active-task-list-route-owner" -- --nocapture`
  `cargo test chapter_batch_generation --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-active-task-list-route-owner" -- --nocapture`
  `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-active-task-list-route-owner"`

- 2026-06-05 batch active-project route-query owner checkpoint:
  this slice stayed on the same adjacent `chapter_batch_generation`
  Phase 5 module package after the batch active-task-list route edge had
  already been narrowed. Before this change, the batch active-project route
  lane already owned only transport semantics, but the project query entry
  still reopened the same local handoff through:
  - `project_id` path extraction in the route
  - `load_active_batch_generation_query(...)`

  `backend-rs/src/api/chapter_batch_generation.rs`,
  `backend-rs/src/services/chapter_batch_generation_task_view_query_service.rs`,
  and `backend-rs/src/api/chapter_batch_generation_error_mapper.rs`
  now narrow that boundary so the neighboring active-project query owner
  itself carries the route-project handoff and shared error mapping:
  - `ActiveProjectBatchGenerationRouteError`
  - `load_active_batch_generation_view_from_route_project(...)`
  - `map_active_project_batch_generation_route_error(...)`
  - owner responsibilities now stay together:
    - route-project handoff
    - active-project query public-start handoff
    - project-access/query error ownership
    - active-project payload view sequencing
  - the HTTP route no longer directly replays the active-project query start
    before calling the neighboring query workflow

  This is a real Phase 5 migration step because Rust now owns one more full
  batch route project -> active-project query owner chain rather than only
  another helper relocation: the batch active-project route edge now hands
  off directly from transport path parameters to the query owner that
  executes the active-project read lane.

  Focused validation passed with:
  `rustfmt --edition 2021 "backend-rs/src/services/chapter_batch_generation_task_view_query_service.rs" "backend-rs/src/api/chapter_batch_generation_error_mapper.rs" "backend-rs/src/api/chapter_batch_generation.rs"`
  `cargo test chapter_batch_generation_task_view_query_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-active-project-route-owner" -- --nocapture`
  `cargo test chapter_batch_generation_error_mapper --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-active-project-route-owner" -- --nocapture`
  `cargo test chapter_batch_generation --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-active-project-route-owner" -- --nocapture`
  `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-active-project-route-owner"`

- 2026-06-05 batch query route-start collapse checkpoint:
  this slice stayed on the same adjacent `chapter_batch_generation`
  Phase 5 module package after the batch active-task-list / active-project
  route-query owners had already been narrowed. Before this change, the batch
  task-view query lane had already moved transport handoff into query owners,
  but `chapter_batch_generation_task_view_query_service.rs` still kept two
  extra local wrapper shells:
  - `ActiveBatchGenerationTaskListRouteStart`
  - `ActiveProjectBatchGenerationRouteStart`

  those wrappers no longer owned a real compatibility boundary. They only
  replayed route-normalized query/path values into the same neighboring query
  owner chain.

  `backend-rs/src/services/chapter_batch_generation_task_view_query_service.rs`
  now narrows that boundary so the route-query owners themselves carry the
  final task-view handoff:
  - active-task-list now stops at:
    - `ActiveBatchGenerationTaskListRouteQuery`
    - `build_active_batch_generation_task_list_query_request_from_route_query(...)`
    - `load_active_user_batch_generation_task_list_view_from_route_query(...)`
  - active-project now stops at:
    - `load_active_batch_generation_view_from_route_project(...)`
    - `ActiveProjectBatchGenerationRouteError`

  `backend-rs/src/api/chapter_batch_generation.rs` tests were updated to stop
  depending on the removed route-start shell and instead assert the current
  project-id transport contract directly.

  This is a real Phase 5 migration step because Rust now owns one tighter
  batch task-view query chain rather than preserving a Python-era
  compatibility-shaped wrapper that adds no validation, no branch selection,
  and no error translation:
  - active-task-list:
    `route query -> route-query owner -> task-view payload`
  - active-project:
    `route path -> route-query owner -> active-project payload`

  Focused validation passed with:
  `cargo test chapter_batch_generation_task_view_query_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-query-route-start-collapse" -- --nocapture`
  `cargo test chapter_batch_generation --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-query-route-start-collapse" -- --nocapture`
  `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-query-route-start-collapse"`

- 2026-06-05 batch owned read-state owner checkpoint:
  this slice stayed on the same `chapter_batch_generation`
  Phase 5 module package after the active-project/active-task route-query
  edges had already been narrowed. Before this change, the neighboring
  batch status-query and batch status-stream read lanes already owned their
  final payload/event semantics, but both helpers still reopened the same
  production read-state chain independently through:
  - `load_owned_task(...)`
  - `recover_batch_generation_task_if_needed(...)`
  - `load_batch_generation_snapshot(...)`

  `backend-rs/src/services/chapter_batch_generation_owned_task_query_service.rs`,
  `backend-rs/src/services/chapter_batch_generation_status_task_query_service.rs`,
  and
  `backend-rs/src/services/chapter_batch_generation_stream_state_query_service.rs`
  now narrow that boundary so one shared owned read-state owner itself carries
  the common read-side materialization:
  - `OwnedBatchGenerationTaskReadState`
  - `load_owned_batch_generation_task_read_state(...)`
  - owner responsibilities now stay together:
    - owned-task load
    - active-timeout recovery
    - snapshot materialization
  - the neighboring status-payload and status-stream owners now consume that
    shared owner state directly instead of replaying the same three-step read
    chain in parallel helper bodies

  This is a real Phase 5 migration step because Rust now owns one more full
  shared batch read-state chain rather than only another helper relocation:
  the status-payload and status-stream lanes now diverge from one explicit
  owned read-state owner instead of each reopening the same production
  `task -> recover -> snapshot` chain independently.

  Focused validation passed with:
  `rustfmt --edition 2021 "backend-rs/src/services/chapter_batch_generation_owned_task_query_service.rs" "backend-rs/src/services/chapter_batch_generation_status_task_query_service.rs" "backend-rs/src/services/chapter_batch_generation_stream_state_query_service.rs"`
  `cargo test chapter_batch_generation_owned_task_query_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-owned-read-state-owner" -- --nocapture`
  `cargo test chapter_batch_generation_status_task_query_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-owned-read-state-owner" -- --nocapture`
  `cargo test chapter_batch_generation_stream_state_query_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-owned-read-state-owner" -- --nocapture`
  `cargo test chapter_batch_generation --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-owned-read-state-owner" -- --nocapture`
  `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-owned-read-state-owner"`

- 2026-06-05 batch status/stream read-state projection collapse checkpoint:
  this slice stayed on the same adjacent `chapter_batch_generation`
  Phase 5 module package after the shared owned read-state owner had already
  been narrowed. Before this change, the batch status-query and batch
  status-stream lanes already consumed one shared
  `OwnedBatchGenerationTaskReadState`, but each lane still kept one extra
  local projection wrapper shell:
  - status:
    `PreparedOwnedBatchGenerationStatusPayloadQuery`
  - stream:
    `build_batch_generation_stream_state_from_read_state(...)`

  those wrappers no longer owned a real compatibility boundary. They only
  replayed:
  - `shared read-state -> status payload projection`
  - `shared read-state -> stream-state projection`

  `backend-rs/src/services/chapter_batch_generation_status_task_query_service.rs`
  now narrows that boundary so the status lane projects directly from the
  shared read-state owner:
  - `load_owned_batch_generation_status_payload(...)`
  - `build_owned_batch_generation_status_payload_from_read_state(...)`

  `backend-rs/src/services/chapter_batch_generation_stream_state_query_service.rs`
  now narrows that boundary so the stream lane projects directly from the
  same shared read-state owner:
  - `load_owned_batch_generation_stream_state(...)`
  - `OwnedBatchGenerationTaskReadState::into_parts()`
  - `build_batch_generation_stream_state_for_task_and_snapshot(...)`

  This is a real Phase 5 migration step because Rust now owns one tighter
  shared read-state -> final projection chain rather than preserving
  compatibility-shaped wrappers that add no validation, no branch selection,
  and no error translation:
  - status:
    `shared read-state -> status payload owner`
  - stream:
    `shared read-state -> stream-state owner`

  Focused validation passed with:
  `cargo test chapter_batch_generation_status_task_query_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-status-stream-read-state-collapse" -- --nocapture`
  `cargo test chapter_batch_generation_stream_state_query_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-status-stream-read-state-collapse" -- --nocapture`
  `cargo test chapter_batch_generation --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-status-stream-read-state-collapse" -- --nocapture`
  `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-status-stream-read-state-collapse"`

- 2026-06-05 batch task-view prepared-query collapse checkpoint:
  this slice stayed on the same adjacent `chapter_batch_generation`
  Phase 5 module package after the route-query and shared read-side seams had
  already been narrowed. Before this change, the batch task-view query lane
  already owned direct task loading plus final payload projection, but
  `chapter_batch_generation_task_view_query_service.rs` still kept three extra
  local wrapper shells:
  - `PreparedActiveBatchGenerationTaskListView`
  - `PreparedActiveProjectBatchGenerationQuery`
  - `PreparedExistingSingleGenerationBackgroundTaskPayloadQuery`

  those wrappers no longer owned a real compatibility boundary. They only
  replayed:
  - `prepare active-task-list items -> into_payload`
  - `prepare active-project payload -> into_payload`
  - `prepare existing single-background payload -> into_payload`

  `backend-rs/src/services/chapter_batch_generation_task_view_query_service.rs`
  now narrows that boundary so the task-view query owner itself carries the
  final payload projection:
  - active-task-list now stops at:
    - `load_active_user_batch_generation_task_list_view(...)`
    - `build_active_batch_generation_task_list_view_payload(...)`
  - active-project now stops at:
    - `load_active_batch_generation_query(...)`
    - `build_active_project_batch_generation_view_payload(...)`
  - existing single-background now stops at:
    - `load_existing_single_generation_background_task_payload(...)`
    - `load_existing_single_generation_background_task_payload_for_tasks(...)`

  This is a real Phase 5 migration step because Rust now owns one tighter
  task-view query -> final payload chain rather than preserving
  compatibility-shaped wrappers that add no validation, no branch selection,
  and no error translation:
  - active-task-list:
    `task-view query owner -> list payload projection`
  - active-project:
    `task-view query owner -> active-project payload projection`
  - existing background:
    `task-view query owner -> existing-background payload projection`

  Focused validation passed with:
  `cargo test chapter_batch_generation_task_view_query_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-task-view-prepared-collapse" -- --nocapture`
  `cargo test chapter_batch_generation --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-task-view-prepared-collapse" -- --nocapture`
  `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-task-view-prepared-collapse"`

- 2026-06-05 batch resume launch-sources collapse checkpoint:
  this slice stayed on the same adjacent `chapter_batch_generation`
  Phase 5 module package after the restored-state runtime launch seam had
  already been narrowed. Before this change, the batch resume command lane
  already owned restored runtime-state recovery plus final launch-persistence
  materialization, but
  `chapter_batch_generation_resume_task_command_service.rs`
  still kept one extra local wrapper shell:
  - `PreparedBatchGenerationResumeLaunchSources`

  that wrapper no longer owned a real compatibility boundary. It only
  replayed:
  - `prepare restored runtime state`
  - `into launch persistence plan`

  `backend-rs/src/services/chapter_batch_generation_resume_task_command_service.rs`
  now narrows that boundary so the resume owner chain itself carries the final
  preparation handoff:
  - `prepare_resume_launch_restored_state(...)`
  - `BatchGenerationResumeLaunchPersistencePlan::prepare(...)`
  - `BatchGenerationResumeLaunchPersistencePlan::prepare_from_validated_execution(...)`

  This is a real Phase 5 migration step because Rust now owns one tighter
  batch resume restored-state -> launch-persistence chain rather than
  preserving a compatibility-shaped wrapper that adds no validation, no error
  translation, and no branch selection:
  - `resume command state -> restored-state owner -> launch-persistence owner`

  Focused validation passed with:
  `cargo test chapter_batch_generation_resume_task_command_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-resume-launch-sources-collapse" -- --nocapture`
  `cargo test chapter_batch_generation --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-resume-launch-sources-collapse" -- --nocapture`
  `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-resume-launch-sources-collapse"`

## Next Recommended Package

Do not continue by choosing another standalone seam. Choose one package and
move the whole file, function group, or module capability that belongs to it.

Recommended next package options:

1. Package A, `chapter_generation` shared owner completion:
   continue the current owner-lift chain and finish moving shared access,
   snapshot, recovery, quality runtime-context, and runtime-state semantics out
   of batch-named files where they are no longer batch-only. This is the safest
   continuation if the goal is minimum behavioral risk with real owner cleanup.
2. Package B, `chapter_single_generation` whole-module migration:
   switch to visible whole-file progress in the single-generation module:
   prepare, write workflow, stream workflow, runtime state, snapshot, task
   model, and quality status. This is the best fit if the next round should
   show direct Python-to-Rust migration progress rather than another shared
   owner lift.
3. Package C, `chapter_batch_generation` whole-module migration:
   use when the next work must improve route-group cutover readiness for batch
   create/resume/read/status/stream/runtime together.

Package entry checklist before code:

- list Python source files and fallback shells being replaced or frozen
- list Rust files to migrate as a package
- list behavior contracts that must remain stable
- list focused tests and `cargo check` command with a dedicated target dir
- list route smoke/manifest validation if route or fallback ownership changes
- list rollback boundary and remaining Python dependency
