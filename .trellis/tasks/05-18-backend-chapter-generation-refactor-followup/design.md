# Design

## Scope

This follow-up task continues the Rust chapter-generation refactor as a new
execution checkpoint after the previous task was archived. The scope is to
finish remaining Rust migration work in coherent module packages. Narrow
slices remain useful when a compatibility boundary is risky, but the default
unit should now be a module-level migration package with explicit validation.

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

### 2026-06-07 Rust-first reset

The next planning unit must begin in `backend-rs`. Recent Python compatibility
cleanup successfully removed several obsolete route compat owners, but that
work should now be treated as fallback shrink evidence, not as the primary
migration lane. The primary lane is Rust owner completion.

The reset changes the package order:

1. `chapter_single_generation` whole-module Rust owner package.
2. `chapter_generation` shared owner package.
3. `chapter_batch_generation` whole-module Rust owner package.
4. `chapters` / Python compatibility shell shrink only after matching Rust
   owner evidence exists.
5. `schema / migration owner` when route packages expose table or field
   ownership pressure.

Package start criteria after the reset:

- name the Rust route/service files that will gain or tighten ownership
- name the Python fallback shell that will remain frozen until Rust validation
  passes
- define the preserved HTTP/SSE/task/checkpoint/error contract
- define the focused Rust tests and `cargo check` command up front
- define the Python fallback shrink step as a follow-up, not the lead change

This means the surviving
`backend/app/services/compat/chapter_generation_route_compat_service.py`
should not simply be moved into another Python route module. It should be used
as the source map for a Rust-first package, primarily
`chapter_single_generation` plus adjacent shared `chapter_generation` owners.

### 2026-06-08 low-analysis acceleration update

The migration has entered a mature owner-consolidation phase. The latest file
audit shows most chapter-generation Python route files are already migrated or
legacy-only; `chapters.py` remains mixed, and `chapter_route_helpers.py` is not
a migration target. Therefore, the next rounds should stop redoing broad
progress analysis unless a concrete route or owner map changed.

Current progress summary:

- Chapter route migration table: 9 migrated route files, 1 mixed shell
  (`chapters.py`), 1 helper-only non-target (`chapter_route_helpers.py`).
- `chapter_draft` now has a coherent Rust owner chain:
  route package, route-facing access, detail read, apply write, and
  history-write contract.
- Existing gateway readiness evidence still reports Rust-owned probes plus
  Python fallback probes; fallback shrink must wait for explicit route parity,
  enabled-path smoke, and rollback evidence.
- Remaining work is not "write missing Rust routes"; it is owner consolidation,
  smoke/rollback hardening, and targeted fallback shrink.

Fast planning rule:

- Use the latest checkpoint and migration table as the baseline.
- Spend at most one short source-map pass per round.
- Then directly migrate one whole owner file/function group/module package.
- Only expand analysis when validation fails, a route map changed, or a new
  schema/gateway boundary appears.

Updated priority after `chapter_draft` owner alignment:

1. `chapter_draft` closeout:
   finish enabled-path smoke / rollback evidence, then decide whether Python
   draft fallback can be frozen or repointed.
2. `chapter_single_generation` closeout:
   finish remaining prepare/write/runtime-state overlap and active enabled-path
   smoke so the generation compatibility shell can shrink.
3. `chapter_generation` shared owners:
   move shared runtime/candidate/quality semantics that are still interpreted
   through compatibility shells into explicit Rust owners.
4. `chapter_batch_generation` package:
   continue only as whole read/write/resume/status/runtime owner blocks, not
   individual helper seams.
5. `schema / migration owner`:
   promote startup/schema assumptions only when a package exposes concrete
   table/field ownership pressure.

### 2026-06-13 owner-collapse strategy update

The owner-collapse acceleration lane has reached a natural boundary. The
remaining `*_owner.rs` files under chapter-generation packages are no longer
mostly forwarding-only child shells. The current surviving set is dominated by
independent semantic owners such as runtime snapshot persistence, context
compaction, quality runtime context normalization, story-repair quality
projection, research payload assembly, and quality profile construction.

This changes the default execution rule for the next rounds:

- stop treating "delete another `*_owner.rs` file" as the default migration
  success metric
- continue owner-file collapse only when the child file is still a true thin
  bridge with 1-2 direct consumers and no meaningful business boundary
- treat independent owner files as valid stable Rust ownership, not as debt
  that must be force-merged for cosmetic file-count reduction
- shift the default acceleration lane to package closeout work:
  single-generation active route closeout, batch-generation active route
  closeout, manifest/health rollback hardening, and explicit Python shell
  freeze or repoint readiness

Current owner-collapse conclusion after the latest pass:

- `chapter_generation_runtime_service/snapshot_persistence_owner.rs` is a
  real persistence owner
- `chapter_generation_runtime_service/context_compaction_owner.rs` is a real
  prompt/runtime context compaction owner
- `chapter_generation_runtime_service/quality_runtime_context_owner.rs` is a
  real runtime quality normalization owner
- `chapter_generation_runtime_service/story_repair_quality_context_owner.rs`
  is a real story-repair quality owner
- `chapter_generation_prompt_service/quality_profile_owner.rs` is a real
  prompt quality profile owner
- `chapter_single_generation_prepare_service/research_payload_owner.rs` is a
  real research payload assembly owner

Therefore the next high-signal migration packages should be chosen from
module-level closeout work instead of from child-owner deletion count.

### 2026-06-13 single-generation bootstrap closeout checkpoint

The first module-closeout move after the owner-collapse strategy reset is now
complete: the Python `chapter_generation_routes` bootstrap path is no longer
part of default FastAPI startup. It has been downgraded to explicit rollback
registration through a dedicated Python config flag.

This narrows the remaining `chapter_single_generation` Python shell work:

- default active startup now aligns better with the Rust readiness story
- the remaining Python files are still kept as source-map / rollback material
- the next closeout step should focus on whether the remaining compat stream
  shells and `chapters.py` references can be frozen, repointed, or deleted as
  one explicit module move with matching rollback policy and business smoke
  evidence

The package should now avoid re-opening the Python bootstrap question unless:

- the rollback flag name or policy changes
- a logged-in business smoke requires Python re-registration
- a deployment rollback story requires stronger operational documentation

After the bootstrap closeout, the next shrink step also became clearer:

- `backend/app/api/chapters.py` should stop depending on
  `chapter_generation_route_compat_service.py` for non-route background
  analysis entrypoints
- the compat service should progressively collapse to explicit rollback/default
  wiring only
- once `chapters.py` and other non-route callers stop consuming compat-only
  helpers, the remaining decision about the compat service becomes a true
  rollback-shell decision instead of a mixed active-runtime dependency

### 2026-06-13 batch-generation bootstrap closeout checkpoint

The next module-closeout move has also been applied to
`chapter_batch_generation`: the Python
`backend/app/api/chapter_batch_generation_routes.py` route group is no longer
part of default FastAPI startup. It is now imported and registered only when
the explicit Python rollback flag is enabled.

This aligns the Python bootstrap state with the existing Rust readiness
evidence:

- active batch generation traffic is owned by
  `backend-rs/src/api/chapter_batch_generation.rs`
- batch create/status/stream/active-list/cancel/resume route behavior is
  covered by the Rust route owner plus read/write/resume/runtime services
- deploy manifest evidence reports `chapter_batch_generation` as Rust-owned
  with no Python fallback probes
- the Python batch route/service files remain source-map and rollback material,
  not default active route ownership

The next batch package step should be an explicit whole-module freeze/delete
review for the batch Python route and service shells. Do not continue by
editing individual Python helper bodies unless the same round also updates the
Rust owner/readiness contract and rollback policy.

Batch bootstrap re-open conditions:

- the rollback flag name or operational rollback policy changes
- a logged-in DB-backed batch smoke requires Python route re-registration
- final deletion/repoint approval is granted for the whole batch Python
  route/service shell package

The stream entry owner has now also crossed that boundary:

- `backend/app/services/chapter_generation/stream/entry_service.py` no longer
  reaches into `chapter_generation_route_compat_service.py` for its default
  background-analysis callback
- both `chapters.py` and the Python stream entry now consume explicit analysis
  owners instead of using the compat shell as an incidental dependency
- the remaining value inside `chapter_generation_route_compat_service.py`
  becomes easier to evaluate as true route rollback/default wiring rather than
  a shared active-runtime dependency bucket

### Primary migration package

`chapter_single_generation` is now the first Rust-first package because it is
the closest match for the remaining active generation compatibility shell and
it gives the next round visible Rust ownership progress. The package should
cover prepare, write, stream, runtime, snapshot, task model, and quality-status
owners as a coherent module.

Current target files include:

- `backend-rs/src/api/chapter_generation_routes.rs`
- `backend-rs/src/services/chapter_single_generation_prepare_service.rs`
- `backend-rs/src/services/chapter_single_generation_write_workflow_service.rs`
- `backend-rs/src/services/chapter_single_generation_stream_entry_service.rs`
- `backend-rs/src/services/chapter_single_generation_stream_workflow_service.rs`
- `backend-rs/src/services/chapter_single_generation_runtime_state_service.rs`

The surviving Python source map for this package includes:

- `backend/app/api/chapter_generation_routes.py`
- `backend/app/api/chapters.py`
- `backend/app/services/compat/chapter_generation_route_compat_service.py`
- `backend/app/services/chapter_generation/stream/entry_service.py`

`chapter_batch_generation` remains a high-value package, but it should follow
the single-generation Rust-first pass unless direct dependency pressure makes
batch work the safer next owner.

Current checkpoint:

- completed one read-side semantics slice in
  `backend-rs/src/services/chapter_batch_generation_status_view_service.rs`
- stream `event_status` is now produced once by
  `resolve_batch_generation_stream_semantics()` and carried through
  `BatchGenerationStreamState`
- focused tests now cover additional terminal/unknown fallback cases

Target package categories:

- read-side status, stream, and fallback normalization
- runtime checkpoint/progress and resume/recover ownership
- workflow-result and task response assembly
- route ownership evidence, fallback shrink readiness, and rollback notes

### Secondary migration package

Chapter route compression remains valid, but the next acceleration step should
bundle route cleanup with the service module that owns the behavior. Route-only
cleanup is useful only when it reduces cutover risk or removes obsolete Python
fallback semantics.

Current recommendation:

- prefer module packages over one-off route compression
- choose package boundaries that can be verified with focused Rust tests plus
  route-group smoke or manifest checks
- allow framework/control-flow adjustments when they consolidate ownership and
  make the module easier to reason about

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
- `chapter_batch_generation_status_semantics_service.rs` now also owns the
  execution-mode literal as a single-branch contract; the previous
  single/batch `match` no longer provided any meaningful ownership split, so
  the helper now exposes a true constant contract instead of a fake
  task-dependent branch
- `chapter_batch_generation_task_payload_base_service.rs` now owns the shared
  checkpoint override path for resume-style response payloads, so response
  adapters no longer patch nested checkpoint fields in place
- `chapter_batch_generation_task_payload_base_service.rs` now also owns the
  shared create/resume task response payload assembly contract for the batch
  module:
  - create and resume now both build response payloads through one shared
    owner boundary
  - loading-stage compatibility fields now come from the same payload owner
    instead of being patched separately in runtime-state response code
  - quality payload, active story-repair payload, quality history context,
    summary fields, and extra compat fields now flow through one options
    contract, which makes the next batch route/fallback cutover step easier to
    audit
- `chapter_batch_generation_task_payload_base_service.rs` now also owns the
  shared read/query task view payload assembly contract for the batch module:
  - read-context status payload, active-project payload, active-task-list item,
    and single-generation existing-background payload now all build through one
    shared owner boundary
  - retry metadata, terminal fields, existing-background task metadata, and
    quality payload injection are now variant-driven payload options instead of
    being reassembled across multiple read-context branches
  - this narrows `chapter_batch_generation_read_context_service.rs` back to
    read-context loading and stream-state projection, which makes the next
    read/stream cutover and fallback audit easier to reason about
- `chapter_batch_generation_status_stream_event_service.rs` now also owns the
  shared status-stream system event contract for the batch module:
  - connected/task-not-found/timeout payloads and heartbeat/data transport
    event builders now live beside the stream event resolution owner
  - `chapter_batch_generation_status_stream_service.rs` is narrower and keeps
    polling / transport orchestration instead of also owning repeated system
    event construction
- stream observation ownership is now also narrower:
  - `BatchGenerationStreamState` now materializes one
    `BatchGenerationStreamObservationKey`
  - `BatchGenerationStreamCursor` now compares that owner-provided observation
    contract instead of locally caching only `status/completed/progress/message`
  - this keeps phase changes, quality-gate projection changes, and
    analysis-started metadata changes on the same stream owner boundary as the
    event batch they drive
- `chapter_batch_generation_command_payload_adapter_service.rs` now owns the
  resume response envelope through a dedicated helper, so the adapter no
  longer hand-builds the outer resume message and totals inline
- cancel and resume command responses now also share one task-summary helper
  for `batch_id/message/completed_chapters/total_chapters`, so the outer
  progress-summary contract is no longer duplicated across two branches
- stream terminal-kind semantics now belong to the stream-state owner:
  `chapter_batch_generation_status_view_service.rs` resolves terminal-kind once
  while building `BatchGenerationStreamState`, and the stream event layer only
  consumes that parsed semantic
- stream semantics ownership is now narrower:
  `resolve_batch_generation_stream_semantics()` returns terminal-kind together
  with progress/message/event-status, so the status-view layer no longer does
  a second terminal-kind lookup for the same status value
- batch owned task-sources ownership is now narrower:
  - `chapter_batch_generation_owned_task_query_service.rs` now exposes a
    shared `OwnedBatchGenerationTaskSources` owner for the lower-level
    `owned task + snapshot` chain
  - the same module now keeps two explicit layers:
    - `load_owned_batch_generation_task_sources(...)`
    - `load_owned_batch_generation_task_read_state(...)`
  - this split is intentional because read/query lanes need
    `task -> recover -> snapshot`, but command lanes such as cancel/resume must
    preserve the current non-recovery semantics
  - the batch module therefore no longer needs to choose between
    duplicated `task + snapshot` loading and accidentally over-sharing the
    recovery owner into command semantics
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
- batch resume restored-launch ownership is now narrower:
  - `RestoredResumeRuntimeStateProjection` exposes `into_launch_parts()` so the
    restored resume owner projects request-runtime state and reset runtime seed
    once
  - `BatchGenerationResumeLaunchPersistencePlan` consumes that owner
    projection instead of cloning the seed locally while dispatch reopens the
    broader restored projection
  - this keeps resume reset persistence and dispatch-plan assembly on the same
    restored owner boundary without changing task lifecycle or response
    payloads
- batch resume restored-state launch ownership is now narrower:
  - `RestoredResumeRuntimeStateProjection` now also owns the final
    batch/single runtime launch materialization through:
    - `prepare_batch_runtime_launch(...)`
    - `prepare_single_chapter_runtime_launch(...)`
  - `chapter_batch_generation_resume_task_command_service.rs` no longer reopens
    `into_launch_parts()` at the command layer and no longer replays
    `request_runtime_state -> runtime_input` assembly after the restored owner
    is already materialized
  - this keeps restored-state projection, launch-input materialization,
    reset-persistence preparation, and dispatch-plan assembly on the same
    restored owner chain without changing resume payloads or task lifecycle
- batch cancel/resume command sources ownership is now narrower:
  - `chapter_batch_generation_cancel_service.rs` no longer replays:
    `load owned task -> load snapshot`
  - `chapter_batch_generation_resume_task_command_service.rs` no longer replays:
    `load owned task -> load snapshot`
  - both neighboring command lanes now consume the shared
    `OwnedBatchGenerationTaskSources` owner directly, while preserving their
    existing error boundary:
    - task lookup failures still map through task errors
    - snapshot load failures still map through domain/config edges as before
  - this keeps the command-side status gating, cancel persistence planning,
    and resume launch preparation on the same lower-level owner chain without
    introducing read-side recovery semantics into cancel/resume
- batch cancel write-workflow ownership is now narrower:
  - `chapter_batch_generation_write_workflow_service.rs` now also owns the
    batch cancel public-start / workflow-start boundary beside create/resume
  - `chapter_batch_generation_cancel_service.rs` is reduced to:
    - owned sources loading
    - terminal status gating
    - cancelled persistence-plan preparation
  - the route no longer treats cancel as a special direct command-service
    branch while create/resume already go through one batch write-workflow
    public-start owner
  - this gives the batch command lane one more consistent module-level owner
    shape:
    - create -> write workflow
    - resume -> write workflow
    - cancel -> write workflow
- batch write-workflow start ownership is now narrower:
  - `chapter_batch_generation_write_workflow_service.rs` no longer keeps a
    second `PreparedBatchGeneration*WorkflowStart` shell for create/resume/
    cancel after the neighboring workflow entry / workflow launch owners are
    already materialized
  - create now starts directly from:
    - `PreparedBatchGenerationCreateWorkflowEntry::start(...)`
  - resume now starts directly from:
    - `PreparedBatchGenerationResumeWorkflowLaunch::start(...)`
  - cancel now starts directly from:
    - `PreparedBatchGenerationCancelWorkflowLaunch::start(...)`
  - this split is intentional because the removed `workflow start` wrappers
    were no longer adding timestamp ownership, validation, branch selection,
    or error translation; they only replayed
    `prepare -> persist_and_dispatch`
  - the batch write lane therefore now keeps one tighter owner chain across
    create / resume / cancel instead of preserving a redundant compatibility
    hop at the public-start neighbor boundary
- batch create workflow-entry ownership is now narrower:
  - `chapter_batch_generation_write_workflow_service.rs` no longer keeps a
    dedicated `PreparedBatchGenerationCreateWorkflowEntry` layer after the
    neighboring create persistence-plan owner is already materialized
  - create now starts directly from:
    - `BatchGenerationCreateLaunchPersistencePlan::start(...)`
  - the same owner now also prepares:
    - `BatchGenerationCreateLaunchPersistencePlan::prepare(...)`
  - this split is intentional because the removed `workflow entry` wrapper
    was no longer adding access checks, request normalization, branch
    selection, or error translation; it only replayed
    `prepare persistence plan -> persist_and_dispatch`
  - the batch create lane therefore now keeps one tighter owner chain:
    `public start -> create persistence-plan owner -> persist-and-dispatch`
    instead of preserving a redundant compatibility hop between the public
    write-workflow entry and the already-ready persistence owner
- batch cancel service file ownership is now narrower:
  - `chapter_batch_generation_write_workflow_service.rs` now also owns the
    remaining batch cancel production chain that was still split across a
    neighboring module file:
    - terminal status validation
    - cancelled persistence-plan preparation from owned sources
    - cancel workflow launch preparation
    - final cancel write-workflow start
  - `chapter_batch_generation_cancel_service.rs` has been removed because it
    no longer owned an independent compatibility boundary after cancel had
    already joined the shared batch write-workflow lane
  - this split is intentional because the removed file was no longer adding
    transport translation, route branching, cross-module policy, or a
    separate rollback seam; it only reopened one more module hop around the
    same batch cancel owner chain
  - the batch cancel lane therefore now keeps one tighter file-local owner
    chain:
    `public cancel start -> owned cancel prepare -> cancelled persistence`
    instead of preserving a redundant compatibility file beside the already
    dominant batch write-workflow owner
- batch stream-state file ownership is now narrower:
  - `chapter_batch_generation_status_stream_service.rs` now also owns the
    remaining batch status-stream production chain that was still split across
    a neighboring module file:
    - shared owned read-state loading
    - stream-state projection from task + snapshot sources
    - status-stream poll loop
    - SSE event emission / close behavior
  - `chapter_batch_generation_stream_state_query_service.rs` has been removed
    because it no longer owned an independent compatibility boundary after the
    stream lane had already collapsed read-state projection and event
    semantics around the same status-stream owner chain
  - this split is intentional because the removed file was no longer adding
    route translation, alternative stream transport, rollback policy, or a
    separate error boundary; it only reopened one more module hop around the
    same owned read-state -> stream projection contract
  - the batch status-stream lane therefore now keeps one tighter file-local
    owner chain:
    `owned read-state -> stream state -> poll / emit`
    instead of preserving a redundant compatibility file beside the already
    dominant status-stream owner
- batch status-query file ownership is now narrower:
  - `chapter_batch_generation_read_context_service.rs` now also owns the
    remaining batch status-query production chain that was still split across
    a neighboring module file:
    - shared owned read-state loading for status routes
    - task + snapshot -> quality-context materialization
    - final status payload projection
  - `chapter_batch_generation_status_task_query_service.rs` has been removed
    because it no longer owned an independent compatibility boundary after the
    read-side owner chain had already converged around shared read-context and
    payload projection semantics
  - this split is intentional because the removed file was no longer adding
    route translation, alternate query transport, rollback policy, or a
    separate error contract; it only reopened one more module hop around the
    same owned read-state -> status payload contract
  - the batch status-query lane therefore now keeps one tighter file-local
    owner chain:
    `owned read-state -> status payload`
    instead of preserving a redundant compatibility file beside the already
    dominant read-context owner
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
- single-chapter write-workflow request ownership is now narrower:
  - `chapter_single_generation_write_workflow_service.rs` now owns the
    route-payload -> write-workflow request conversion through one shared
    helper instead of repeating the same conversion in background and stream
    entrypoints
  - this keeps the route-compatible request shape unchanged while reducing
    one more write-lane owner split before runtime launch
- single-chapter runtime lifecycle ownership is now narrower:
  - `chapter_single_generation_runtime_state_service.rs` now owns an explicit
    `SingleGenerationRuntimeLifecyclePlan`
  - the same owner now sequences:
    `persist preparing -> execute generation -> run follow-up analysis ->
    persist completed/manual-review/failed`
  - runtime dispatch no longer reopens a separate
    `dispatch -> execute_single_generation_runtime(...)` wrapper chain, which
    keeps the background/resume runtime handoff closer to one lifecycle owner
    boundary
- single-chapter runtime direct generation-analysis ownership is now narrower:
  - `SingleGenerationRuntimeLifecyclePlan` now directly owns the active
    `generation execution -> follow-up analysis -> manual-review/completed/
    failed persistence` chain
  - the single runtime lane no longer reopens
    `execute_owned_single_chapter_generation(...)`,
    `run_single_generation_follow_up_analysis(...)`, or
    `maybe_fail_single_generation_for_quality_gate_manual_review(...)`
    as free-helper handoff hops beside the lifecycle owner
  - this keeps single runtime launch input, generation execution, analysis
    routing, and terminal persistence on the same production owner boundary
    without changing quality-gate or checkpoint semantics
- single-chapter stream workflow public-start ownership is now narrower:
  - `chapter_single_generation_stream_workflow_service.rs` now owns an
    explicit `SingleGenerationStreamWorkflowStart`
  - the same owner now sequences:
    `prepare restored runtime launch -> hand off to lifecycle.spawn`
  - the outer public stream entry no longer reopens
    `prepare -> into_runtime_launch_input -> from_runtime_launch(...).spawn(...)`
    as a repeated handoff chain
- single-chapter restored-launch materialization ownership is now narrower:
  - `PreparedSingleChapterGenerationRestoredRuntimeLaunch` now also exposes
    direct owner materialization for neighboring production lanes:
    - `prepare_runtime_launch_input(...)`
    - `prepare_background_launch_parts_from_target(...)`
  - `chapter_single_generation_stream_workflow_service.rs` no longer reopens
    `prepare(...).into_runtime_launch_input()` at the stream workflow edge
  - `chapter_single_generation_write_workflow_service.rs` no longer reopens
    `prepare_from_target(...).into_background_launch_parts(task_id)` at the
    background write-workflow edge
  - this keeps restored-launch preparation, startup snapshot planning,
    background response/task-seed assembly, and runtime launch materialization
    on the same restored-launch owner chain without changing payloads or task
    lifecycle semantics
- single-chapter prepare/runtime owner chain is now narrower:
  - `PreparedSingleChapterGenerationRestoredRuntimeLaunch::prepare_from_target(...)`
    now directly owns the validated request/target -> restored-launch
    materialization chain; the previous `prepare_validated_*` wrapper hop has
    been removed
  - `SingleGenerationRuntimeLaunchInput` now directly owns
    `execute_generation(...)`, and both runtime lifecycle and stream lifecycle
    consume that owner method instead of a separate
    `execute_single_generation_runtime_generation(...)` free helper
  - this keeps validated prepare, runtime launch materialization, and direct
    generation execution on the same single-generation owner chain without
    changing task lifecycle, SSE ordering, or payload semantics
- single-chapter background launch-parts persistence ownership is now narrower:
  - `PreparedSingleGenerationBackgroundLaunchParts` now also owns:
    `persist_and_dispatch(...)`
  - the same owner now sequences:
    `task insert -> startup snapshot persist -> runtime dispatch ->
    response payload`
  - `chapter_single_generation_write_workflow_service.rs` no longer keeps
    a neighboring free helper that reopens the final persistence/dispatch
    chain after the launch-parts owner is already fully materialized
  - this keeps background task-seed assembly, startup snapshot ownership,
    runtime launch input, and final persistence/dispatch on the same
    production owner boundary without changing payloads or task lifecycle
    semantics
- single-chapter stream success analysis-projection ownership is now narrower:
  - `SingleGenerationStreamAnalysisOutcome` now also owns:
    - `from_generated_result(...)`
    - `run_follow_up_analysis(...)`
    - `quality_metrics_event(...)`
    - `quality_gate_event(...)`
    - `analysis_started_event(...)`
    - `response_payload(...)`
  - `SingleGenerationStreamCompletionProjection` now consumes one explicit
    analysis owner result instead of reopening free helper hops for follow-up
    analysis execution and success payload/event reconstruction
  - this keeps stream success analysis, quality-event projection,
    analysis-started projection, and terminal response payload assembly on the
    same production owner chain without changing SSE ordering or payload
    semantics
- single-chapter stream success owner chain is now narrower again:
  - `SingleGenerationStreamAnalysisOutcome` now also owns:
    - `completion_message()`
    - `ordered_success_event_payloads(...)`
    - `emit_success(...)`
  - `chapter_single_generation_stream_workflow_service.rs` no longer keeps a
    neighboring `SingleGenerationStreamCompletionProjection` owner between
    `analysis outcome` and `complete -> quality events -> result ->
    analysis-started -> done`
  - this keeps follow-up analysis, completion projection, ordered success
    emission, and terminal SSE close on the same production owner chain
    without changing SSE ordering or payload semantics
- single-chapter runtime checkpoint ownership is now narrower:
  - `chapter_single_generation_runtime_state_service.rs` now also owns:
    - `SingleGenerationSnapshotStage`
    - `build_single_generation_runtime_checkpoint_for_stage(...)`
  - `chapter_single_generation_prepare_service.rs` now consumes that runtime
    owner directly for pending checkpoint projection
  - the neighboring
    `chapter_single_generation_runtime_checkpoint_service.rs` file has been
    removed instead of preserving one more module hop around the same
    `snapshot stage -> checkpoint payload` projection contract
  - this keeps single runtime task mutation, runtime snapshot persistence, and
    checkpoint payload projection on the same production owner chain without
    changing payload semantics
- single-chapter existing-background query ownership is now narrower:
  - `chapter_single_generation_write_workflow_service.rs` now also owns:
    - `load_active_single_generation_background_tasks(...)`
    - `load_owned_single_generation_existing_background_task_payload(...)`
  - the existing-background branch no longer reopens one neighboring batch
    task-view query entrypoint before returning to the single background write
    owner
  - `chapter_batch_generation_task_view_query_service.rs` now keeps only the
    batch active-task-list / active-project query lanes, while the
    single-background existing-task query lane has been pulled back into the
    single-generation write owner
  - this keeps target loading, existing-task short-circuit selection, and the
    final compat payload consumer on the same single-generation production
    owner chain without changing payload semantics
- single-chapter existing-background payload ownership is now narrower again:
  - `chapter_single_generation_write_workflow_service.rs` now also owns:
    - `into_single_generation_existing_background_task_payload(...)`
  - the existing-background branch no longer reopens one neighboring batch
    read-context projection seam before returning the final compat payload
  - `chapter_batch_generation_read_context_service.rs` now keeps only the
    remaining batch shared read-context payload owners, while the
    single-generation-specific existing-background payload projection has been
    pulled back into the single-generation write owner
  - this keeps active-task query selection, chapter match filtering, and the
    final compat payload projection on the same single-generation production
    owner chain without changing payload semantics
- single-chapter existing-background payload variant ownership is now narrower again:
  - `chapter_single_generation_write_workflow_service.rs` now also owns the
    single-generation-specific existing-background payload field assembly on
    top of the shared task-view payload base
  - the existing-background branch no longer reopens one neighboring batch
    payload-base variant before returning the final compat payload
  - `chapter_batch_generation_task_payload_base_service.rs` now keeps only the
    remaining batch shared task-view payload variants, while the
    single-generation-specific existing-background payload variant has been
    pulled back into the single-generation write owner
  - this keeps shared task-view base projection, quality-context insertion,
    and the final single-generation existing-background payload shape on the
    same single-generation production owner chain without changing payload
    semantics
- single-chapter existing-background read-context ownership is now narrower again:
  - `chapter_single_generation_write_workflow_service.rs` now also owns:
    - `SingleGenerationExistingBackgroundTaskContext`
    - `load_active_single_generation_existing_background_task_contexts(...)`
    - `single_generation_existing_background_task_contains_chapter(...)`
  - the existing-background branch no longer reopens one neighboring batch
    read-context owner chain before reaching the final single-generation
    payload projection
  - `chapter_batch_generation_read_context_service.rs` now keeps only the
    remaining batch shared read-context owner lanes, while the
    single-generation-specific existing-background read-state/context chain has
    been pulled back into the single-generation write owner
  - this keeps active-task recovery, snapshot-backed quality-context
    preparation, chapter match filtering, and the final existing-background
    payload projection on the same single-generation production owner chain
    without changing payload semantics
- single-chapter background payload base ownership is now narrower again:
  - `chapter_single_generation_prepare_service.rs` now also owns:
    - `estimated_single_generation_task_minutes(...)`
    - `single_generation_pending_stage_code()`
    - `single_generation_active_task_statuses()`
    - `build_single_generation_runtime_payload_base(...)`
    - `build_single_generation_task_view_payload_from_task_state(...)`
  - `chapter_single_generation_write_workflow_service.rs` now consumes that
    local payload base owner directly for existing-background payloads instead
    of reopening neighboring batch task-view/status semantics
  - the background create response payload and the existing-background
    short-circuit payload now share one single-generation-local base contract
    for task/runtime fields, stage semantics, execution mode, and estimated
    duration semantics
  - this keeps background create payload projection and existing-task payload
    projection on the same single-generation production owner chain without
    changing payload semantics
- single-chapter quality-status ownership is now narrower again:
  - `chapter_single_generation_quality_status_service.rs` now owns:
    - `SingleGenerationQualityStatusContext`
    - `SingleGenerationQualityStatusContext::from_snapshot_and_runtime_state(...)`
    - `SingleGenerationQualityStatusContext::insert_into_payload(...)`
    - `manual_review_label_from_single_generation_quality_context(...)`
  - `chapter_single_generation_write_workflow_service.rs` no longer reopens
    a neighboring batch quality-status semantic shell for existing-background
    quality payload projection
  - `chapter_single_generation_runtime_state_service.rs` no longer reopens
    a neighboring batch quality-status helper for runtime manual-review label
    resolution
  - this keeps chapter-scoped quality payload reconstruction and
    single-generation manual-review label semantics on the same
    single-generation production owner chain without changing payload
    semantics
- single-chapter route workflow-start ownership is now narrower:
  - `chapter_generation_routes.rs` no longer rebuilds
    `SingleChapterGenerationRequest` locally before calling neighboring
    background/stream workflows
  - `chapter_single_generation_write_workflow_service.rs` now exposes
    `start_owned_single_generation_background_write_workflow_from_route_payload(...)`
    directly, and the background write lane no longer keeps
    `SingleGenerationBackgroundWorkflowRouteStart` /
    `SingleGenerationBackgroundWorkflowStart` as extra wrapper hops around the
    workflow-entry owner
  - `chapter_single_generation_stream_workflow_service.rs` now exposes
    `create_single_generation_stream_workflow_from_route_payload(...)`
    directly, and the stream lane no longer keeps
    `SingleGenerationStreamWorkflowRouteStart` or a separate
    `SingleGenerationStreamWorkflowStart::start(...)` wrapper around the same
    prepare/spawn owner chain
  - this keeps route-payload normalization and workflow public-start handoff
    on the same background/stream owner boundary while leaving the HTTP route
    transport-only
- batch-create route workflow-start ownership is now narrower:
  - `chapter_batch_generation.rs` no longer rebuilds
    `BatchGenerationCreateWorkflowRequest` locally before calling the
    neighboring batch create write workflow
  - `chapter_batch_generation_write_workflow_service.rs` now exposes
    `build_batch_generation_create_workflow_request_from_route_payload(...)` and
    `start_owned_batch_generation_write_workflow_from_route_payload(...)`
  - this keeps route-payload normalization and workflow public-start handoff
    on the same batch-create owner boundary while leaving the HTTP route
    transport-only
- batch create route-start ownership is now narrower:
  - `chapter_batch_generation_write_workflow_service.rs` no longer keeps a
    neighboring `BatchGenerationCreateWorkflowRouteStart` shell after the
    route edge already hands transport payload directly into the batch create
    write-workflow owner chain
  - the create lane now stops at:
    - `build_batch_generation_create_workflow_request_from_route_payload(...)`
    - `start_owned_batch_generation_write_workflow_from_route_payload(...)`
    - `start_owned_batch_generation_write_workflow(...)`
  - this split is intentional because the removed route-start wrapper was no
    longer adding validation, access control, error translation, or branch
    selection; it only replayed `route payload -> workflow request -> start`
    into the same neighboring write-workflow owner chain
  - the batch create route lane therefore now keeps one tighter owner shape
    across route-payload normalization and workflow public-start handoff
    instead of preserving a Python-era compatibility hop beside the
    already-materialized write-workflow boundary
- batch active-task-list route-query ownership is now narrower:
  - `chapter_batch_generation.rs` no longer rebuilds
    `ActiveBatchGenerationTaskListQueryRequest` locally before calling the
    neighboring active-task query workflow
  - `chapter_batch_generation_task_view_query_service.rs` now exposes
    `ActiveBatchGenerationTaskListRouteQuery`,
    `build_active_batch_generation_task_list_query_request_from_route_query(...)`,
    and
    `load_active_user_batch_generation_task_list_view_from_route_query(...)`
  - `chapter_batch_generation_error_mapper.rs` now also owns the shared
    route-query error mapping through
    `map_active_batch_generation_task_list_route_error(...)`
  - this keeps route-query normalization, active-task query start, and
    request/query error ownership on the same batch active-query boundary
    while leaving the HTTP route transport-only
- batch active-project route-query ownership is now narrower:
  - `chapter_batch_generation.rs` no longer directly replays the
    `project_id -> active-project query` handoff before calling the
    neighboring active-project query workflow
  - `chapter_batch_generation_task_view_query_service.rs` now exposes
    `ActiveProjectBatchGenerationRouteError` and
    `load_active_batch_generation_view_from_route_project(...)`
  - `chapter_batch_generation_error_mapper.rs` now also owns the shared
    route-query error mapping through
    `map_active_project_batch_generation_route_error(...)`
  - this keeps route-project handoff, active-project query start, and
    project-access/query error ownership on the same batch active-project
    boundary while leaving the HTTP route transport-only
- batch task-view route-start ownership is now narrower:
  - `chapter_batch_generation_task_view_query_service.rs` no longer keeps
    neighboring `ActiveBatchGenerationTaskListRouteStart` or
    `ActiveProjectBatchGenerationRouteStart` shells after the route edges
    already hand query/path transport inputs directly into route-query owners
  - the active-task-list lane now stops at:
    - `ActiveBatchGenerationTaskListRouteQuery`
    - `build_active_batch_generation_task_list_query_request_from_route_query(...)`
    - `load_active_user_batch_generation_task_list_view_from_route_query(...)`
  - the active-project lane now stops at:
    - `load_active_batch_generation_view_from_route_project(...)`
    - `ActiveProjectBatchGenerationRouteError`
  - this split is intentional because the removed route-start wrappers were no
    longer adding validation, access control, error translation, or branch
    selection; they only replayed route-normalized values into the same query
    owner chain
  - the batch task-view query lane therefore now keeps one tighter owner
    shape across active-task-list and active-project reads instead of
    preserving a Python-era compatibility hop beside already-materialized
    route-query owners
- batch owned read-state ownership is now narrower:
  - `chapter_batch_generation_status_task_query_service.rs` and
    `chapter_batch_generation_stream_state_query_service.rs` no longer keep
    parallel copies of the same owned `task -> recover -> snapshot` read
    chain
  - `chapter_batch_generation_owned_task_query_service.rs` now exposes
    `OwnedBatchGenerationTaskReadState` and
    `load_owned_batch_generation_task_read_state(...)`
  - status-payload projection and stream-state projection now both consume
    that shared owner state instead of each reopening owned-task recovery plus
    snapshot load independently
  - this keeps owned-task load, active-timeout recovery, and snapshot
    materialization on one shared batch read-state boundary before the
    neighboring status-payload and status-stream owners diverge
- batch status/stream read-state projection ownership is now narrower:
  - `chapter_batch_generation_status_task_query_service.rs` no longer keeps a
    neighboring `PreparedOwnedBatchGenerationStatusPayloadQuery` shell after
    the status lane already consumes one shared
    `OwnedBatchGenerationTaskReadState`
  - `chapter_batch_generation_stream_state_query_service.rs` no longer keeps a
    separate `build_batch_generation_stream_state_from_read_state(...)` hop
    after the stream lane already consumes the same shared read-state owner
  - the status lane now stops at:
    - `load_owned_batch_generation_status_payload(...)`
    - `build_owned_batch_generation_status_payload_from_read_state(...)`
  - the stream lane now stops at:
    - `load_owned_batch_generation_stream_state(...)`
    - `OwnedBatchGenerationTaskReadState::into_parts()`
    - `build_batch_generation_stream_state_for_task_and_snapshot(...)`
  - this split is intentional because the removed wrappers were no longer
    adding validation, access control, recovery, or branch selection; they
    only replayed one already-materialized shared read-state into the same
    neighboring payload/stream projections
  - the batch status-query / status-stream lanes therefore now keep one tighter
    owner shape after the shared owned read-state boundary instead of
    preserving a second Python-era compatibility hop beside already-explicit
    final projection owners
- batch task-view prepared-query ownership is now narrower:
  - `chapter_batch_generation_task_view_query_service.rs` no longer keeps
    neighboring query/view wrappers after the task-view lane already owns both
    direct active-task loading and final payload projection:
    - `PreparedActiveBatchGenerationTaskListView`
    - `PreparedActiveProjectBatchGenerationQuery`
    - `PreparedExistingSingleGenerationBackgroundTaskPayloadQuery`
  - the active-task-list lane now stops at:
    - `load_active_user_batch_generation_task_list_view(...)`
    - `build_active_batch_generation_task_list_view_payload(...)`
  - the active-project lane now stops at:
    - `load_active_batch_generation_query(...)`
    - `build_active_project_batch_generation_view_payload(...)`
  - the existing single-background branch now stops at:
    - `load_existing_single_generation_background_task_payload(...)`
    - `load_existing_single_generation_background_task_payload_for_tasks(...)`
  - this split is intentional because the removed wrappers were no longer
    adding access control, request validation, error translation, or branch
    selection; they only replayed `prepare -> into_payload` after the same
    task-view owner had already loaded the relevant active task set
  - the batch task-view query lane therefore now keeps one tighter owner shape
    across active-task-list, active-project, and existing-background query
    branches instead of preserving a second Python-era compatibility hop
    beside already-materialized final payload projections
- batch resume launch-sources ownership is now narrower:
  - `chapter_batch_generation_resume_task_command_service.rs` no longer keeps
    a neighboring `PreparedBatchGenerationResumeLaunchSources` shell after the
    resume lane already owns:
    - restored runtime-state materialization
    - manual-review blocker detection
    - validated execution selection
    - launch-persistence plan assembly
  - the batch resume lane now stops at:
    - `prepare_resume_launch_restored_state(...)`
    - `BatchGenerationResumeLaunchPersistencePlan::prepare(...)`
    - `BatchGenerationResumeLaunchPersistencePlan::prepare_from_validated_execution(...)`
  - this split is intentional because the removed wrapper was no longer adding
    error translation, access control, request validation, or dispatch branch
    selection; it only replayed
    `prepare restored state -> into launch persistence plan`
    after the same command owner had already materialized the required resume
    sources
  - the batch resume command lane therefore now keeps one tighter owner chain
    from restored-state recovery into final launch-persistence materialization
    instead of preserving a Python-era compatibility hop beside the already
    explicit resume persistence owner
- batch write-workflow execution-config ownership is now narrower:
  - `chapter_batch_generation_write_workflow_service.rs` now prepares the
    explicit `AIConfig + provider_payload` execution config before handing off
    to runtime launch for both create and resume flows
  - `chapter_batch_generation_runtime_launch_service.rs` now consumes that
    prepared config as an explicit input and narrows to launch assembly plus
    dispatch, instead of reloading execution config inside the launch owner
  - this keeps create/resume payloads and runtime behavior stable while making
    the write-workflow boundary consistent with single-chapter generation
- batch create workflow-launch persistence ownership is now narrower:
  - `PreparedBatchGenerationCreateWorkflowLaunch` now also owns direct
    persistence-plan materialization through
    `prepare_persistence_plan(...)`
  - `PreparedBatchGenerationCreateWorkflowEntry::prepare(...)` no longer
    reopens `prepare(...).into_persistence_plan(...)` at the create workflow
    edge after the workflow-launch owner is already materialized
  - this keeps create launch preparation, startup snapshot planning,
    response/task-seed assembly, and persistence-plan materialization on the
    same batch-create owner chain without changing payloads or task lifecycle
    semantics
- single-generation startup snapshot ownership is now narrower:
  - `chapter_single_generation_snapshot_service.rs` now owns the full
    chapter-scoped startup snapshot contract through
    `SingleGenerationStartupSnapshotPlan`
  - `chapter_single_generation_prepare_service.rs` and
    `chapter_single_generation_write_workflow_service.rs` no longer reopen
    that owner from the neighboring batch snapshot file
  - `chapter_batch_generation_snapshot_service.rs` now keeps only the
    remaining batch-shared snapshot owners plus persistence helpers
  - this keeps restored-launch preparation, startup snapshot planning,
    quality/runtime restore payloads, and snapshot persistence on the same
    single-generation owner chain without changing payloads or task lifecycle
    semantics
- single-generation existing-background query file ownership is now narrower:
  - `chapter_single_generation_existing_background_query_service.rs` now owns
    the existing-background task lookup, recovered read-state loading, and
    compat payload projection in one dedicated single-generation file
  - `chapter_single_generation_write_workflow_service.rs` no longer mixes
    workflow-entry branching with the full
    `active task -> snapshot -> payload` owner chain inline
  - this keeps existing-background query/load/projection semantics on the
    same single-generation owner chain while leaving write-workflow focused on
    branch selection plus launch/persist-and-dispatch behavior
- single-generation snapshot persistence / merge ownership is now narrower:
  - `chapter_single_generation_snapshot_service.rs` now also owns the
    chapter-scoped runtime-state merge and snapshot upsert boundary through:
    - `merge_single_generation_runtime_state(...)`
    - `upsert_single_generation_runtime_snapshot(...)`
  - `chapter_single_generation_runtime_state_service.rs` no longer reopens
    `project_merged_batch_generation_runtime_state(...)` or
    `upsert_batch_generation_runtime_snapshot(...)` from the neighboring batch
    snapshot owner chain
  - this keeps startup snapshot planning, single runtime checkpoint
    persistence, and snapshot merge semantics on one clearer
    single-generation owner boundary without changing payloads or task
    lifecycle semantics
- single-generation task-model ownership is now narrower:
  - `chapter_single_generation_task_model_service.rs` now owns the
    chapter-scoped task insert seed and runtime task mutation boundary
    through:
    - `SingleGenerationTaskPersistenceSeed`
    - `build_single_generation_background_task_persistence_seed(...)`
    - `build_single_generation_background_task_active_model(...)`
    - `SingleGenerationTaskStage`
  - `chapter_single_generation_prepare_service.rs` no longer reopens batch
    task seed semantics for the single background create lane
  - `chapter_single_generation_runtime_state_service.rs` no longer keeps a
    file-local copy of the single task stage mutation contract
  - this keeps single background task insertion, runtime task-stage
    persistence, and chapter-scoped task-model semantics on one clearer
    single-generation owner boundary without changing payloads or task
    lifecycle semantics
- chapter-generation shared snapshot-query / task-recovery ownership is now narrower:
  - `chapter_generation_snapshot_query_service.rs` now owns the shared
    chapter-generation snapshot load/query boundary through:
    - `load_chapter_generation_snapshot(...)`
    - `load_chapter_generation_snapshot_map(...)`
  - `chapter_generation_task_recovery_service.rs` now owns the shared
    generation-task timeout auto-recovery boundary through:
    - `resolve_generation_task_auto_recovery_error(...)`
    - `recover_generation_task_if_needed(...)`
  - single-generation existing-background query and the neighboring batch
    read/query/runtime owners now consume that shared lower-level owner
    directly instead of leaving single-generation production lanes attached
    to batch file names for the same lower-level semantics
  - this keeps batch/single module owners behavior-preserving while making
    the shared lower-level owner chain explicit and easier to audit for
    fallback shrink, rollback, and stronger smoke readiness
- chapter-generation shared chapter-access ownership is now narrower:
  - `chapter_generation_access_service.rs` now owns the shared
    chapter-generation access boundary through:
    - `LoadAccessibleChapterForGenerationError`
    - `load_accessible_chapter_for_generation(...)`
    - `load_accessible_chapters_for_generation(...)`
  - `chapter_generation_runtime_service.rs`,
    `chapter_batch_generation_resume_task_command_service.rs`,
    `chapter_single_generation_prepare_service.rs`,
    `chapter_single_generation_stream_workflow_service.rs`, and
    `chapter_generation_error_mapper.rs` now consume that shared lower-level
    owner directly instead of leaving non-batch production lanes attached to
    a batch-named access file for the same lower-level semantics
  - `chapter_batch_generation_access_service.rs` has been removed because it
    no longer owned a real batch-only compatibility boundary after the access
    semantics were shown to be shared lower-level chapter-generation
    ownership
  - this keeps batch/single module owners behavior-preserving while making
    the shared lower-level access chain explicit and easier to audit for
    fallback shrink, rollback, and stronger smoke readiness
- chapter-generation shared quality runtime-context persisted-source ownership is now narrower:
  - `chapter_generation_snapshot_persistence_service.rs` no longer routes its
    persisted quality-column backfill through a batch-named quality runtime
    context owner
  - the shared snapshot persistence owner now consumes:
    - `resolve_generation_quality_runtime_context_from_persisted_sources("batch", ...)`
    from `chapter_generation_quality_runtime_context_service.rs`
  - the shared generation quality owner now carries an explicit batch-scope
    regression proving the persisted-source rebuild contract still preserves
    batch summary-state / history semantics
  - this keeps batch/single module owners behavior-preserving while making
    the shared lower-level persisted quality chain explicit and easier to
    audit for fallback shrink, rollback, and stronger smoke readiness
- chapter-generation shared snapshot persistence ownership is now narrower:
  - `chapter_generation_snapshot_persistence_service.rs` now owns the shared
    chapter-generation snapshot merge / replace persistence boundary through:
    - `ChapterGenerationSnapshotWriteMode`
    - `merge_chapter_generation_runtime_state(...)`
    - `persist_chapter_generation_runtime_snapshot(...)`
    - `upsert_chapter_generation_runtime_snapshot(...)`
  - `chapter_batch_generation_snapshot_service.rs` now keeps only batch-local
    queued/resume snapshot plan ownership plus the batch public API surface,
    while delegating lower-level snapshot writes into that shared owner
  - `chapter_single_generation_snapshot_service.rs` now delegates directly
    into that shared persistence owner instead of routing the chapter-scoped
    write path back through the batch snapshot file
  - this keeps batch/single module owners behavior-preserving while making
    the shared lower-level write chain explicit and easier to audit for
    fallback shrink, rollback, and stronger smoke readiness
- batch runtime public-start ownership is now narrower:
  - `chapter_batch_generation_runtime_state_service.rs` now lets
    `BatchGenerationRuntimeLifecyclePlan::start(...)` own the public runtime
    handoff directly
  - batch runtime dispatch no longer reopens
    `dispatch -> execute_batch_generation_runtime(...) -> runtime driver ->
    lifecycle.execute(...)` as a repeated wrapper chain
  - this keeps batch runtime dispatch and lifecycle sequencing on the same
    runtime owner boundary without changing task lifecycle or checkpoint
    semantics
- batch runtime post-analysis ownership is now narrower:
  - `PreparedBatchGenerationStepExecution::execute_success_chain(...)` now
    hands the follow-up analysis result directly to
    `BatchGenerationPostAnalysisTerminalPlan`
  - the success lane no longer reopens
    `run_follow_up_analysis(...) -> resolve_analysis_outcome(...)` as a local
    forwarding chain after the analysis owner is already explicit
  - this keeps batch runtime success, follow-up analysis, and terminal
    post-analysis routing on the same production owner boundary without
    changing retry, quality-gate, or persistence semantics
- batch runtime analysis-attempt ownership is now narrower:
  - `BatchGenerationAnalysisAttemptPlan::execute(...)` now directly owns
    prepared-analysis selection, started-snapshot persistence, prepared vs
    fallback execution, and resolution handoff
  - the analysis-attempt lane no longer reopens
    `persist_started(...)` in one branch and
    `execute_prepared_or_fallback(...)` in another local wrapper chain
  - this keeps batch runtime follow-up analysis attempt preparation and
    execution on the same production owner boundary without changing analysis
    retry or completion semantics
- batch runtime analysis-attempt resolution ownership is now narrower:
  - `BatchGenerationAnalysisAttemptPlan::execute(...)` now also directly owns
    completion vs retry resolution after prepared/fallback analysis returns
  - the analysis-attempt lane no longer materializes a separate
    `BatchGenerationAnalysisAttemptResolutionPlan` neighbor for the same
    production handoff
  - this keeps batch runtime follow-up analysis attempt execution and
    completion/retry routing on the same production owner boundary without
    changing analysis retry budget or completion snapshot semantics
- batch runtime terminal quality-gate ownership is now narrower:
  - `BatchGenerationPostAnalysisTerminalPlan` now directly owns quality-gate
    retry-budget loading and terminal routing on the success path
  - the terminal lane no longer materializes a separate
    `BatchGenerationQualityGateResolutionPlan` neighbor for the same
    production handoff
  - this keeps batch runtime post-analysis terminal routing and quality-gate
    retry/manual-review resolution on the same production owner boundary
    without changing retry or failure semantics
- batch runtime lifecycle-step ownership is now narrower:
  - `PreparedBatchGenerationStepExecution::start(...)` now directly owns the
    retry-aware step entry on the production runtime lane
  - the lifecycle lane no longer reopens
    `prepare step -> carry retry -> execute prepared step` as a local wrapper
    chain inside `BatchGenerationRuntimeLifecyclePlan::execute(...)`
  - this keeps batch runtime step preparation, retry carry, and prepared-step
    execution on the same production owner boundary without changing
    prerequisite, generation, or terminal persistence semantics
- batch runtime step-generation-attempt ownership is now narrower:
  - `PreparedBatchGenerationStepExecution::execute(...)` now directly owns the
    full generation-attempt chain on the production runtime lane
  - the prepared-step lane no longer reopens
    `execute_generation_attempt(...)` or `execute_success_chain(...)` as local
    wrapper chains around the same production path
  - this keeps batch runtime chapter-started persistence, prerequisite gate,
    attempt-input preparation, generation execution, post-write guard,
    follow-up analysis, and terminal routing on the same production owner
    boundary without changing retry, quality-gate, or persistence semantics
- batch runtime attempt-input generation ownership is now narrower:
  - `BatchGenerationAttemptInputPlan::execute(...)` now directly owns the
    full `attempt-input materialization -> generation execution` chain on the
    production runtime lane
  - the prepared-step lane no longer reopens
    `prepare attempt input -> local generate_and_persist... call` after the
    attempt-input owner is already explicit
  - this keeps batch runtime compat restore, prompt overrides, provider
    payload preparation, and generation execution on the same production owner
    boundary without changing prerequisite, retry, post-write guard, or
    terminal routing semantics
- analysis trigger ownership is now narrower:
  - `PreparedChapterAnalysisTrigger` no longer carries the raw chapter model,
    because dispatch only needs the task and chapter identifiers plus the
    prepared payload
  - this trims one unused field from the prepared trigger contract without
    changing the runtime dispatch path
- 2026-05-28 迁移提速判断已更新：
  - 当前速度慢的根因不再主要是“Rust 代码切片推进不够”，而是三层进度
    没有并行推进：
    1. API owner 收口
    2. Python fallback 收缩准备
    3. schema / migration owner 切换
  - 因此本 follow-up 后续不应再只以“删了多少 wrapper / helper”衡量进展，
    而要把 seam 收口明确绑定到 cutover readiness 上
  - 每轮 seam 改动都应尽量回答至少一个问题：
    - 是否让某个 route group 更接近 fallback 收缩？
    - 是否让 stronger smoke 更容易补？
    - 是否让 rollback 边界更清晰？
    - 是否让 schema assumption 更明确？
  - 后续执行要从“单线 seam 微收口”切换为“三线并行”：
    - 线 A：继续 `chapter_generation` / `chapter_batch_generation` /
      `chapters` 邻域的高价值 seam 收口
    - 线 B：同步补 route-group owner / fallback / rollback /
      stronger-smoke cutover 包
    - 线 C：开始把 schema / migration owner 从远期债务前移为显式执行线
  - 当前任务仍以线 A 为主，但每轮都应尽量产出对线 B / 线 C 有帮助的
    结构化结论，而不是只做局部代码美化

### Module Package Strategy

The earlier micro-slice approach was useful while owner boundaries were still
unclear. It is now slowing the migration because each tiny move repeats
context loading, documentation, and validation overhead. Future execution must
use whole-file, whole-function-group, or whole-module migration packages as the
default unit:

- package A: `chapter_generation` shared lower-level owner package
- package B: `chapter_single_generation` prepare/write/stream/runtime package
- package C: `chapter_batch_generation` read/write/resume/status/runtime package
- package D: `chapters` compatibility shell and route delegation cleanup
- package E: `schema / migration owner` when route packages expose table or
  field assumptions that must stop depending on Python startup behavior

Each package should include:

- entrypoint map: Python routes, Rust routes, services, task tables, smoke
  probes, and rollback knobs
- behavior contract: HTTP payloads, SSE events, task lifecycle, checkpoint
  shape, default provider behavior, and failure semantics
- implementation plan: move or rewrite the Python-owned logic into Rust
  services first, then remove fallback/delegation only after owner evidence is
  strong enough
- quality gate: focused tests for changed service behavior, `cargo check`, and
  route-group smoke/manifest validation when transport ownership changes
- maintainability gate: cohesive module names, small readable functions,
  explicit error types, and short comments for non-obvious lifecycle or
  compatibility decisions

Small slices are still allowed inside a package when they reduce review risk,
but a package should not be considered complete until the whole module's
behavior, smoke coverage, rollback story, and remaining Python dependency are
documented together.

### 2026-06-06 Whole-Module Acceleration Strategy

The execution model is now package-first. A package can contain several commits
or validation checkpoints, but the planning target must be a complete file,
function group, or module capability. The package owner decides the boundary
before code changes start, then keeps all package edits pointed at that
boundary until it is either completed or explicitly paused.

Priority table:

| Priority | Package | Rust target files | Python / fallback target | Done means |
|----------|---------|-------------------|--------------------------|------------|
| A | `chapter_generation` shared owners | `chapter_generation_*` shared access, snapshot, recovery, quality, runtime-context services | Batch-named helpers that are actually shared generation semantics | Shared lower-level owners no longer live under batch-only file names unless they are truly batch-only. |
| B | `chapter_single_generation` | `chapter_single_generation_prepare_service.rs`, `chapter_single_generation_write_workflow_service.rs`, `chapter_single_generation_stream_workflow_service.rs`, `chapter_single_generation_runtime_state_service.rs`, `chapter_single_generation_snapshot_service.rs`, `chapter_single_generation_task_model_service.rs`, `chapter_single_generation_quality_status_service.rs` | `chapter_generation_routes.py` and single-branch logic inside `chapters.py` compatibility shells | Single-chapter generation can be audited as one Rust-owned prepare/write/stream/runtime module with explicit fallback and smoke evidence. |
| C | `chapter_batch_generation` | `chapter_batch_generation_read_context_service.rs`, `chapter_batch_generation_status_stream_service.rs`, `chapter_batch_generation_runtime_state_service.rs`, `chapter_batch_generation_write_workflow_service.rs`, `chapter_batch_generation_resume_task_command_service.rs`, task/query/status services | `chapter_batch_generation_routes.py` compatibility shell and batch branches in `chapters.py` | Batch create/resume/read/status/stream/runtime is owned as one coherent Rust package with route parity and rollback notes. |
| D | `chapters` compatibility shell | `backend-rs/src/api/chapters.rs`, CRUD/generation/regeneration/analysis route owners, request/response adapters | `backend/app/api/chapters.py` | Python shell is either frozen as explicit fallback or shrunk only where Rust route parity, smoke, and rollback are proven. |
| E | `schema / migration owner` | Rust migration readiness docs, model/migration owners, startup assumptions | Python Alembic startup and legacy schema mutation assumptions | Table/field ownership no longer depends on implicit Python app startup behavior for package-owned flows. |

Package start gate:

- Write the Python source map before editing Rust code.
- Write the Rust target map and decide whether the package is file-level,
  function-group-level, or module-level.
- List the preserved behavior contract: HTTP response shell, SSE event order,
  task lifecycle, runtime checkpoint, provider defaults, and error mapping.
- List package validation commands and smoke/manifest checks.
- List rollback/cutover evidence and the remaining compatibility shell.

Package stop rule:

- Do not start another standalone seam while the selected package still has
  unresolved owner, fallback, smoke, rollback, or schema assumptions.
- Do not count helper relocation as migration progress unless it removes or
  freezes a Python dependency, shrinks a fallback shell, clarifies cutover, or
  makes a stronger package-level validation possible.
- Micro-slices are allowed only as review checkpoints inside the selected
  package, not as the thing being planned or reported.

### Paused seam

`chapter_quality` remains paused unless direct dependency pressure appears.

## Design Principles

1. Preserve compatibility first

- Keep status vocabulary, checkpoint semantics, SSE event categories, and
  default provider behavior stable.
- Moving logic across boundaries is allowed only when the observable result
  remains unchanged.

2. Choose one package at a time

- Each package should be understandable and reviewable as a coherent module.
- Avoid overlapping unrelated packages in one implementation batch.
- Within a package, allow multiple coordinated file edits when they retire a
  real Python dependency or remove repeated compatibility logic.

3. Prefer service-owned contracts

- Route files should only orchestrate transport concerns.
- Repeated fallback assembly, progress calculation, status adaptation, and
  task semantics belong in focused service helpers.

4. Maintainability and robustness are migration requirements

- Code should remain readable, cohesive, and robust after the migration.
- Add concise comments only for non-obvious runtime, fallback, checkpoint, or
  rollback behavior.
- If a larger framework or control-flow adjustment materially reduces
  migration drag and preserves behavior, prefer that over another narrow
  wrapper move.

5. Stop at diminishing returns

- If the next move only relocates code without reducing compatibility risk,
  clarifying ownership, or helping a package reach cutover readiness, stop and
  leave a checkpoint.

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

Updated file-level migration map after the latest audit:

| Python file | Rust counterpart | Status | Planning note |
|-------------|-------------------|--------|---------------|
| `chapter_analysis_task_routes.py` | `backend-rs/src/api/chapter_analysis_routes.rs` | Migrated | Python shell can be removed when route parity is no longer needed. |
| `chapter_annotation_routes.py` | `backend-rs/src/api/chapter_crud_routes.rs` + `chapter_annotation_query_service.rs` | Migrated | Rust owns the query boundary; Python is now legacy compatibility surface. |
| `chapter_batch_generation_routes.py` | `backend-rs/src/api/chapter_batch_generation.rs` | Migrated | Main chapter-generation/batch surface is already on Rust. |
| `chapter_draft_routes.py` | `backend-rs/src/api/chapter_draft_routes.rs` + `chapter_draft_*` services | Migrated | Draft transport, route-facing access, detail, apply, and history-write contracts are now Rust-owned; Python is source map/fallback reference only. |
| `chapter_expansion_plan_routes.py` | `backend-rs/src/api/chapter_crud_routes.rs` + CRUD workflow/request services | Migrated | Expansion-plan behavior now belongs to Rust write workflow. |
| `chapter_generation_routes.py` | `backend-rs/src/api/chapter_batch_generation.rs` + single-generation services | Migrated | Single-chapter generation route shell is legacy only. |
| `chapter_partial_regeneration_routes.py` | `backend-rs/src/api/chapter_regeneration_routes.rs` | Migrated | Partial-regeneration route shell is legacy only. |
| `chapter_quality_routes.py` | `backend-rs/src/api/chapter_crud_routes.rs` + quality services | Migrated | Quality-trend behavior is already Rust-owned. |
| `wizard_stream.py` | `backend-rs/src/api/wizard.rs` | Migrated | Wizard stream functionality now has a Rust entrypoint. |
| `chapters.py` | mixed across multiple Rust route/service modules | Partial / mixed | Treat as seam cleanup only; do not count as fresh route migration debt. |
| `chapter_route_helpers.py` | helper only | Not a migration target | Shared helper, excluded from migration accounting. |

Planning implication:

- The remaining Python files are mostly compatibility shells, not open Rust
  capability gaps.
- The only clearly mixed boundary that still deserves careful seam analysis is
  `backend/app/api/chapters.py`.
- Latest audit update:
  `backend/app/api/chapters.py` still contains many helper-style wrappers, but
  they primarily delegate to already-owned Rust or compatibility services.
  The file should be treated as a compatibility shell unless a concrete
  Rust-owned replacement becomes available for a specific branch.
- Current stop-rule update:
  the remaining useful phase 5 work is now mostly in Rust-side semantic
  cleanup and route-delegation polish; `chapters.py` is no longer a good
  primary migration target for this task.
- Additional stop-rule update:
  do not reintroduce nested payload patching in response adapters when the
  shared payload base can accept the override once.
- Additional stop-rule update:
  do not rebuild resume envelope totals/messages inline in the adapter when a
  dedicated helper can own that shape once.
- Additional stop-rule update:
  do not hand-maintain duplicate cancel/resume task-summary fields in separate
  response builders when one shared helper can own the outer contract.
- Additional stop-rule update:
  do not let SSE event builders re-parse terminal task status when the
  stream-state owner can carry terminal-kind once.
- Additional stop-rule update:
  do not let the status-view layer perform a second terminal-kind lookup after
  stream semantics were already resolved from the same status value.
- Additional stop-rule update:
  do not duplicate the same route-payload -> domain-request conversion across
  sibling write-workflow entrypoints when one workflow-local helper can own
  that boundary once.
- Additional stop-rule update:
  do not let runtime-launch helpers fetch user execution config implicitly
  when the write-workflow owner can prepare and pass the explicit config once.
- Additional stop-rule update:
  do not keep unused prepared fields in trigger contracts when downstream
  dispatch only consumes identifiers and payload.
- Additional stop-rule update:
  do not keep finalized regeneration helper fields that production consumers
  never read when the payload owner already carries the externally observable
  stream contract.
- Additional stop-rule update:
  do not keep prepared regeneration fields that are only used to build the
  prompt when downstream stream/runtime owners already consume the derived
  prompt and numeric execution parameters only.
- Additional stop-rule update:
  do not keep internal source-classification enums in quality-metrics source
  helpers when the production query path already consumes only the resolved
  metrics and identifiers.
- Additional stop-rule update:
  do not keep a default prompt-context wrapper when execution-config ownership
  can depend directly on the placeholder payload owner.
- Additional stop-rule update:
  do not keep a batch execution-input constructor when the runtime launch
  owner already has the full input fields and can construct the struct once.
- Additional stop-rule update:
  when a shared lower-level generation seam is proven to belong to an existing
  Rust owner file, merge it into that owner and cut consumers directly to the
  merged owner; do not preserve or expand a compatibility shim just to keep the
  old file name alive.
- Latest package checkpoint update:
  `chapter_generation_request_runtime_state_service.rs` should now be treated
  as a shrinking compatibility shim, not a long-term owner. The real owner for
  batch request runtime-state shape is
  `chapter_generation_execution_contract_service.rs`, and subsequent migration
  slices should continue deleting shim consumers rather than adding new ones.
- Latest package checkpoint update:
  the request-runtime-state shim deletion is now complete. The file
  `chapter_generation_request_runtime_state_service.rs` has been removed after
  the last consumer was cut to
  `chapter_generation_execution_contract_service.rs`. Future shared-owner work
  should reuse this pattern: owner merge first, whole-file deletion second.
- Latest package checkpoint update:
  the target-word-count shim deletion is now complete. The file
  `chapter_generation_target_word_count_service.rs` has been removed after its
  default/minimum target-word-count semantics and normalization helper were
  merged into `chapter_generation_execution_contract_service.rs`, then all
  active consumers were cut directly to that merged owner.
- Latest package checkpoint update:
  the next shared-owner acceleration pass should continue selecting seams that
  already behave like subordinate runtime helpers for one stronger owner file.
  Current candidate: verify whether
  `chapter_generation_terminal_runtime_patch_service.rs` is now effectively a
  subordinate runtime-state seam for
  `chapter_batch_generation_runtime_state_service.rs`; if yes, apply the same
  owner-merge-then-delete pattern instead of preserving another shared shim.
- Latest package checkpoint update:
  the terminal-runtime-patch shim deletion is now complete. The file
  `chapter_generation_terminal_runtime_patch_service.rs` has been removed after
  its terminal manual-review/retry patch logic and owner contract were merged
  into `chapter_batch_generation_runtime_state_service.rs`, then the remaining
  smoke/contract consumers were cut to that merged owner.
- Latest package checkpoint update:
  the `chapter_draft` detail helper shrink is now complete. The file
  `chapter_draft_detail_service.rs` has been removed after its candidate/auto-
  revision detail payload builders and loaders were merged into
  `chapter_draft_route_service.rs`, which is now the single route-facing Rust
  owner for draft access, selection, detail payload assembly, and rollback
  contract publication.
- Latest package checkpoint update:
  the `chapter_draft` route-owner file relocation is now complete. The former
  route-facing helper file `backend-rs/src/services/chapter_draft_route_service.rs`
  has been retired after its full access/selection/detail/apply/readiness owner
  was moved beside the actual route boundary under
  `backend-rs/src/api/chapter_draft_routes.rs`. The temporary intermediate
  route-owner seam `backend-rs/src/api/chapter_draft_route_owner.rs` has also
  now been retired after its full route-facing owner was merged back into the
  real route file in the same API package. This keeps the route transport
  owner and route-facing draft payload owner on one file boundary, which
  matches the route-owner consolidation rule and removes one extra API seam
  without changing the draft apply/load behavior.
- Latest package checkpoint update:
  the `chapter_analysis` shared task-state helper shrink is now complete. The
  file `chapter_analysis_task_state_service.rs` has been removed after its
  shared analysis task lifecycle state machine was merged into
  `chapter_analysis_service.rs`, which now owns both shared analysis error
  types and shared task-state transitions for runtime/query consumers.
- Latest package checkpoint update:
  the `chapter_analysis` shared read-context helper shrink is now complete.
  The file `chapter_analysis_read_context_service.rs` has been removed after
  its shared candidate-attempt + recent-history loader was merged into
  `chapter_analysis_service.rs`, which now also owns the shared analysis
  read-context consumed by analysis view, quality metrics, and
  single-generation runtime-restore lanes.
- Latest package checkpoint update:
  the `chapter_analysis` character-state helper shrink is now complete. The
  file `chapter_analysis_character_state_service.rs` has been removed after
  its character/organization analysis sync implementation was merged into
  `chapter_analysis_runtime_service.rs`, which is now the single runtime owner
  for post-persist analysis follow-up sync on the `character_states` and
  `organization_states` branches.
- Latest package checkpoint update:
  the `chapter_regeneration` stream helper shrink is now complete. The files
  `chapter_regeneration_stream_launch_service.rs` and
  `chapter_regeneration_text_service.rs` have been removed after their SSE
  launch/finalize implementation was merged into
  `chapter_regeneration_stream_workflow_service.rs`, which is now the single
  workflow owner for full/partial regeneration stream launch, progress
  materialization, finalize cleanup, and stable finalize error semantics.
- Latest package checkpoint update:
  the `chapter_regeneration` query helper shrink is now complete. The file
  `chapter_regeneration_query_service.rs` has been removed after its route
  query normalization, owned task-list payload loading, datetime formatting,
  and owner contract/tests were merged into
  `chapter_regeneration_routes.rs`, which is now the single route-facing Rust
  owner for regeneration task query bounds, access-checked task listing, and
  rollback contract publication.
- Latest package checkpoint update:
  the `chapter_regeneration` apply helper shrink is now complete. The file
  `chapter_regeneration_apply_service.rs` has been removed after its route
  payload coercion, partial-apply validation, chapter-slice replacement,
  persistence contract, and owner contract/tests were merged into
  `chapter_regeneration_routes.rs`, which is now the single route-facing Rust
  owner for partial-regeneration apply bounds, access-checked chapter update,
  and rollback contract publication.
- Latest package checkpoint update:
  the `chapter_single_generation` candidate-quality helper shrink is now
  complete. The file
  `chapter_single_generation_candidate_quality_service.rs` has been removed
  after its single-generation candidate quality runtime-context builder,
  story-quality metric heuristics, continuity preflight, and quality-gate plan
  logic were merged into `chapter_generation_runtime_service.rs`, which is now
  the single Rust owner for the active single-generation candidate runtime plus
  its quality-policy callbacks.
- Latest package checkpoint update:
  the `chapter_single_generation` active gateway smoke helper shrink is now
  complete. The file
  `chapter_single_generation_active_gateway_smoke_service.rs` has been removed
  after its active-route smoke suite, readiness payload projection, owner
  contract, and focused tests were merged into `backend-rs/src/api/health.rs`,
  which is now the real route-facing Rust owner for the single-generation
  active gateway health/readiness evidence.
- Latest package checkpoint update:
  the `chapter_batch_generation` active gateway smoke helper shrink is now
  complete. The file
  `chapter_batch_generation_active_gateway_smoke_service.rs` has been removed
  from the `services` owner lane after its batch active-route smoke suite,
  readiness payload projection, owner contract, and focused tests were moved
  under the route-facing health owner boundary in
  `backend-rs/src/api/health.rs` plus
  `backend-rs/src/api/health/chapter_batch_generation_active_gateway_smoke_owner.rs`.
- Latest package checkpoint update:
  the `chapter_batch_generation` candidate-event helper shrink is now
  complete. The file `chapter_candidate_event_service.rs` has been removed
  after its two remaining real owner responsibilities were split back into the
  actual consumers:
  `chapter_batch_generation_read_context_stream_progress_owner.rs` now owns
  stream progress event projection, and
  `chapter_batch_generation_runtime_selected_candidate_event_owner.rs` now owns
  selected-candidate snapshot plus chunk-event projection. The runtime-facing
  batch owner remains
  `backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs`,
  and route-facing readiness evidence remains under
  `backend-rs/src/api/health/chapter_batch_generation_active_gateway_smoke_owner.rs`.
- Latest package checkpoint update:
  the shared `chapter_generation` quality-gate semantics helper shrink is now
  complete. The file
  `backend-rs/src/services/chapter_generation_quality_gate_semantics_service.rs`
  has been removed after its manual-review / retry-budget label semantics were
  merged into the stronger shared owner
  `backend-rs/src/services/chapter_generation_quality_runtime_context_service.rs`.
  The remaining consumers now read quality-gate semantics from the same shared
  runtime-context owner that already owns terminal quality normalization,
  persisted quality-context rebuild, and batch/single quality payload shaping.
- Latest package checkpoint update:
  the next shared-owner acceleration pass should keep selecting seams that have
  already collapsed to one real runtime owner plus evidence-only consumers.
  The goal is not "move helper code somewhere else"; the goal is "merge the
  helper into the real owner and delete the extra file in the same round."
- Future Phase 5 work should prefer removing the remaining legacy shells only
  if the Rust route/service owner already exists and the change reduces drift,
  not if it merely renames wrappers.

## Validation Strategy

- Run `cargo check` after each completed slice with the shared target dir:
  `$env:CARGO_TARGET_DIR='E:/Code/ProjectsCode/WorkSpace/Codex/NovelAi/MuMuNovel/.codex-targets/shared'; cargo check`
- Add focused unit tests when extracting or tightening pure helpers.
- Prefer narrow regression protection in touched service files over broad test
  churn.
- For candidate-event owner removal rounds, validate both the focused Rust
  owner chain and manifest readiness:
  `cargo test chapter_batch_generation_read_context_service`,
  `cargo test chapter_batch_generation_runtime_state_service`,
  `cargo test api::health`,
  `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "E:/Code/ProjectsCode/WorkSpace/Codex/NovelAi/MuMuNovel/.codex-targets/chapter-candidate-event-delete"`,
  and
  `python backend/tools/run_strangler_gateway_smoke.py --validate-manifest-only`.
- For shared quality-owner merge rounds, validate both direct semantics
  consumers and manifest readiness:
  `cargo test services::chapter_generation_quality_runtime_context_service`,
  `cargo test services::chapter_batch_generation_task_payload_base_service`,
  `cargo test services::chapter_batch_generation_resume_task_command_service`,
  `cargo test services::chapter_single_generation_runtime_state_service`,
  `cargo test services::chapter_story_repair_quality_context_service`,
  `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "E:/Code/ProjectsCode/WorkSpace/Codex/NovelAi/MuMuNovel/.codex-targets/chapter-generation-quality-runtime-owner-merge"`,
  and
  `python backend/tools/run_strangler_gateway_smoke.py --validate-manifest-only`.

## Rollback Shape

- Roll back only the latest seam slice if validation fails.
- Do not mix unrelated refactor moves in the same execution batch.

## Latest Shared-Owner Checkpoint

- Latest package checkpoint update:
  the shared `chapter_generation` task-semantics helper shrink is now
  complete. The file
  `backend-rs/src/services/chapter_generation_task_semantics_service.rs`
  has been removed after its two remaining real owner responsibilities were
  merged into the actual batch owners:
  - `backend-rs/src/services/chapter_batch_generation_read_context_service.rs`
    now owns `active_batch_generation_statuses()`
  - `backend-rs/src/services/chapter_batch_generation_task_payload_base_service.rs`
    now owns `BatchGenerationTaskKind`, `batch_generation_task_kind(...)`,
    `task_kind(...)`, `batch_generation_task_type(...)`, and `task_type(...)`
- Remaining consumers now read active batch status semantics from the
  read-context owner and task-kind / task-type semantics from the shared batch
  payload owner that already serves runtime-state, write-workflow, resume, and
  read/query projections.
- Route-facing readiness evidence has been updated in:
  - `backend-rs/src/api/health.rs`
  - `backend-rs/src/api/health/chapter_batch_generation_active_gateway_smoke_owner.rs`
  so deleted forwarding-only semantics files are no longer named as active
  Rust owners.
- Validation evidence for this merge:
  - `cargo test services::chapter_batch_generation_read_context_service --manifest-path "backend-rs/Cargo.toml" --target-dir "E:/Code/ProjectsCode/WorkSpace/Codex/NovelAi/MuMuNovel/.codex-targets/chapter-generation-task-semantics-delete" -- --nocapture` -> 49 passed
  - `cargo test services::chapter_batch_generation_task_payload_base_service --manifest-path "backend-rs/Cargo.toml" --target-dir "E:/Code/ProjectsCode/WorkSpace/Codex/NovelAi/MuMuNovel/.codex-targets/chapter-generation-task-semantics-delete" -- --nocapture` -> 37 passed
  - `cargo test services::chapter_batch_generation_resume_task_command_service --manifest-path "backend-rs/Cargo.toml" --target-dir "E:/Code/ProjectsCode/WorkSpace/Codex/NovelAi/MuMuNovel/.codex-targets/chapter-generation-task-semantics-delete" -- --nocapture` -> 65 passed
  - `cargo test services::chapter_batch_generation_runtime_state_service --manifest-path "backend-rs/Cargo.toml" --target-dir "E:/Code/ProjectsCode/WorkSpace/Codex/NovelAi/MuMuNovel/.codex-targets/chapter-generation-task-semantics-delete" -- --nocapture` -> 112 passed
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "E:/Code/ProjectsCode/WorkSpace/Codex/NovelAi/MuMuNovel/.codex-targets/chapter-generation-task-semantics-delete"` -> passed
  - `python backend/tools/run_strangler_gateway_smoke.py --validate-manifest-only` -> passed
- Active Rust source scan is clean for
  `chapter_generation_task_semantics_service`; no active `backend-rs/src`
  references remain after the same-round owner merge and file deletion.
- Latest package checkpoint update:
  the `chapter_batch_generation` projection-owner collapse is now complete.
  The forwarding-only Rust files
  `backend-rs/src/services/chapter_batch_generation_read_context_stream_progress_owner.rs`
  and
  `backend-rs/src/services/chapter_batch_generation_runtime_selected_candidate_event_owner.rs`
  have been removed after their only real production responsibilities were
  merged into the stronger owners that already consume them:
  - `backend-rs/src/services/chapter_batch_generation_read_context_service.rs`
    now owns the stream progress event projection contract and payload builder
  - `backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs`
    now owns the selected-candidate event projection contract, snapshot view,
    progress event, and chunk event batch builder
- Route-facing readiness evidence has been updated in:
  - `backend-rs/src/api/health/chapter_batch_generation_active_gateway_smoke_owner.rs`
  - `backend-rs/src/services/chapter_candidate_runtime_state_service.rs`
  - `backend-rs/src/services/mod.rs`
  so the deleted projection-only files are no longer named as active Rust
  owners or target files.
- Validation evidence for this merge:
  - `cargo test chapter_batch_generation_read_context_service --manifest-path "backend-rs/Cargo.toml" --target-dir "E:/Code/ProjectsCode/WorkSpace/Codex/NovelAi/MuMuNovel/.codex-targets/batch-projection-owner-collapse" -- --nocapture` -> 49 passed
  - `cargo test chapter_batch_generation_runtime_state_service --manifest-path "backend-rs/Cargo.toml" --target-dir "E:/Code/ProjectsCode/WorkSpace/Codex/NovelAi/MuMuNovel/.codex-targets/batch-projection-owner-collapse" -- --nocapture` -> 112 passed
  - `cargo test api::health --manifest-path "backend-rs/Cargo.toml" --target-dir "E:/Code/ProjectsCode/WorkSpace/Codex/NovelAi/MuMuNovel/.codex-targets/batch-projection-owner-collapse" -- --nocapture` -> 16 passed
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "E:/Code/ProjectsCode/WorkSpace/Codex/NovelAi/MuMuNovel/.codex-targets/batch-projection-owner-collapse"` -> passed
  - `python backend/tools/run_strangler_gateway_smoke.py --validate-manifest-only` -> passed
- Active Rust source scan is clean for both deleted files; no active
  `backend-rs/src` references remain after the same-round owner merge and file
  deletion.
- Latest package checkpoint update:
  the shared `chapter_generation` execution-config helper shrink is now
  complete. The file
  `backend-rs/src/services/chapter_generation_execution_config_service.rs`
  has been removed after its remaining owner responsibilities were merged back
  into the stronger shared owner
  `backend-rs/src/services/chapter_generation_execution_contract_service.rs`.
- The merged owner now keeps both shared execution-contract semantics and the
  execution-config bridge contract:
  - `PreparedGenerationExecutionConfig`
  - `prepare_generation_execution_config(...)`
  - `prepare_generation_execution_config_with_provider_payload(...)`
  - `build_generation_execution_config_owner_contract()`
- Remaining consumers now read execution-config ownership directly from
  `chapter_generation_execution_contract_service.rs`, including:
  - `chapter_single_generation_prepare_service.rs`
  - `chapter_single_generation_runtime_restore_service.rs`
  - `chapter_single_generation_runtime_state_service.rs`
  - `chapter_single_generation_stream_workflow_service.rs`
  - `chapter_single_generation_write_workflow_service.rs`
  - `chapter_batch_generation_write_workflow_service.rs`
  - `chapter_batch_generation_runtime_state_service.rs`
  - `chapter_batch_generation_resume_task_command_service.rs`
  - `backend-rs/src/api/health.rs`
  - `backend-rs/src/api/health/chapter_batch_generation_active_gateway_smoke_owner.rs`
- Route-facing readiness evidence and prompt-context source maps have been
  updated so the deleted forwarding-only file is no longer named as an active
  Rust owner or target file.
- Validation evidence for this merge:
  - `cargo test chapter_generation_execution_contract_service --manifest-path "backend-rs/Cargo.toml" --target-dir "E:/Code/ProjectsCode/WorkSpace/Codex/NovelAi/MuMuNovel/.codex-targets/execution-config-owner-collapse" -- --nocapture` -> 9 passed
  - `cargo test chapter_single_generation_prepare_service --manifest-path "backend-rs/Cargo.toml" --target-dir "E:/Code/ProjectsCode/WorkSpace/Codex/NovelAi/MuMuNovel/.codex-targets/execution-config-owner-collapse" -- --nocapture` -> 29 passed
  - `cargo test chapter_batch_generation_write_workflow_service --manifest-path "backend-rs/Cargo.toml" --target-dir "E:/Code/ProjectsCode/WorkSpace/Codex/NovelAi/MuMuNovel/.codex-targets/execution-config-owner-collapse" -- --nocapture` -> 78 passed
  - `cargo test chapter_batch_generation_runtime_state_service --manifest-path "backend-rs/Cargo.toml" --target-dir "E:/Code/ProjectsCode/WorkSpace/Codex/NovelAi/MuMuNovel/.codex-targets/execution-config-owner-collapse" -- --nocapture` -> 112 passed
  - `cargo test api::health --manifest-path "backend-rs/Cargo.toml" --target-dir "E:/Code/ProjectsCode/WorkSpace/Codex/NovelAi/MuMuNovel/.codex-targets/execution-config-owner-collapse" -- --nocapture` -> 16 passed
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "E:/Code/ProjectsCode/WorkSpace/Codex/NovelAi/MuMuNovel/.codex-targets/execution-config-owner-collapse"` -> passed
  - `python backend/tools/run_strangler_gateway_smoke.py --validate-manifest-only` -> passed
- Active Rust source scan is clean for
  `chapter_generation_execution_config_service`; no active `backend-rs/src`
  references remain after the same-round owner merge and file deletion.
- Latest package checkpoint update:
  the `chapter_batch_generation` route-facing health smoke helper shrink is
  now complete. The file
  `backend-rs/src/api/health/chapter_batch_generation_active_gateway_smoke_owner.rs`
  has been removed after its only real production responsibility was merged
  into the stronger route-facing owner
  `backend-rs/src/api/health.rs`.
- The health owner now keeps both active gateway smoke suites in one API
  boundary:
  - `chapter_single_generation_active_gateway_smoke_owner` remains inline
  - `chapter_batch_generation_active_gateway_smoke_owner` is now also inline
- This matches the route gateway smoke/readiness consolidation rule: public
  health endpoint shape stays stable, but the deleted helper file is no longer
  counted as a standalone active Rust owner.
- Validation evidence for this merge:
  - `cargo test api::health --manifest-path "backend-rs/Cargo.toml" --target-dir "E:/Code/ProjectsCode/WorkSpace/Codex/NovelAi/MuMuNovel/.codex-targets/health-batch-owner-inline" -- --nocapture` -> 16 passed
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "E:/Code/ProjectsCode/WorkSpace/Codex/NovelAi/MuMuNovel/.codex-targets/health-batch-owner-inline"` -> passed
  - `python backend/tools/run_strangler_gateway_smoke.py --validate-manifest-only` -> passed
- Active Rust source scan is clean for
  `chapter_batch_generation_active_gateway_smoke_owner.rs`; remaining hits are
  only the expected inline module name inside `backend-rs/src/api/health.rs`.

## 2026-06-13 active route-wiring extraction checkpoint

The next `chapter_single_generation` closeout move is now clearer after the
bootstrap and compat-dependency shrink rounds: the surviving Python
`chapter_generation_route_compat_service.py` should no longer own the active
default route wiring for single-generation stream/background routes.

This checkpoint formalizes a new split:

- active owner:
  `backend/app/services/chapter_generation/route_wiring_service.py`
- legacy source-map / rollback shell:
  `backend/app/services/compat/chapter_generation_route_compat_service.py`

The design reason is straightforward:

- `chapter_generation_routes.py` is still an active route file, so its default
  wiring owner should live beside the active `chapter_generation` module
  instead of a broad `compat` bucket
- the wiring contract is large enough to be its own owner boundary:
  context-builder overrides, prompt/template overrides, runtime prompt
  wiring, candidate rerank wiring, analysis callback wiring, and background
  orchestration wiring all move together as one file-level package
- keeping this contract in `compat` made the compat shell look like an active
  production owner even after bootstrap gating and stream/chapters dependency
  shrink had already reduced its legitimate role

Resulting design rule for the next rounds:

- treat `route_wiring_service.py` as the active Python source-map owner while
  the legacy Python route files still exist
- treat `chapter_generation_route_compat_service.py` as a rollback/source-map
  shell only; do not add new active wiring or runtime dependencies to it
- when the next freeze/delete round is ready, the remaining decision is now
  about whether the legacy compat shell can be frozen or removed entirely,
  not about where active single-generation route semantics live

## 2026-06-13 Rust readiness evidence sync checkpoint

After the Python active route owner moved to
`backend/app/services/chapter_generation/route_wiring_service.py`, the Rust
single-generation owner contracts and health/readiness evidence were still
describing the old compat module as if it were the only route-wiring seam.

This checkpoint closes that drift:

- route-owner, prepare-owner, runtime-restore-owner, stream-workflow-owner,
  and write-workflow-owner contracts now list
  `route_wiring_service.py` in the Python source map
- the active single-generation readiness evidence in
  `backend-rs/src/api/health.rs` now distinguishes:
  - `active_route_wiring_shells`
  - `compat_shells`
- the frozen source-map module list now reflects the actual Python module
  structure that remains after the route-wiring extraction round

Why this matters:

- future freeze/delete decisions for the remaining Python single-generation
  shell must rely on accurate Rust readiness evidence
- otherwise the project will keep carrying stale rollback/source-map facts and
  may delete or freeze the wrong boundary first
- this is a Rust-side migration-closeout task: it makes the Rust owner and
  readiness layer authoritative about the current Python fallback/source-map
  topology

Next design implication:

- the `chapter_single_generation` background write owner-collapse round is now
  complete
- completed whole-file closeout:
  `backend-rs/src/services/chapter_single_generation_write_workflow_service.rs`
  has been deleted after its remaining active production logic moved into the
  surviving owner
  `backend-rs/src/services/chapter_single_generation_runtime_restore_service.rs`
- the surviving Rust owner now keeps one coherent background chain in a single
  file-level boundary:
  existing-task lookup, restored snapshot payload projection, route-facing
  entry branching, launch-part preparation, persistence, and dispatch
- route-facing wiring, health/readiness evidence, and shared owner/source-map
  references have already been repointed to
  `chapter_single_generation_runtime_restore_service.rs`, so the deleted write
  owner should not be scheduled again as a next-step candidate
- do not force-merge
  `runtime_restore_service.rs`
  and
  `runtime_state_service.rs`
  in the immediate next round
- both surviving files still hold dense runtime/snapshot/checkpoint/terminal
  semantics and are not forwarding-only shells
- the next acceleration round should instead choose another whole-file or
  whole-module owner that is still glue-heavy enough to collapse without
  weakening readability or runtime ownership clarity, with
  `chapter_single_generation_stream_workflow_service.rs`
  or the next shared `chapter_generation` glue owner as the stronger
  candidates

## 2026-06-13 fast migration execution plan

This section is the working plan for the next migration rounds. Treat it as
the fast-development baseline: future rounds should start from one of the
lanes below, confirm the local file boundary, edit the Rust owner package, and
record only the new delta and validation evidence.

### First-principles progress model

Migration progress is counted by active ownership, not by raw file movement.
A round counts as real Python-to-Rust migration only when it advances at least
one of these outcomes:

- active route or runtime traffic is owned by a Rust route/service owner
- a Python fallback branch becomes frozen, repointed, removed, or explicitly
  rollback-only
- a Rust owner becomes cohesive enough to delete a forwarding-only Rust helper
  in the same round
- manifest, health, or smoke evidence stops naming stale Python fallback or
  deleted Rust helper files as active targets
- schema, startup, checkpoint, or task lifecycle assumptions move into an
  explicit Rust owner with focused validation

Do not count a round as primary migration progress if it only renames wrappers,
moves a trivial helper, or edits Python compatibility code before Rust owner
validation exists.

### Fast lane order

1. `chapter_single_generation` stream workflow closeout.

   Target the stream side as the next visible whole-file / whole-function-group
   migration unit:
   `backend-rs/src/services/chapter_single_generation_stream_workflow_service.rs`.
   The purpose is to determine whether any remaining stream response,
   lifecycle, or launch glue can move into stronger owners, or whether the file
   should remain the single coherent stream workflow owner. Do not force a
   merge into `runtime_restore_service.rs` or `runtime_state_service.rs`;
   those are dense runtime owners, not cleanup targets.

2. `chapter_single_generation` Python freeze / delete decision.

   After the stream workflow owner is confirmed, convert the existing readiness
   state into one module-level Python shell action. Candidate scope:
   `backend/app/api/chapter_generation_routes.py`,
   `backend/app/services/chapter_generation/route_wiring_service.py`,
   `backend/app/services/compat/chapter_generation_route_compat_service.py`,
   and the `backend/app/services/chapter_generation/stream/*` source-map
   files. The expected result is a documented freeze, repoint, or deletion
   decision, not more active Python route logic.

3. Shared `chapter_generation` Rust owner cleanup.

   Continue deleting only forwarding-only shared helpers whose responsibilities
   have collapsed into one real owner. Strong candidates are helper files or
   submodules that only project runtime/prompt/quality data into a single
   consumer and do not own transport shaping, persistence, rollback policy, or
   a separate validation boundary. Keep
   `chapter_generation_execution_contract_service.rs`,
   `chapter_generation_runtime_service.rs`, and
   `chapter_generation_prompt_service.rs` as stronger owners unless a focused
   pass proves a child module is only evidence or forwarding glue.

4. `chapter_batch_generation` owner reduction.

   Batch generation is already in the high-complexity stage. Treat
   `chapter_batch_generation_read_context_service.rs`,
   `chapter_batch_generation_runtime_state_service.rs`,
   `chapter_batch_generation_write_workflow_service.rs`, and
   `chapter_batch_generation_resume_task_command_service.rs` as real owners by
   default. Only collapse code that has become a forwarding-only projection or
   duplicate task/status lookup. Do not split or merge the large runtime owner
   just for file-count reduction.

5. Business-owner source-map closeout.

   For already validated business route groups (`auth`, `book_import`,
   `characters`, `wizard-stream`, `writing_styles`, and adjacent mature
   profiles), the next useful work is not another auth-guard smoke. The useful
   work is deciding whether the Python source-map files can be frozen,
   repointed, or deleted as a whole group after Rust business probes and
   rollback notes are current.

### Per-round execution contract

Each migration round should be small in number of files but large in ownership
impact. Before editing, record only these five facts:

- selected lane and package
- Python source-map files that are affected
- Rust owner files that will survive after the round
- behavior contract that must not change
- validation commands, using a non-`C:` target dir for Rust build artifacts

Default validation shape:

- focused Rust tests for the touched owner files
- `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "<non-C target>"`
- `python backend/tools/run_strangler_gateway_smoke.py --validate-manifest-only`
  when route, manifest, health, or readiness evidence changes
- focused Python tests only when a Python rollback/source-map shell or test
  patch surface changes

### Speed rules

- Start from this plan and the latest checkpoint; do not re-analyze the whole
  migration history every round.
- Prefer whole-file or whole-function-group edits. Micro-slices are allowed
  only inside the selected lane as risk checkpoints.
- Delete a forwarding-only Rust helper in the same round that merges its last
  responsibility into the real owner.
- Keep dense runtime owners intact unless the next pass proves they are only
  forwarding or evidence glue.
- Update health/readiness/source-map evidence in the same lane whenever owner
  files are removed or repointed.
- Do not put Rust build artifacts on `C:`; use `D:/CodexTargets/...` or the
  repo-local `.codex-targets/...` path if available.

### Immediate next-entry recommendation

The next implementation round should select lane 1:
`chapter_single_generation_stream_workflow_service.rs`.

Expected decision at the end of that round:

- either collapse any remaining forwarding-only stream glue into a stronger
  owner and delete the redundant file or helper
- or explicitly mark the stream workflow file as the surviving real stream
  owner, then proceed to lane 2 for Python route/stream shell freeze or delete
  planning

## 2026-06-13 stream workflow owner confirmation

Lane 1 is now resolved: `chapter_single_generation_stream_workflow_service.rs`
is a surviving real Rust owner, not a forwarding-only cleanup target.

The file owns multiple active stream contracts that should stay together:

- route payload to restored runtime launch preparation
- SSE lifecycle spawn/run and event ordering
- stream success response payload projection
- quality-gate event projection and completion messaging
- optional follow-up analysis launch/result projection
- story runtime contract projection and active story-repair payload shaping

Because those responsibilities are tightly coupled to the stream transport
contract, the next migration progress should not force this file into
`runtime_restore_service.rs` or `runtime_state_service.rs`. Those files own
runtime restoration and persistence semantics; the stream workflow file owns
the route-facing SSE contract.

The fast plan therefore moves to lane 2: reduce the remaining Python
single-generation route/stream shell presence. The first low-risk closeout is
to make the legacy Python route module lazy-imported only when
`legacy_single_generation_python_routes_enabled` is true. Default startup
should not register or import the legacy `chapter_generation_routes.py` module;
it remains available only for explicit rollback registration.

## 2026-06-16 post-runtime-restore owner audit

After the 2026-06-16 owner-absorption rounds, the next planning baseline needs
to be adjusted again. The current bottleneck is no longer the previously
identified shared-runtime function group alone. The more immediate and more
coherent next package is now the remaining route-facing restore / background
workflow concentration inside
`backend-rs/src/services/chapter_single_generation_runtime_restore_service.rs`.

This follows from the current state:

- `chapter_single_generation_runtime_checkpoint_service.rs` now owns checkpoint
  projection and task-stage persistence.
- `chapter_single_generation_background_launch_service.rs` now owns startup
  snapshot planning, background task seed shaping, background response payload
  shaping, and persistence/dispatch.
- `chapter_single_generation_existing_background_task_service.rs` now owns the
  existing-background read-side and response projection chain.
- `chapter_single_generation_runtime_seed_service.rs` now owns restored
  runtime-state seed projection and launch-input reconstruction.

The surviving concentration in `runtime_restore_service.rs` is therefore
smaller and clearer than before. It is now mainly the route-facing restore and
background workflow shell that still ties those owners together:

- `PreparedSingleChapterGenerationRestoredRuntimeLaunch`
- `SingleGenerationBackgroundWriteWorkflowEntry`
- route-facing `prepare_*` and `start_from_route_payload(...)` orchestration
- `from_parts(...)` and adjacent test surfaces
- runtime-restore / write-workflow owner contracts

This changes the default next-package choice:

1. Prefer another whole-owner split inside
   `chapter_single_generation_runtime_restore_service.rs` first.
2. Only switch back to the `chapter_generation` shared-runtime owner-lift lane
   if the remaining restore/workflow shell proves too coupled to split cleanly
   in one round.

Why this is the right acceleration move:

- it continues the same active `chapter_single_generation` runtime-restore lane
  that already produced multiple validated Rust owners in sequence
- it keeps migration work concentrated on one real active route/runtime owner
  chain instead of reopening a broader shared-runtime package too early
- it improves the odds that later Python single-generation route/stream shell
  freeze decisions can be reviewed against a smaller, easier-to-audit Rust
  restore owner boundary

Current quantitative baseline for planning:

- `backend/app/**/*.py`: `293` files / about `82,945` lines
- `backend-rs/src/**/*.rs`: `164` files / about `145,712` lines
- chapter/generation/draft/analysis/regeneration/compat related Python files:
  `123` files / about `26,233` lines

Planning interpretation:

- the project is already Rust-heavy at the executable owner layer, but Python
  physical surface is still large
- future acceleration should therefore count progress by active owner
  contraction, not by Python line edits or metadata cleanup
- do not spend the default next round on Python shell touch-up or source-map
  maintenance unless the same round also closes an approved freeze/delete/
  repoint package

Revised next-entry recommendation:

- primary:
  split the remaining
  `PreparedSingleChapterGenerationRestoredRuntimeLaunch` +
  `SingleGenerationBackgroundWriteWorkflowEntry` route-facing workflow group
  into one new dedicated Rust owner file
- secondary fallback:
  if that split does not give a clean whole-owner boundary, return to the
  `chapter_generation_runtime_service.rs` single-generation-specific shared
  function-group absorption plan as the next full package

## 2026-06-14 progress audit and planning adjustment

The migration has moved past the "find another Rust owner marker" phase. The
current whole-file Rust service/runtime baseline is complete for the existing
top-level chapter service set: all 36 `backend-rs/src/services/chapter*.rs`
files publish `service_runtime_closeout_status`. The route/readiness layer is
also Rust-owned for the audited chapter groups, with the latest manifest-only
evidence still reporting 407 readiness probes and Rust-owned chapter route
groups.

The remaining bottleneck is therefore not a missing Rust route or service
implementation. It is physical Python source-map closeout. The core chapter
rollback/source-map package is 9 files / 1854 lines:

- `backend/app/api/chapters.py`
- `backend/app/api/chapter_generation_routes.py`
- `backend/app/api/chapter_batch_generation_routes.py`
- `backend/app/api/chapter_regeneration_routes.py`
- `backend/app/api/chapter_partial_regeneration_routes.py`
- `backend/app/api/chapter_analysis_routes.py`
- `backend/app/api/chapter_analysis_task_routes.py`
- `backend/app/api/chapter_draft_routes.py`
- `backend/app/services/compat/chapter_generation_route_compat_service.py`

Planning is adjusted accordingly:

- Batch A: close the default-off generation route source maps
  (`chapter_generation_routes.py`, `chapter_batch_generation_routes.py`) after
  explicit same-round approval for freeze, delete, or repoint.
- Batch B: close the registry-gated draft/analysis/regeneration route source
  maps after verifying the legacy registration knobs and matching Rust owner
  profiles.
- Batch C: close aggregate source maps (`chapters.py` and the compatibility
  service) only after direct Python consumers are removed, gated, or explicitly
  approved as source-map-only tests.
- Do not continue adding marker-only Rust changes to the already closed 36/36
  service files. Future Rust work must be triggered by concrete drift, newly
  added files, missing smoke/readiness evidence, or a real ownership gap.
- If no source-map closeout approval exists and no Rust drift exists, the
  correct fast path is to request the batch action decision, not to create
  another micro-slice.

## 2026-06-16 post-stale-closeout reset

The 2026-06-16 source-map stale closeout lane is now complete. Rust
source-map/rollback contracts under `backend-rs/src` no longer point at deleted
Python files, including the previously remaining top-level users/admin package.
This is useful migration hygiene, but it is not the same thing as completing
Python-to-Rust migration.

The project still contains substantial Python surface:

- `backend/app/**/*.py`: `293` files / about `92,274` lines
- `backend-rs/src/**/*.rs`: `153` files / about `155,497` lines
- chapter/generation/draft/analysis/regeneration related Python files:
  roughly `122`
- highest remaining Python concentration still sits around
  `services/chapter_generation`, `services/batch_generation`, compat shells,
  and aggregate route/source-map files

This changes the planning interpretation:

- do not treat more Python source-map repoint work as the default fast lane
- do not treat small Python edits or stale naming cleanup as equivalent to
  active runtime migration
- from this point, the default progress unit must again be a real Rust owner
  package that absorbs active behavior, not just metadata or rollback mapping

### Reset package order after stale closeout

1. `chapter_single_generation` active runtime owner completion
2. `chapter_generation` shared runtime owner completion
3. `chapter_batch_generation` active owner cleanup only when it exposes a real
   behavior gap
4. Python source-map freeze/delete/repoint batches only after the corresponding
   Rust runtime owner package is materially stronger
5. `chapters` aggregate shell and schema/startup follow-up after the route and
   runtime packages above are stable

### New fast lane: active runtime owner absorption

The next acceleration lane should not reopen
`chapter_single_generation_stream_workflow_service.rs`; that file has already
been confirmed as a surviving real owner. The higher-value gap is now inside
the shared `chapter_generation` runtime package, where a single-generation
result/draft/history lifecycle function group still lives in the shared file
even though its active consumers are predominantly single-generation owners.

Primary target:

- source owner:
  `backend-rs/src/services/chapter_generation_runtime_service.rs`
- target package:
  `chapter_single_generation`
- preferred landing owner:
  `backend-rs/src/services/chapter_single_generation_runtime_state_service.rs`
  or a dedicated child owner such as
  `backend-rs/src/services/chapter_single_generation_runtime_state_service/result_lifecycle_owner.rs`

Function-group boundary to move as one package:

- `update_latest_generated_chapter_history_quality_metrics(...)`
- `normalized_non_empty_string(...)`
- `GeneratedResultQualityView`
- `generated_result_quality_view(...)`
- `GeneratedResultLifecycleView`
- `generated_result_lifecycle_view(...)`
- `apply_generated_result_quality_view(...)`
- `apply_generated_result_lifecycle_view(...)`
- `build_single_generation_followup_draft_result(...)`
- `build_single_generation_candidate_draft_attempt(...)`
- `SingleGenerationCandidateDraftLifecycleView`
- `single_generation_candidate_draft_lifecycle_view(...)`
- `single_generation_candidate_draft_attempt_view(...)`
- `PersistedHistoryPayloadView`
- `persisted_history_payload_view(...)`

Why this boundary matters:

- these functions shape single-generation draft lifecycle, follow-up draft
  behavior, and generated-history quality write-back
- the active consumers are already concentrated in
  `chapter_single_generation_stream_workflow_service.rs`,
  `chapter_single_generation_runtime_state_service.rs`, and only one narrow
  history-quality consumer in `chapter_analysis_runtime_service.rs`
- leaving the group in `chapter_generation_runtime_service.rs` keeps single-
  generation behavior split across package boundaries and slows future
  Python-shell freeze decisions

Done means:

- the whole function group above is owned by the single-generation package as
  one coherent Rust boundary
- downstream consumers import the new owner directly
- the old shared-runtime file no longer keeps single-generation-only draft /
  follow-up / history-lifecycle glue
- the same round runs focused Rust tests and `cargo check` with
  `E:/Code/ProjectsCode/WorkSpace/Codex/NovelAi/MuMuNovel/.codex-targets/backend-rs`

### Revised speed rule

For the next phase, a round counts as acceleration only if it does one of the
following:

- moves an active runtime function group from a shared/compat owner into the
  real package owner
- removes a forwarding-only Rust helper in the same round as the owner merge
- enables a whole Python shell freeze/delete/repoint decision by making the
  corresponding Rust owner materially easier to audit

Rounds that only rename tests, repoint stale source-map strings, or lightly
patch Python compatibility code should now be treated as maintenance, not as
the primary migration lane.
