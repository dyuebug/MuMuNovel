# Quality Guidelines

> Code quality expectations for backend changes.

---

## Overview

Backend quality in this repository is driven by three themes:

- keep route handlers thin and move behavior into services
- preserve compatibility across shared runtime flows, especially tasks and
  generation pipelines
- verify schema/API changes with pytest and cross-layer review

The codebase contains both active refactors and compatibility layers, so
"working code" is not enough; changes must also land in the correct layer.

---

## Design Expectations

- Prefer focused modules over giant catch-all services.
- When touching generation, regeneration, analysis, or batch workflows, trace
  the whole pipeline before editing one file in isolation.
- Separate persistence concerns (`models/`) from API contracts (`schemas/`)
  and from behavior (`services/`).
- Keep bootstrap, runtime services, compat wrappers, and HTTP routes in their
  own lanes.

## Whole-Module Rust Migration Package Contract

When a backend task is explicitly about accelerating Python-to-Rust migration,
the default unit is a whole file, function group, or module package. A tiny
seam is acceptable only as a review checkpoint inside the active package; it
must not become the standalone planning or progress unit.

Before implementing a package, record:

- Python source map: route files, service helpers, schemas, and compatibility
  shells that are replaced, frozen, or intentionally left as fallback.
- Rust target map: route handlers, service owners, models, tests, smoke probes,
  and manifest entries that prove the package owner exists.
- Behavior contract: HTTP payloads, SSE events, task lifecycle, checkpoint
  shape, provider defaults, error shells, fallback behavior, and schema
  assumptions that must stay stable.
- Validation boundary: focused Rust tests, `cargo check`, and route-group smoke
  or manifest validation whenever transport ownership changes.
- Rollback boundary: gateway route, fallback shell, config/feature knob,
  migration step, or deployment probe that makes rollback observable.

Do not count helper relocation as migration progress unless it removes or
freezes a Python dependency, shrinks a fallback shell, clarifies Rust owner
cutover, improves smoke coverage, clarifies rollback, or moves a schema
assumption toward an explicit migration owner.

## Scenario: Rust chapter batch generation route seam

### 1. Scope / Trigger
- Trigger: a change touches `backend-rs/src/api/chapter_batch_generation.rs`
  or its service-owned task/query/stream/runtime helpers.
- Why this needs code-spec depth: the route group spans HTTP, SSE, task
  persistence, resume/recover semantics, and compatibility response shapes.

### 2. Signatures
- Route entrypoints may accept HTTP request payloads or SSE stream requests,
  but must delegate business execution to service helpers.
- Service signatures should be grouped by responsibility:
  `create plan`, `workflow`, `query/view context`, `stream builder`,
  `runtime executor`.
- `tokio::spawn` remains a route boundary concern unless the background
  ownership model changes for the whole runtime.

### 3. Contracts
- Request boundary contract: route parses inputs, performs basic validation,
  resolves access context, and builds `AIConfig`.
- Single-route workflow-start contract: if a single-chapter route lane only
  needs to hand the transport payload into one background/stream workflow
  start owner, the route should delegate the route-payload -> workflow-request
  normalization directly to that workflow owner instead of locally rebuilding
  the same request contract before calling neighboring background/stream
  workflow entrypoints.
- Batch-route workflow-start contract: if a batch-create route lane only
  needs to hand the transport payload into one batch create workflow-start
  owner, the route should delegate the route-payload ->
  `BatchGenerationCreateWorkflowRequest` normalization directly to that
  workflow owner instead of locally rebuilding the same request contract
  before calling the neighboring batch create write-workflow entrypoint.
- Batch create route-start collapse contract: if the batch create write lane
  already has both a route-payload -> workflow-request builder and a public
  write-workflow start owner, do not preserve a neighboring `RouteStart`
  wrapper that only forwards the built request into that same owner chain;
  collapse that empty hop back into the write-workflow entry boundary.
- Batch active-task-list route-query contract: if a batch active-task-list
  route lane only needs to hand transport query parameters into one
  active-task query owner, the route should delegate
  `route query -> active-task-list request` normalization directly to that
  query owner instead of locally rebuilding
  `ActiveBatchGenerationTaskListQueryRequest` before calling neighboring
  read/query helpers.
- Batch active-project route-query contract: if a batch active-project route
  lane only needs to hand the transport project identifier into one
  active-project query owner, the route should delegate that
  `route path -> active-project query start` handoff directly to the query
  owner instead of locally replaying the same project-access/query start
  boundary before calling neighboring read/query helpers.
- Batch query route-start collapse contract: if a batch active-task-list or
  active-project query lane already routes transport query/path inputs
  directly into one route-query owner function, do not preserve a neighboring
  `RouteStart` wrapper that only forwards normalized route inputs into that
  same owner chain; collapse that empty hop back into the route-query owner
  boundary instead.
- Batch task-view prepared-query collapse contract: if the batch task-view
  query lane already owns direct task loading plus final payload projection,
  do not preserve neighboring `Prepared*` wrappers that only replay
  `prepare -> into_payload` for active-task-list, active-project, or existing
  single-background query branches; collapse those empty hops back into the
  task-view query owner and its final payload projection helpers.
- Batch resume launch-sources collapse contract: if the batch resume command
  lane already owns restored runtime-state materialization, manual-review
  blocker checks, validated execution selection, and final reset-persistence
  plan assembly, do not preserve a neighboring `Prepared*LaunchSources`
  wrapper that only forwards `prepare restored state -> into launch
  persistence plan`; collapse that empty hop back into the final resume
  launch-persistence owner.
- Batch owned read-state contract: if neighboring batch status-query and
  status-stream lanes both need the same owned `task -> recover -> snapshot`
  read-state sources, they should consume one shared owned read-state owner
  directly instead of replaying the same owned-task recovery and snapshot-load
  chain independently in parallel query helpers.
- Batch status/stream read-state projection collapse contract: if the batch
  status-payload lane and batch status-stream lane already consume one shared
  owned read-state owner, do not preserve neighboring projection wrappers
  that only reopen `shared read-state -> payload/stream projection` without
  adding validation, error translation, or semantic branching; collapse those
  empty hops back into the final status-payload / stream-state owners.
- Batch owned task-sources contract: if neighboring batch cancel and resume
  command lanes both need the same owned `task + snapshot` sources but must
  preserve the current non-recovery semantics, they should consume one shared
  owned task-sources owner directly instead of replaying task lookup and
  snapshot-load independently; recovery stays in the higher read-state owner
  and must not be pulled down into cancel/resume by accident.
- Provider context contract: placeholder/default prompt-context provider payload
  should be assembled once at the request/workflow preparation boundary, then
  passed explicitly through dispatch/runtime calls. Route handlers and stream
  helpers should not recreate the same default payload locally.
- Response boundary contract: route may return compat response envelopes, but
  task status payload assembly belongs to service helpers.
- Read/query contract: if the batch query owner already holds the selected
  task rows plus their matching snapshot sources for one active-task set,
  downstream query/read-context helpers should project from that batch owner
  directly instead of reloading snapshots one task at a time through a local
  per-task read-context loop.
- Stream contract: polling state transitions and SSE event payload shape must
  be owned by shared stream helpers, not rebuilt inline per endpoint.
- Stream-state projection contract: if a stream lane only consumes
  `BatchGenerationStreamState`, it should load owned task + snapshot sources
  into that stream-state owner directly instead of first materializing a full
  read-context object and then discarding everything except the stream view.
- Single-stream completion projection contract: if a single-chapter stream
  lane already owns the terminal `result + analysis` pair for one completion,
  the same owner should project the completion message, follow-up SSE events,
  and response payload together instead of letting the workflow neighbor
  locally rebuild those terminal contracts from the same sources.
- Single-stream success follow-up contract: if a single-chapter stream lane
  already owns the generated chapter result plus stream launch intent, the
  same owner should carry story-runtime contract assembly, follow-up analysis,
  latest-history quality sync, and terminal completion projection together
  instead of leaving those post-success steps split across the workflow body.
- Single-stream success emission contract: if a single-chapter stream lane
  already owns the completion projection for one successful generation, the
  same owner should also project the ordered SSE completion emission plan
  (`complete -> quality events -> result -> analysis-started -> done`)
  instead of letting the workflow neighbor rebuild and emit those follow-up
  payloads one by one from the same completion sources.
- Single-stream success analysis-projection contract: if a single-chapter
  stream success lane already owns the generated result plus follow-up
  analysis outcome, that same owner chain should also project the quality
  events, response payload, and analysis-started event directly instead of
  reopening free helper hops for `analysis -> quality event -> response
  payload` reconstruction beside the completion owner.
- Single-stream success owner-collapse contract: if a single-chapter stream
  success lane already lets `SingleGenerationStreamAnalysisOutcome` own the
  generated result's follow-up analysis, quality event projection, terminal
  response payload, and ordered SSE success emission plan, do not preserve a
  neighboring `SingleGenerationStreamCompletionProjection` owner that only
  replays `analysis outcome -> completion/result/event emission` through one
  more file-local hop; collapse that empty seam back into the analysis owner.
- Single-stream prepare-owner contract: if a single-chapter stream lane only
  needs the runtime launch input produced by the single-generation prepare
  boundary, it should consume that prepare owner directly instead of routing
  the same launch-input projection back through the neighboring background
  write workflow service.
- Single-restored-launch direct materialization contract: if the
  single-generation restored-launch owner already owns request validation,
  chapter-target ownership, startup snapshot planning, response payload
  assembly, and runtime launch input, neighboring stream/background workflow
  lanes should consume owner-provided direct runtime/background materialization
  instead of reopening `prepare(...).into_runtime_launch_input()` or
  `prepare_from_target(...).into_background_launch_parts(...)` as local
  handoff chains.
- Single-snapshot persistence / merge owner contract: if the
  single-generation startup snapshot lane already owns chapter-scoped startup
  snapshot planning and the runtime lane already owns single checkpoint stage
  projection, neighboring single-generation owners should consume a local
  `merge_single_generation_runtime_state(...)` /
  `upsert_single_generation_runtime_snapshot(...)` boundary instead of
  reopening `project_merged_batch_generation_runtime_state(...)` or
  `upsert_batch_generation_runtime_snapshot(...)` directly from the batch
  snapshot module.
- Shared snapshot persistence owner contract: if both batch and
  single-generation lanes need the same lower-level `task id + runtime state
  -> snapshot merge / replace persistence` semantics, do not preserve the
  batch snapshot file as the de facto shared write entrypoint and do not add
  another single-only forwarding facade; instead, move that truly shared
  lower-level owner into a chapter-generation-scoped persistence service and
  let both module owners consume that shared boundary directly while keeping
  only their module-local queued/resume/startup plan semantics.
- Single-runtime checkpoint file-collapse contract: if the single-generation
  runtime owner already owns task-stage mutation, runtime snapshot writes,
  and all remaining `SingleGenerationSnapshotStage -> checkpoint payload`
  projection call sites, do not preserve a neighboring
  `chapter_single_generation_runtime_checkpoint_service` file that only
  reopens checkpoint stage projection through another module boundary;
  collapse that file back into the single runtime-state owner.
- Status-payload projection contract: if a route or runtime-write lane only
  needs the status-task payload shape, it should project that payload from the
  owned task + runtime-state/snapshot sources directly instead of first
  materializing `BatchGenerationReadContext` and then consuming only the
  status view.
- Owned batch status payload query contract: if a batch status query lane
  already owns `load owned task -> recover active timeout -> load snapshot`,
  that same query owner should also project the final compat status payload
  directly instead of returning recovered task/snapshot sources for the outer
  query layer to assemble locally.
- Restored-resume launch contract: if a batch resume lane already owns a
  restored `BatchGenerationRequestRuntimeState` plus resume
  `runtime_state_seed`, the runtime-state owner should materialize the batch
  runtime launch input directly from that restored owner instead of letting
  the resume command/write caller split the same restored state into separate
  local request-runtime and launch-input assembly branches.
- Single-resume launch contract: if a single-chapter resume lane already owns
  a restored `BatchGenerationRequestRuntimeState` plus resume
  `runtime_state_seed`, the runtime-state owner should materialize the single
  runtime launch input directly from that restored owner instead of letting
  the neighboring resume command branch reopen `into_launch_parts()` and
  split request-runtime / runtime-seed handling locally again.
- Resume restored-state owner contract: if a batch or single-chapter resume
  lane already owns a restored runtime-state projection with both
  `request_runtime_state` and `runtime_state_seed`, that same restored-state
  owner should materialize the final batch/single runtime launch directly
  instead of letting the neighboring command layer reopen
  `into_launch_parts()` and replay request-runtime / runtime-seed handoff
  locally again.
- Cancel-response envelope contract: if the batch cancel runtime/write owner
  already owns the merged cancelled runtime state and the response-ready
  status payload projection, that same owner should also project the final
  command-summary response envelope instead of letting the neighboring cancel
  workflow extend the payload with local summary fields again.
- Batch cancel workflow-start contract: if the batch cancel lane already owns
  a prepared cancelled persistence plan, the same batch write-workflow owner
  should sequence `prepare owned cancel -> persist cancelled state` directly
  instead of leaving cancel as a route-local special case beside create/resume
  write workflow starts.
- Batch cancel service file-collapse contract: if the batch cancel write lane
  already owns shared owned-task source loading, terminal status gating,
  cancelled persistence-plan materialization, and final write-workflow start,
  do not preserve a neighboring `chapter_batch_generation_cancel_service`
  file that only reopens the same owner chain through another module
  boundary; collapse that file back into the batch write-workflow owner.
- Batch stream-state file-collapse contract: if the batch status-stream lane
  already owns shared owned read-state loading, stream-state projection, poll
  orchestration, and SSE event emission, do not preserve a neighboring
  `chapter_batch_generation_stream_state_query_service` file that only
  reopens `owned read-state -> stream state` projection through another
  module boundary; collapse that file back into the status-stream owner.
- Batch status-query file-collapse contract: if the batch read/query lane
  already owns shared owned read-state loading, quality-context materialization,
  and final status payload projection, do not preserve a neighboring
  `chapter_batch_generation_status_task_query_service` file that only reopens
  `owned read-state -> status payload` projection through another module
  boundary; collapse that file back into the read-context owner.
- Persistence contract: checkpoint updates, snapshot persistence, and runtime
  state advancement belong to service/runtime helpers.
- Single-background task-seed contract: if the single-chapter background
  launch owner already owns the startup snapshot, response payload, and
  runtime launch input, that same owner should also project the persistence-
  ready task insert seed instead of letting the neighboring write/persistence
  step rebuild the single background task active-model contract from
  `chapter_target`, `task_id`, and runtime-derived fields again.
- Single-task-model owner contract: if the single-chapter background write
  lane already owns chapter-scoped launch targets and the single runtime lane
  already owns task-stage persistence, neighboring single-generation owners
  should consume one local
  `SingleGenerationTaskPersistenceSeed` /
  `SingleGenerationTaskStage` boundary instead of reopening batch task seed
  semantics or keeping file-local copies of task mutation contracts for the
  same chapter-scoped branch.
- Shared snapshot-query / task-recovery owner contract: if both batch and
  single-generation lanes need the same lower-level `task timeout recovery`
  or `task ids -> snapshot query` semantics, do not create another
  single-only forwarding facade and do not leave the single-generation
  production lane directly attached to a batch-named file by accident;
  instead, move the truly shared lower-level owner into a chapter-generation-
  scoped service and let both module owners consume that shared boundary
  directly.
- Shared chapter-access owner contract: if both batch-adjacent and
  single/generation lanes need the same lower-level
  `chapter id(s) -> accessible generation chapter(s)` semantics, do not
  preserve a batch-named access service as the shared entrypoint and do not
  add another single-only forwarding facade; instead, move that truly shared
  lower-level owner into a chapter-generation-scoped access service and let
  both module owners consume that shared boundary directly.
- Shared quality runtime-context persisted-source contract: if a
  chapter-generation-scoped shared persistence owner needs the same
  lower-level `persisted quality columns + summary state -> quality runtime
  context` semantics for batch-shaped history, do not preserve a
  batch-named quality runtime-context service as the shared persisted-source
  entrypoint; instead, consume the chapter-generation-scoped quality runtime
  context owner directly and prove batch-scope ordering/state semantics with
  focused regression tests.
- Single-background existing-task payload query contract: if the
  single-chapter background write lane only needs the compat payload for an
  already-running background task, the neighboring query owner should project
  that payload directly instead of returning `BatchGenerationReadContext` and
  letting the write workflow reopen the read-context -> payload hop locally.
- Single-background existing-task query owner-collapse contract: if the
  single-chapter background write lane already owns target loading, existing
  task short-circuit branching, and the final compat payload consumer, do not
  preserve a neighboring batch task-view query owner entrypoint that only
  reopens `load active tasks -> existing task payload projection` for this
  single-generation branch; collapse that query lane back into the
  single-generation write owner and keep batch task-view query scoped to the
  remaining batch active-list / active-project lanes.
- Single-background workflow entry contract: if the single-chapter background
  write lane already owns both the existing-task compat payload branch and the
  prepared background launch branch, the same workflow-entry owner should
  choose between reuse vs launch directly instead of leaving
  `load target -> existing-task short-circuit -> prepare launch -> persist`
  orchestration split across the outer workflow body.
- Single-background workflow start contract: if the single-chapter background
  write lane already owns the workflow-entry branch decision
  (`existing payload` vs `prepared launch`), the same workflow-start owner
  should also sequence `prepare workflow entry -> persist-and-dispatch`
  directly instead of leaving that final handoff split across the outer
  write-workflow function body.
- Single-background workflow public-start contract: if the single-chapter
  background write lane already owns an explicit workflow-start owner, the
  outer public write-workflow entry should call that owner directly instead of
  reopening `prepare(...).persist_and_dispatch(...)` as a separate handoff
  chain in the free function entrypoint.
- Single-background workflow wrapper-collapse contract: if the
  single-chapter background write lane already lets the workflow-entry owner
  sequence `prepare -> persist_and_dispatch` directly, do not preserve
  neighboring `SingleGenerationBackgroundWorkflowStart` or
  `SingleGenerationBackgroundWorkflowRouteStart` wrappers that only replay
  route-payload normalization or a single-call
  `prepare -> persist-and-dispatch` handoff around that same owner chain.
- Single-existing-background query file-collapse contract: if the
  single-chapter background write lane already owns the branch decision between
  `existing payload` and `prepared launch`, do not keep the full
  `active task query -> recovered read-state -> existing-background payload`
  owner chain inline in the write-workflow file; collapse that query/load/
  projection contract into one dedicated single-generation query owner file.
- Single-background launch-parts persistence contract: if the single-chapter
  background launch-parts owner already owns the task insert seed, startup
  snapshot plan, response payload, and runtime launch input, that same owner
  should also sequence `task insert -> snapshot persist -> runtime dispatch ->
  response payload` directly instead of leaving a neighboring write-workflow
  free helper to reopen the final persistence/disptach chain.
- Single-runtime lifecycle contract: if the single-chapter runtime lane
  already owns the preparing persistence step, runtime execution input, and
  terminal persistence routing contract, the same lifecycle owner should
  sequence `prepare -> execute generation -> persist terminal result`
  directly instead of leaving that lifecycle orchestration split across the
  outer runtime driver body.
- Single-runtime dispatch contract: if the single-chapter runtime lane
  already owns an explicit lifecycle owner, the outer background/resume
  dispatch entry should hand runtime launch input to that lifecycle owner
  directly instead of reopening a separate
  `dispatch -> execute_single_generation_runtime(...)` wrapper chain.
- Batch-resume launch-sources contract: if the batch resume command lane
  already owns status gating, persisted runtime-context restore,
  manual-review blocker detection, and existing workflow-runtime-state
  handoff, the same launch-sources owner should materialize that restored
  resume source contract directly instead of returning to the outer command
  layer to replay those checks locally before launch preparation.
- Batch-resume workflow launch contract: if the batch resume write lane
  already owns the prepared resume launch persistence plan, the same
  workflow-launch owner should sequence `prepare owned resume ->
  persist-and-dispatch` directly instead of leaving that final owner handoff
  split across the outer write-workflow function body.
- Batch-resume workflow start contract: if the batch resume write lane already
  owns the prepared resume workflow-launch branch, the same workflow-start
  owner should sequence `prepare workflow launch -> persist-and-dispatch`
  directly instead of leaving that final handoff split across the outer
  write-workflow entry function body.
- Batch-create workflow entry contract: if the batch create write lane already
  owns access-checked request input plus the prepared create launch
  persistence contract, the same workflow-entry owner should sequence
  `prepare owned create -> persist-and-dispatch` directly instead of leaving
  that create-entry handoff split across the outer write-workflow function
  body.
- Batch-create direct persistence-plan materialization contract: if the batch
  create workflow-launch owner already owns normalized chapter targets,
  startup snapshot planning, runtime launch input, task-spec projection, and
  response payload/task-seed assembly, the neighboring workflow-entry owner
  should consume one owner-provided direct persistence-plan materialization
  instead of reopening `prepare(...).into_persistence_plan(...)` as a local
  handoff chain.
- Batch-create workflow start contract: if the batch create write lane already
  owns the access-checked workflow-entry branch, the same workflow-start
  owner should sequence `prepare workflow entry -> persist-and-dispatch`
  directly instead of leaving that final handoff split across the outer
  write-workflow entry function body.
- Batch-create workflow-entry collapse contract: if the batch create write
  lane already has a persistence-plan owner that can both materialize and
  dispatch the final create contract, do not preserve a neighboring
  `workflow entry` wrapper that only forwards
  `prepare persistence plan -> persist-and-dispatch`; collapse that hop back
  into the persistence-plan owner instead.
- Batch-write workflow public-start contract: if the batch create or batch
  resume write lane already owns a workflow-start owner, the outer public
  write-workflow entry should call that owner directly instead of reopening
  `prepare(...).persist_and_dispatch(...)` as a separate handoff chain in the
  free function entrypoint.
- Batch write-workflow start-collapse contract: if a neighboring batch create,
  resume, or cancel write lane keeps a `workflow start` wrapper that only
  forwards `prepare -> persist_and_dispatch` without adding validation,
  timestamp ownership, error translation, or branch selection, collapse that
  wrapper back into the adjacent `workflow entry` / `workflow launch` owner
  instead of preserving an empty compatibility hop.
- Batch-runtime lifecycle contract: if the batch runtime lane already owns
  the preparing persistence step, runtime session/chapter iteration input,
  and per-step progression handoff, the same lifecycle owner should sequence
  `prepare -> iterate chapter steps -> stop/continue handoff` directly
  instead of leaving that lifecycle orchestration split across the outer
  runtime driver body.
- Batch-success attempt contract: if the batch runtime lane already owns the
  post-write guard, follow-up analysis failure contract, quality-gate routing,
  and post-generation persistence handoff, the same success-attempt owner
  should sequence `post-write guard -> follow-up analysis -> persist success
  outcome` directly instead of leaving that success-path orchestration split
  across the outer generation-attempt branch.
- Batch-step preparation contract: if the batch runtime lane already owns the
  task reload, cancelled-status gate, chapter lookup, and project-match check
  needed before one chapter attempt begins, the same step-preparation owner
  should sequence `load task -> cancel gate -> load chapter -> validate
  project match` directly instead of leaving that preparation orchestration
  split across the outer step execution boundary.
- Batch-step execution contract: if the batch runtime lane already owns the
  per-step chapter id, step progress, step-preparation owner, retry carry,
  and generation-attempt execution handoff, the same step-execution owner
  should sequence `prepare step -> carry retry -> execute generation attempt`
  directly instead of leaving that step-level orchestration split across the
  outer runtime lifecycle body.
- Batch-post-generation contract: if the batch runtime lane already owns the
  generated chapter result, follow-up analysis boundary, quality-gate routing,
  and success persistence handoff, the same post-generation owner should
  sequence `run analysis -> route quality gate -> persist success or stop`
  directly instead of leaving that post-generation orchestration split across
  the outer success-attempt body or a neighboring post-analysis-resolution
  owner that only forwards the same analysis outcome into the terminal lane.
- Batch-success-attempt chain contract: if the batch runtime success lane
  already owns post-write guard verification, follow-up analysis workflow,
  terminal quality-gate routing, and final success persistence, the same
  success-attempt owner should sequence that whole success chain directly
  instead of materializing a neighboring post-generation owner for a
  single-call handoff.
- Batch-generation-attempt success-chain contract: if the batch runtime
  generation-attempt lane already owns the generated chapter result plus the
  downstream post-write guard, follow-up analysis workflow, and post-analysis
  terminal handoff chain, the same generation-attempt owner should sequence
  that whole success path directly instead of materializing a neighboring
  success-attempt owner for a single-call handoff.
- Batch-step generation-attempt contract: if the batch runtime step lane
  already owns the loaded chapter plus retry carry and the full downstream
  generation-attempt lifecycle, the same step owner should sequence
  `chapter-started persistence -> prerequisite gate -> attempt-input prepare
  -> generation call -> success/failure routing` directly instead of
  materializing a neighboring prepared generation-attempt owner for a
  single-call handoff.
- Batch-runtime lifecycle-step contract: if the batch runtime lifecycle lane
  already owns chapter iteration plus the downstream step-prepare and
  step-execute lifecycle, the same lifecycle owner should sequence
  `iterate chapter ids -> prepare step -> execute prepared step -> advance or
  stop` directly instead of materializing a neighboring step-execution owner
  for a single-call handoff.
- Batch-runtime lifecycle-step direct-owner contract: if the batch runtime
  step lane already owns step preparation, retry-carry continuation, and the
  prepared step execution owner, the same step owner should sequence
  `prepare step -> reuse retry carry -> execute prepared step` directly
  instead of leaving that retry-aware step orchestration split across the
  outer runtime lifecycle body.
- Single-background workflow-entry persistence contract: if the
  single-generation background write lane already owns the chapter target
  lookup, existing-task payload short-circuit, prepared launch persistence,
  and runtime dispatch handoff, the same workflow-entry owner should sequence
  `load target -> return existing payload or persist launch -> dispatch`
  directly instead of materializing a neighboring workflow-start wrapper or a
  single-call prepared launch wrapper for the same handoff.
- Single-runtime lifecycle terminal-persistence contract: if the
  single-generation runtime lane already owns launch input, preparation
  persistence, generated-result execution, follow-up analysis routing, and
  terminal persistence, the same lifecycle owner should sequence
  `persist preparing -> execute runtime -> route analysis result -> persist
  completed/manual-review/failed` directly instead of materializing a
  neighboring runtime driver or a terminal-persistence wrapper for the same
  single-call handoff.
- Single-runtime lifecycle direct-generation-analysis contract: if the
  single-generation runtime lane already owns runtime launch input,
  generated-result execution, follow-up analysis routing, and manual-review /
  completed / failed persistence semantics, the same lifecycle owner should
  also execute `call generate_and_persist... -> run follow-up analysis ->
  persist manual-review or completed/failed` directly instead of handing that
  active production chain back to neighboring free helper functions for
  one-call replay.
- Single-stream completion-emission contract: if the single-generation stream
  lane already owns generated-result success handling, follow-up analysis,
  quality event projection, response payload assembly, and SSE emission, the
  same completion owner should sequence `run analysis -> build completion
  projection -> emit quality/result events` directly instead of materializing
  a neighboring success-follow-up wrapper for the same single-call handoff.
- Single-stream direct-emission contract: if the single-generation stream
  completion owner already owns the terminal completion message plus the
  ordered success event payloads (`quality_metrics -> quality_gate -> result
  -> analysis_started`), the same completion owner should emit those SSE
  payloads directly instead of materializing a neighboring
  `SingleGenerationStreamSuccessEmissionPlan` wrapper for a single-call
  handoff.
- Single-stream lifecycle contract: if the single-generation stream lane
  already owns prepared runtime launch input, progress-tracker transport
  semantics, runtime execution, completion projection, and terminal
  success/error SSE emission, the same lifecycle owner should sequence
  `spawn stream -> emit start/preparing/generating -> execute runtime ->
  emit completion or error` directly instead of leaving that active path in a
  neighboring `launch_owned_single_chapter_generation_stream(...)` free
  function for a single-call handoff.
- Single-stream workflow public-start contract: if the single-generation
  stream lane already owns an explicit workflow-start owner, the outer public
  stream entry should call that owner directly instead of reopening
  `prepare -> into_runtime_launch_input -> lifecycle.spawn(...)` as a
  separate handoff chain in the free function entrypoint.
- Single-stream workflow wrapper-collapse contract: if the single-generation
  stream lane already keeps prepare ownership on
  `SingleGenerationStreamWorkflowStart` and lifecycle launch ownership on
  `spawn(...)`, do not preserve a neighboring
  `SingleGenerationStreamWorkflowRouteStart` shell or a separate
  `SingleGenerationStreamWorkflowStart::start(...)` wrapper for the same
  single-call `route payload -> prepare -> lifecycle.spawn` handoff.
- Single-prepare entry contract: if the single-generation prepare lane already
  owns request-bound validation, chapter-target lookup or from-target
  ownership, runtime-state restore, and restored launch materialization, the
  same `PreparedSingleChapterGenerationRestoredRuntimeLaunch` owner should
  expose the public prepare entrypoints directly instead of leaving neighboring
  `prepare_single_chapter_generation_request(...)`,
  `prepare_single_chapter_generation_request_from_target(...)`, or
  `prepare_owned_single_generation_runtime_launch_input(...)` free functions as
  single-call handoff wrappers around the same owner boundary.
- Single-prepare restored-launch contract: if the single-generation prepare
  lane already owns request validation, chapter-target load/from-target
  ownership, request-runtime-state assembly, runtime-state restore, and
  startup snapshot / runtime launch materialization, the same prepare owner
  should sequence `validate/load target -> build request runtime state ->
  restore runtime state -> materialize restored launch` directly instead of
  materializing a neighboring prepared-execution wrapper for a single-call
  handoff into the restored-launch owner.
- Single-prepare validated-wrapper collapse contract: if the
  single-generation restored-launch owner already owns both request-bound
  validation and final restored-launch materialization, do not preserve
  neighboring `prepare_validated_*` wrappers that only forward one validated
  request/target handoff into that same owner chain.
- Single-runtime direct-persistence contract: if the single-generation runtime
  lane already owns preparation persistence, runtime execution, follow-up
  analysis routing, and the terminal task/snapshot write contract, the same
  lifecycle owner should persist preparing/completed/failed/manual-review
  state through focused checkpoint/runtime-state helpers directly instead of
  materializing neighboring persistence-plan wrappers for each single-call
  terminal handoff.
- Single-runtime launch-input execution contract: if the single-generation
  runtime launch input already owns the exact `chapter_id/user_id/execution`
  fields consumed by the generation call, that same owner should expose the
  direct runtime execute entry instead of preserving a neighboring free helper
  that only reopens `launch input -> prompt overrides -> generate/persist`.
- Single-background direct-launch-parts contract: if the single-generation
  background write lane already owns chapter-target lookup, existing-task
  payload short-circuit, restored launch preparation, task-seed projection,
  startup snapshot planning, response payload assembly, and runtime launch
  input, the same workflow-entry owner should pass
  `PreparedSingleGenerationBackgroundLaunchParts` directly into one
  `persist task -> persist snapshot -> dispatch runtime` sequence instead of
  materializing a neighboring
  `SingleGenerationBackgroundLaunchPersistencePlan` wrapper for a single-call
  handoff.
- Single-startup-snapshot owner contract: if the single-generation restored
  launch / prepare / write lane already owns chapter-scoped pending-checkpoint
  planning plus the resulting quality/runtime restore payloads, that startup
  snapshot owner should live in a single-generation snapshot module instead of
  remaining in the neighboring batch snapshot file as a chapter-only branch.
- Batch-quality-gate context contract: if the batch quality-gate lane already
  owns terminal semantics parsing plus retry/manual-review routing, the same
  quality-gate owner should also load the persisted retry-budget context it
  needs instead of leaving the terminal owner to read task retry counters
  before handing control back into the quality-gate owner.
- Batch-analysis-workflow contract: if the batch runtime lane already owns the
  generated chapter result plus the follow-up analysis retry/attempt
  resolution chain, the same analysis-workflow owner should sequence
  `analysis-enabled gate -> retry loop -> execute analysis attempt ->
  complete or stop` directly instead of leaving that analysis orchestration
  split across the outer post-generation body.
- Batch-analysis-attempt contract: if the batch runtime lane already owns one
  generated chapter result plus the analysis-started persistence, prepared
  analysis execution handoff, and attempt completion/retry resolution
  semantics, the same analysis-attempt owner should sequence
  `persist started -> execute prepared/fallback analysis -> resolve completed
  or retry` directly instead of leaving that attempt-level orchestration split
  across free helper functions adjacent to the analysis workflow owner.
- Batch-generation-attempt-input contract: if the batch runtime lane already
  owns one generation attempt's persisted compat restore, prompt-override
  projection, and provider-payload preparation, the same attempt-input owner
  should materialize those inputs together instead of leaving neighboring
  prepare bodies to stitch the same runtime snapshot and provider handoff
  through separate free helper functions.
- Batch-runtime active-path helper elimination contract: if the batch runtime
  lane already has an explicit attempt-input owner or post-analysis terminal
  owner, active-path free helpers such as persisted compat restore or
  story-repair snapshot refresh should be pulled under that owner boundary
  instead of surviving as single-call wrappers adjacent to the production
  path.
- Batch-analysis-completion contract: if the batch analysis completion lane
  already owns the completed analysis snapshot plus the optional
  current-quality runtime snapshot handoff, the same completion owner should
  materialize that current-quality snapshot directly instead of relying on a
  neighboring free helper to rebuild persisted runtime context and current
  chapter quality state for one production call site.
- Batch-analysis-attempt execution contract: if the batch follow-up analysis
  lane already owns the generated chapter result, analysis-started snapshot
  persistence, prepared analysis execution handoff, and attempt
  completion/retry resolution, the same analysis-attempt owner should
  sequence `persist started -> execute prepared analysis or fallback follow-up
  -> hand result to resolution owner` directly instead of leaving prepared vs
  fallback orchestration split across the outer attempt body.
- Batch-analysis-attempt direct-preparation contract: if the batch follow-up
  analysis lane already owns prepared-analysis selection, analysis-started
  snapshot persistence, prepared/fallback execution, and attempt resolution,
  the same analysis-attempt owner should sequence that full chain directly
  instead of reopening local `execute_prepared_or_fallback(...)` or split
  `persist_started(...)` branches around the same production attempt.
- Batch-analysis-attempt direct-resolution contract: if the batch follow-up
  analysis lane already owns the generated chapter result, analysis-started
  snapshot persistence, prepared/fallback execution result, and the final
  completion/retry routing semantics, the same analysis-attempt owner should
  resolve `completed or retry` directly instead of materializing a neighboring
  analysis-attempt-resolution owner for one production handoff.
- Batch-step generation-attempt direct-owner contract: if the batch runtime
  step lane already owns chapter-started persistence, prerequisite gating,
  attempt-input preparation, generation execution, post-write guard, follow-up
  analysis, and terminal routing, the same step owner should sequence that
  whole generation-attempt chain directly instead of leaving local
  `execute_generation_attempt(...)` or `execute_success_chain(...)` wrappers
  inside the prepared-step owner.
- Batch-attempt-input direct-generation contract: if the batch runtime
  generation-attempt lane already owns compat restore, prompt override
  materialization, provider payload preparation, and the downstream generation
  execution call, the same attempt-input owner should also execute
  `prepare input -> call generate_and_persist...` directly instead of handing
  a prepared payload struct back to the neighboring step owner for one more
  production generation-call replay.
- Batch-post-analysis-resolution contract: if the batch runtime lane already
  owns the chapter step progress plus the resolved analysis outcome, the same
  post-analysis-resolution owner should sequence
  `analysis success/failure -> quality-gate route or fail stop -> persist
  success progression` directly instead of leaving that terminal resolution
  split across the outer post-generation body.
- Batch-post-analysis direct-owner handoff contract: if the batch runtime
  success lane already owns the generated chapter result plus the follow-up
  analysis owner and post-analysis-terminal owner, the same success chain
  should hand the analysis result directly to the terminal owner instead of
  reopening local `run_follow_up_analysis(...)` or
  `resolve_analysis_outcome(...)` wrappers that only forward the same
  production outcome.
- Batch-post-analysis-terminal contract: if the batch runtime lane already
  owns the resolved analysis outcome plus the chapter step progress and final
  quality-gate/persistence handoff, the same post-analysis-terminal owner
  should sequence `resolve quality gate or stop -> persist success/failure`
  directly instead of leaving success and failure terminal persistence spread
  across neighboring methods or free helper functions.
- Batch-post-analysis terminal quality-gate contract: if the batch runtime
  terminal lane already owns the post-analysis success outcome, chapter step
  progress, and the quality-gate retry budget sources, the same
  post-analysis-terminal owner should load retry budget, resolve terminal
  semantics, and route quality-gate retry/manual-review directly instead of
  materializing a neighboring quality-gate-resolution owner for one
  production handoff.
- Batch-quality-gate-resolution contract: if the batch runtime lane already
  owns the chapter step progress plus the post-analysis quality-runtime state
  and retry budget, the same quality-gate-resolution owner should sequence
  `load snapshot -> resolve terminal semantics -> route retry/manual-review`
  directly instead of leaving that quality-gate routing chain split across the
  outer post-analysis-resolution body.
- Batch-runtime helper-wrapper contract: if the batch runtime lane already
  has a dedicated owner for retry progression, post-write guard resolution, or
  prepared step / generation-attempt execution, the outer workflow body should
  consume that owner directly instead of keeping thin helper wrappers that only
  forward the same owner-ready inputs back into the runtime lane.
- Batch-runtime public-start contract: if the batch runtime lane already owns
  an explicit lifecycle owner, the outer runtime dispatch/public entry should
  hand execution input to that lifecycle owner directly instead of reopening
  `execute_batch_generation_runtime(...)` or a runtime-driver wrapper chain
  that only forwards the same owner-ready launch input.
- Batch-cancel workflow contract: if the batch cancel lane already owns the
  cancelled persistence/status payload contract, the same cancel workflow
  owner should sequence `load owned task -> validate cancellable status ->
  load snapshot -> persist cancel result` directly instead of leaving that
  final orchestration split across the outer cancel function body.
- Batch task-view payload contract: if the batch read/query lane already owns
  the selected active task set plus snapshot-backed read-context projection,
  the same read-context owner should project active-task list items,
  active-project task payload, and single-generation existing-background
  payloads directly instead of returning raw `BatchGenerationReadContext`
  values for the neighboring query layer to map/find into payloads locally.
- Single-background existing payload owner-collapse contract: if the
  single-chapter background write lane already owns active-task query
  selection, chapter match filtering, and the final compat payload consumer,
  do not preserve a neighboring batch read-context projection helper that only
  reopens `BatchGenerationReadContext -> single-generation existing-background
  payload`; collapse that single-generation-specific payload projection back
  into the single-generation write owner and keep batch read-context scoped to
  remaining batch shared payload variants.
- Single-background existing payload variant-collapse contract: if the
  single-chapter background write lane already owns the
  single-generation-specific existing-background payload projection and only
  still depends on a neighboring batch task-view payload variant for the
  final `task_id/chapter_id/message/estimated_time_minutes` assembly, do not
  preserve that single-generation-specific variant inside the batch payload
  base; keep only the shared task-view payload base in the batch owner and
  collapse the single-generation-specific payload field assembly back into the
  single-generation write owner.
- Single-background existing read-context owner-collapse contract: if the
  single-chapter background write lane already owns the
  single-generation-specific existing-background payload projection and only
  still depends on a neighboring batch read-context owner chain for
  `recover active tasks -> load snapshots -> build read context -> match chapter`,
  do not preserve that single-generation-specific read-context chain inside
  the batch read-context owner; keep only the lower-level shared recovery /
  snapshot primitives reusable in batch, and collapse the
  single-generation-specific existing-background read-state/context owner back
  into the single-generation write owner.
- Single-background payload base owner-collapse contract: if the
  single-chapter background create lane and existing-background short-circuit
  lane already share one single-generation-local runtime/task payload shape,
  do not preserve neighboring batch task-view payload base or active-status
  helpers only for `task state -> single payload base` projection; collapse
  single-generation task/runtime payload base, stage semantics, active status
  semantics, and single-task estimated-duration semantics back into the
  single-generation prepare/write owner chain.
- Single-generation quality-status owner-collapse contract: if the
  single-chapter background payload lane and single runtime manual-review lane
  already share one chapter-scoped quality status contract, do not preserve a
  neighboring batch quality-status semantic shell only for
  `snapshot/runtime state -> chapter quality status` projection or
  `quality payload -> manual-review label` parsing; collapse those
  chapter-scoped quality-status semantics back into the single-generation
  owner chain and keep batch quality-status scoped to the remaining batch
  task/status/terminal lanes.
- Batch active-query envelope contract: if the batch task-view query lane
  already owns selected active task rows plus the final active-list or
  active-project payload projection, the same query owner should also project
  the compat response envelope (`{total, items}` / `{has_active_task, task}`)
  directly instead of letting the outer query entry rebuild those wrappers
  locally around the same owner-returned payloads.
- Quality-summary history contract: when quality summary owners materialize
  `quality_runtime_context.recent_metrics`, the sequence must stay
  newest-first so summary -> history -> rebuilt-summary flows preserve
  deterministic `recent_focus_areas` and latest-metric semantics.

### 4. Validation & Error Matrix
- Invalid request fields -> reject at route boundary before spawning work.
- Access / ownership failure -> reject at route boundary.
- Task orchestration failure -> service returns the domain error; route only
  translates it to transport form.
- Polling / stream state mismatch -> fix in stream helper, not with route-local
  field patches.
- Checkpoint / snapshot write failure -> fail in runtime helper and preserve a
  single error path for task status.

### 5. Good/Base/Bad Cases
- Good: route extracts request context, calls one service helper, and returns
  the compat response or stream. If runtime needs provider payload, it comes
  from a prepared request/workflow result object and is only forwarded here.
- Base: route keeps `tokio::spawn` and transport-specific wiring, while
  service owns the create/query/runtime logic.
- Bad: route mutates task status, builds checkpoint payloads, and assembles SSE
  events inline inside the same handler.
- Bad: active-task list, active-project query, or existing-background query
  loops through task rows and reloads one snapshot per task even though the
  neighboring batch query/read owner can materialize the same read-context set
  in one owner-controlled projection step.
- Bad: status-stream polling reloads `BatchGenerationReadContext` on every poll
  even though the polling lane only needs the stream-state projection and can
  consume one dedicated stream-state owner directly.
- Bad: status route or cancel/write response persistence first materializes
  `BatchGenerationReadContext` even though the caller only needs the status
  task payload and already owns the narrower task + runtime-state sources.
- Bad: owned batch status query already loaded the owned task, applied timeout
  recovery, and restored the snapshot, but the outer query layer still keeps a
  local `task + snapshot -> status payload` projection branch instead of
  consuming one owner-projected compat payload directly.
- Bad: single-generation background write workflow reloads an owned existing
  task as `BatchGenerationReadContext` and then projects the existing-task
  payload locally even though the adjacent query owner can return the compat
  payload directly.
- Bad: single-generation stream workflow already has one success completion
  owner, but still locally emits `complete`, quality events, result payload,
  and analysis-started follow-up events one by one instead of consuming one
  ordered owner-projected emission plan.
- Bad: single-generation background write workflow still owns
  `load target -> existing-task reuse -> prepare background launch -> persist`
  branching inline even though the neighboring owners already materialize the
  exact existing-payload and launch-plan contracts the workflow needs.
- Bad: single-generation background write workflow first materializes a
  workflow-entry owner and then still keeps the final
  `persist-and-dispatch(...)` handoff in the outer function body instead of
  consuming one workflow-start owner that sequences the whole branch.
- Bad: single-generation runtime driver still locally sequences
  preparing persistence, generation execution, and terminal persistence
  routing even though the runtime lane already has dedicated owners for each
  lifecycle stage.
- Bad: batch step execution still reloads the task, applies the cancelled
  gate, reloads the chapter, and validates project ownership inline even
  though the same runtime lane already has a dedicated step-preparation owner
  for the exact attempt-entry contract.
- Bad: batch runtime lifecycle still locally carries one per-step
  `prepare/retry/execute` branch even though the same runtime lane already
  has a dedicated step-execution owner for the exact chapter-step contract.
- Bad: batch runtime success flow still locally runs follow-up analysis,
  inspects quality-gate outcome, and persists the successful next-progress
  branch inline even though the same runtime lane already has a dedicated
  post-generation owner for the exact terminal handoff contract.
- Bad: batch runtime driver still locally sequences preparing persistence,
  chapter-id step iteration, and final stop/continue handoff even though the
  runtime lane already has dedicated owners for the lifecycle stages and
  per-step progression contract.
- Bad: batch generation attempt success path already has post-write guard,
  follow-up analysis, quality-gate routing, and post-generation persistence
  owners available, but the outer generation-attempt branch still locally
  sequences those success-only handoffs after content generation.
- Bad: batch resume command lane already owns invalid-status gating,
  persisted-runtime restore, manual-review blocker detection, and workflow
  runtime-state handoff, but the neighboring command/write layer still
  replays those restore checks locally before launch preparation.
- Bad: batch resume write workflow still calls
  `prepare_owned_batch_generation_resume(...)` and then separately invokes
  `persist_and_dispatch(...)` in the outer function body even though the
  neighboring resume lane already materializes the launch persistence plan the
  workflow needs.
- Bad: batch resume write workflow first materializes a workflow-launch owner
  and then still keeps the final `persist_and_dispatch(...)` handoff in the
  outer entry function body instead of consuming one workflow-start owner
  that sequences the whole resume branch.
- Bad: batch create write workflow finishes access check, but then still keeps
  local `prepare create workflow entry -> persist_and_dispatch(...)`
  orchestration in the outer function body even though the neighboring create
  lane can materialize the exact workflow-entry owner contract directly.
- Bad: batch create write workflow first materializes a workflow-entry owner
  after access check and then still keeps the final
  `persist_and_dispatch(...)` handoff in the outer entry function body instead
  of consuming one workflow-start owner that sequences the whole create
  branch.
- Bad: cancel persistence/runtime owners already materialize the cancelled
  status payload, but the neighboring cancel workflow still appends the final
  `Batch generation cancelled` summary envelope locally instead of returning
  the owner-projected final response directly.
- Bad: batch cancel route/service still locally sequences owned task loading,
  terminal-status validation, snapshot loading, and cancel persistence even
  though the neighboring cancel lane can materialize the same workflow owner
  contract directly.
- Bad: batch cancel already routes through the batch write-workflow owner, but
  a neighboring `chapter_batch_generation_cancel_service` file still reopens
  owned-source loading, terminal gating, or cancelled persistence-plan
  assembly through another module boundary even though those contracts already
  belong to the same write-lane owner chain.
- Bad: batch status stream already owns polling plus SSE emission, but a
  neighboring `chapter_batch_generation_stream_state_query_service` file still
  reopens the same shared owned-read-state -> stream-state projection through
  another module boundary even though that contract already belongs to the
  same status-stream owner chain.
- Bad: batch status query already owns shared owned read-state plus status
  payload semantics, but a neighboring
  `chapter_batch_generation_status_task_query_service` file still reopens the
  same owned-read-state -> status-payload projection through another module
  boundary even though that contract already belongs to the same read/query
  owner chain.
- Bad: batch task-view query first loads active `BatchGenerationReadContext`
  values, then locally performs `map / next / find` to build active-list,
  active-project, or existing-background payloads even though the neighboring
  read-context owner can project those payload variants directly.
- Bad: batch active-task query already owns the selected task set and final
  compat payload items, but the outer query entry still locally wraps them into
  `{total, items}` or `{has_active_task, task}` instead of consuming one
  owner-projected response envelope directly.
- Bad: batch resume restore path first materializes restored
  `request_runtime_state` / `runtime_state_seed`, then the neighboring command
  or write owner splits the same restored owner again to rebuild batch runtime
  launch input locally.
- Bad: single-chapter resume restore path already has a restored runtime
  owner, but the neighboring resume command branch still calls
  `into_launch_parts()` locally and rebuilds the single runtime launch input
  beside that owner.
- Bad: route or stream helper calls a local
  `resolve_default_prompt_context_provider_payload()` fallback even though the
  request/workflow preparation step can own that default once.
- Bad: a quality summary owner emits `recent_metrics` in oldest-first order
  while the summary-only restore path expects newest-first metrics, because
  that silently flips `recent_focus_areas` ordering after summary rebuild.

### 6. Tests Required
- Unit tests for active task/status response builders.
- Unit tests for SSE event builder and stream terminal conditions.
- Targeted route or integration checks that verify delegation still preserves
  request validation and compat response shape.
- Runtime tests for checkpoint/snapshot progression when changing task-flow
  semantics.
- When provider payload ownership moves across boundaries, add or update a
  focused unit test on the new owner object/helper rather than relying on a
  route-level smoke only.

### 7. Wrong vs Correct
#### Wrong
- Route handler parses request, loads task state, mutates checkpoint, formats
  status response, and pushes SSE event payloads inline.
- Route or stream helper recreates placeholder provider payload locally before
  dispatching runtime work.

#### Correct
- Route handler keeps transport concerns only, then delegates create/query/
  stream/runtime behavior to service-owned helpers with focused tests.
- Prepared request/workflow objects own default provider payload assembly once,
  and downstream route/stream/runtime boundaries only pass the explicit
  payload through.

## Scenario: Rust startup and runtime hardening boundary

### 1. Scope / Trigger
- Trigger: a change touches Rust startup/runtime boundary files such as
  `backend-rs/src/config.rs`, `backend-rs/src/main.rs`,
  `backend-rs/src/db/connection.rs`, `backend-rs/src/api/router.rs`,
  `backend-rs/src/api/auth.rs`, or `backend-rs/src/middleware/auth.rs`.
- Why this needs code-spec depth: these files own environment wiring,
  startup failure policy, credentialed browser access, cookie policy, and
  public-vs-protected route boundaries. A small local edit can silently widen
  runtime exposure or make deployment config lie about actual behavior.

### 2. Signatures
- `config::load() -> Result<AppConfig, ConfigError>` is the startup config
  entrypoint and must remain the owner of runtime-mode-sensitive validation.
- `db::connection::connect(cfg: &AppConfig)` consumes a validated
  `cfg.database_url`; it must not invent a second fallback policy.
- `api::router::build(...) -> Result<Router, RouterBuildError>` owns CORS
  layer construction and may fail when `CORS_ORIGINS` is invalid for the
  selected runtime mode.
- Auth cookie writers in `backend-rs/src/api/auth.rs` must route through one
  local cookie builder/helper boundary instead of ad hoc format strings.
- `middleware::auth::is_public(path: &str)` remains the owner of public-path
  auth bypass policy and should be expressed as explicit exact/prefix match
  tables, not a long inline boolean chain.

### 3. Contracts
- Environment keys:
  - `DEBUG=false` means non-development runtime policy.
  - `JWT_SECRET` is required in non-development; development may generate an
    ephemeral local secret with an explicit warning log.
  - `DATABASE_URL` is required in non-development; development may fall back
    to `sqlite::memory:` with an explicit warning log.
  - `CORS_ORIGINS` must be the actual router input. In non-development it must
    be either a comma-separated explicit origin list or startup/router build
    must fail. `*` is development-only.
- CORS contract:
  - credentialed browser flows are supported, so explicit-origin mode must
    keep `allow_credentials(true)`.
  - origin parsing must reject userinfo, path segments, query, fragment, and
    malformed absolute origins.
- Cookie contract:
  - shared attributes (`Path`, `SameSite`, `Max-Age`) come from one builder.
  - `HttpOnly` vs non-`HttpOnly` stays explicit at the call boundary.
- Public path contract:
  - health/docs/auth bootstrap endpoints and static asset prefixes may stay
    public only through the middleware owner boundary.
  - route composition in `router.rs` must not silently change module exposure
    while refactoring CORS or startup behavior.

### 4. Validation & Error Matrix
- Non-development + empty `JWT_SECRET` -> `ConfigError::MissingJwtSecret` and
  process exits during startup.
- Non-development + empty `DATABASE_URL` -> `ConfigError::MissingDatabaseUrl`
  and process exits during startup.
- Non-development + `CORS_ORIGINS="*"` -> `RouterBuildError::WildcardCorsOriginsNotAllowed`
  and process exits during router build.
- `CORS_ORIGINS` contains malformed or non-origin values -> `RouterBuildError::InvalidCorsOrigin`.
- Editing router composition and dropping an existing route merge -> treat as a
  behavioral regression even if `cargo check` still passes; restore the route
  and re-run focused validation.

### 5. Good/Base/Bad Cases
- Good: runtime mode is decided once in config loading, startup errors fail
  fast before serving traffic, router CORS behavior matches `CORS_ORIGINS`,
  cookie formatting flows through one helper, and public paths are auditable
  from one matcher table.
- Base: development keeps explicit convenience fallbacks for local bootstrap,
  but warnings make the fallback visible and non-development never reuses the
  same implicit behavior.
- Bad: `db::connection` or `router.rs` adds its own hidden fallback after
  config validation already ran.
- Bad: router refactor changes the `.merge(...)` chain and silently drops an
  existing route group while focusing on unrelated runtime hardening.
- Bad: a new auth cookie path reintroduces hand-built `Set-Cookie` strings
  outside the local cookie builder.

### 6. Tests Required
- Unit tests for runtime-mode-sensitive config helpers:
  - development allows ephemeral JWT / in-memory DB fallback
  - non-development rejects missing JWT / DB URL
- Unit tests for CORS parsing:
  - development wildcard allowed
  - non-development wildcard rejected
  - explicit origins normalized/deduplicated
  - path-bearing origins rejected
- Unit tests for cookie rendering:
  - `HttpOnly` cookie shape
  - frontend-visible cookie shape
  - clear-cookie shape
- Unit tests for public/protected path classification:
  - representative exact public paths
  - `/assets` prefix
  - representative protected API paths
- After router composition edits, run at least one targeted review against the
  `.merge(...)` list so route groups were not dropped accidentally.

### 7. Wrong vs Correct
#### Wrong
- `config::load()` silently generates a production secret or leaves
  `DATABASE_URL` empty and lets downstream code guess.
- `router.rs` ignores `cfg.cors_origins`, uses a permissive default for every
  mode, and accidentally removes an unrelated route merge in the same patch.
- Auth handlers build `Set-Cookie` strings in multiple helper variants with
  duplicated `Path` / `SameSite` fragments.
- Public route policy is encoded as an ever-growing inline boolean expression
  with no tests for representative paths.

#### Correct
- `config::load()` centralizes runtime mode classification and returns typed
  startup errors for non-development misconfiguration.
- `router.rs` builds CORS from validated config, preserves credential support,
  and keeps the route merge surface intact during refactors.
- Auth handlers use one cookie builder/helper boundary with explicit
  `HttpOnly` control.
- Public route policy stays local to `middleware/auth.rs` and is expressed as
  exact/prefix policy tables with focused tests.

## Scenario: Rust settings models provider contract boundary

### 1. Scope / Trigger
- Trigger: a change touches `backend-rs/src/api/settings.rs` model-discovery
  paths such as `get_available_models(...)`, `/settings/fetch-models`, or the
  helper functions that build provider-specific model-list requests.
- Why this needs code-spec depth: `settings/models` is not a generic “GET
  `/models`” wrapper. It is a provider contract boundary with Python-defined
  differences in candidate URL fallback, auth headers, friendly-empty
  semantics, and model filtering. Small local simplifications can silently
  widen route-group drift.

### 2. Signatures
- `GET /api/settings/models`
  - query: `api_key?`, `api_base_url?`, `provider?`
  - response shell: `provider`, `models`, `count`, optional `message`
- `POST /api/settings/fetch-models`
  - request body: `api_key?`, `api_base_url?`, `provider?`, `models_url?`
  - response shell: `success`, `models`, `message`, optional `error`,
    `error_type`
- Rust owner helpers should keep `settings/models` and `fetch-models`
  boundaries distinguishable even if they share parsing logic.

### 3. Contracts
- OpenAI-compatible providers for `settings/models` include:
  `openai`, `openai_responses`, `azure`, `newapi`, `custom`, `sub2api`.
- `settings/models` openai-compatible candidate order must remain explicit:
  - if base ends with `/v1`: try `{base}/models`, then root `/models`
  - otherwise: try `{base}/models`, then `{base}/v1/models`
- Candidate `404` on an earlier openai-compatible URL may continue to the next
  candidate instead of failing immediately.
- Azure contract:
  - auth header uses `api-key`, not Bearer
  - `404/403` or an empty model list returns `200` with empty `models` and a
    friendly guidance `message`
- Anthropic contract:
  - request real `{base}/v1/models`
  - include `x-api-key` and `anthropic-version: 2023-06-01`
- Gemini contract:
  - request `{base}/models?key=...`
  - only expose models whose `supportedGenerationMethods` contain
    `generateContent`

### 4. Validation & Error Matrix
- openai-compatible first candidate `404` + later candidate success ->
  continue and return the later candidate's models.
- Azure `404/403` -> `200` success shell with empty models and friendly
  guidance message.
- Azure empty payload -> same friendly empty success shell.
- Anthropic/Gemini non-success HTTP status -> preserve error path for the route
  owner; do not silently replace with curated success data.
- Unsupported provider -> explicit route/service error, not a generic openai
  fallback.

### 5. Good/Base/Bad Cases
- Good: provider-specific request building and payload parsing live behind one
  focused owner boundary with helper tests and route-level assertions.
- Base: `settings/models` keeps its current external shell while gaining more
  faithful Python provider semantics on the success path.
- Bad: Anthropic falls back to a Rust-local curated static list and claims the
  route is migrated.
- Bad: Gemini exposes embedding/non-generation models because the filter moved
  out of the owner boundary or was dropped.
- Bad: Azure uses Bearer auth or returns a generic fallback envelope instead of
  the Python-style empty-list guidance response.

### 6. Tests Required
- Helper tests for:
  - openai-compatible candidate ordering
  - provider-specific auth headers
  - openai-compatible payload parsing
  - anthropic payload parsing
  - gemini generation-capability filtering
  - Azure friendly empty-message contract
- Route-level focused tests for:
  - root `/models` -> `/v1/models` fallback success
  - Azure `404` returning `200 + [] + message`
- If a future slice changes error-shell parity too, add explicit assertions for
  `400 detail` behavior instead of relying on success-path tests only.

### 7. Wrong vs Correct
#### Wrong
- Reuse one generic openai `/models` implementation for every provider and
  ignore Azure/Anthropic/Gemini differences.
- Replace provider drift with curated success payloads because it is easier to
  test than real provider contracts.
- Let `settings/models` and `fetch-models` collapse into one undifferentiated
  route contract even though they intentionally serve different shells.

#### Correct
- Keep provider-specific request building and parsing local to the
  `settings/models` owner boundary, with focused tests near that owner.
- Migrate the real Python success-path semantics first, then separately decide
  whether error-shell parity is worth the next slice.
- Reuse shared parsing/building helpers where possible, but do not flatten away
  externally observable provider differences.

## Scenario: Rust settings function-calling probe contract boundary

### 1. Scope / Trigger
- Trigger: a change touches `backend-rs/src/api/settings.rs`
  `check_function_calling(...)`, or the Rust AI owner path it depends on
  (`backend-rs/src/ai/service.rs`, `backend-rs/src/ai/clients/openai.rs`,
  `backend-rs/src/ai/clients/anthropic.rs`, `backend-rs/src/ai/types.rs`).
- Why this needs code-spec depth: `settings/check-function-calling` is not just
  a convenience probe. Python already defines a stable success/error shell,
  forced-tool-call behavior, and details payload. Local simplifications can
  silently widen provider capability drift and mislead Phase 5 shrink decisions.

### 2. Signatures
- `POST /api/settings/check-function-calling`
  - request body currently reuses the settings probe request shape:
    `api_key?`, `api_base_url?`, `provider?`, `llm_model?`, `temperature?`,
    `max_tokens?`
  - response shell:
    - `success`
    - `supported`
    - `message`
    - `response_time_ms`
    - `provider`
    - `model`
    - optional `tool_calls`
    - optional `response_preview`
    - optional `error`, `error_type`, `suggestions`
    - `details`
- Rust AI abstraction signatures must be able to carry explicit tool-choice
  policy through the owner path, rather than forcing all tool calls onto
  provider defaults.

### 3. Contracts
- The real probe tool is `get_weather`, not a Rust-local placeholder tool.
- The route must force a tool call with `tool_choice = required`.
- OpenAI-compatible providers must send `tool_choice = "required"` when that
  override is requested.
- Anthropic must send the equivalent `{"type":"any"}` override when that
  override is requested.
- Gemini must not be treated as an OpenAI-compatible `/chat/completions`
  provider on this lane; its Rust owner path should use the native
  `generateContent` contract and still map tool-call parts back into the
  shared probe response shell.
- When tools are present but no explicit override is requested, Rust should
  preserve existing `auto` behavior instead of silently changing all tool calls
  to required/none.
- Success-path shell must match Python semantics:
  - tool calls present -> `success = true`, `supported = true`
  - request succeeded but model answered in plain text ->
    `success = true`, `supported = false`
- `details` must keep the minimal stable probe contract:
  - `endpoint_diagnostics`
  - `finish_reason`
  - `has_tool_calls`
  - `tool_call_count`
  - `test_tool`
  - `response_type`
- Failure-path shell must keep:
  - `success = false`
  - `supported = null`
  - `details.endpoint_diagnostics`
  - when probe transport has already identified an HTTP status,
    `details.http_status_code`
  - status-aware `message` semantics for at least:
    - `5xx` -> upstream temporarily unavailable
    - `429` -> rate-limited / cannot confirm capability yet
    - `401` -> authentication failed
    - `404` -> endpoint or model unavailable
    - timeout -> timeout wording instead of the generic function-calling
      failure sentence
  - provider/gateway/base-url-aware `suggestions`; do not regress this route
    back to generic "check model/tool support" text when the failure is
    actually transport or gateway drift
  - if the AI owner path already carries a structured HTTP status, route logic
    should consume that status directly; message parsing is only a compatibility
    fallback, not the preferred owner path

### 4. Validation & Error Matrix
- provider returns tool calls -> route returns success shell with
  `supported = true` and includes `tool_calls`.
- provider returns plain text only -> route still returns success shell with
  `supported = false` and `response_preview`.
- timeout / runtime failure -> route returns failure shell with
  `supported = null`, `error_type`, `suggestions`, and endpoint diagnostics.
- HTTP failure with a known status -> route should also preserve
  `details.http_status_code`, and the top-level `message` should reflect the
  Python status-specific shell instead of a single generic failure sentence.
- AI owner path adds explicit tool-choice support -> adjacent callers that do
  not request an override must keep the previous `auto` behavior.

### 5. Good/Base/Bad Cases
- Good: `check-function-calling` owns the Python-aligned probe contract, while
  the AI abstraction exposes one reusable `ToolChoice` capability that other
  owners may opt into explicitly.
- Base: this slice migrates the core success/error shell and forced tool-call
  semantics first, without claiming full backup/fallback/request-options parity
  in the same patch.
- Bad: the route uses a local placeholder tool such as `ping_tool` and still
  claims the seam is migrated.
- Bad: plain-text unsupported responses are flattened into `success = false`,
  which makes provider capability drift look like transport failure.
- Bad: explicit `required` support is added for this route but the default
  `auto` behavior for existing tool-enabled callers is accidentally changed.

### 6. Tests Required
- Client-level tests for:
  - OpenAI tool-choice serialization
  - Anthropic tool-choice serialization
  - Gemini tool schema conversion / native response parsing where that provider
    owner path is touched
- Focused route/helper tests for:
  - endpoint-diagnostics shell
  - tool-call-supported response path
  - plain-text unsupported response path
  - gateway / status-aware failure guidance path
  - Gemini success-path probes proving the route is not still going through an
    OpenAI-compatible fake owner path
- Targeted validation commands:
  - `cargo test settings --manifest-path "backend-rs/Cargo.toml" -- --nocapture`
  - `cargo test serializes_required_and_named_tool_choice --manifest-path "backend-rs/Cargo.toml" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml"`

### 7. Wrong vs Correct
#### Wrong
- Route uses a fake Rust-local tool, leaves tool choice on provider default,
  and treats `tool_calls` as the only success condition.
- AI abstraction hardcodes `required` for every tool-enabled request and
  accidentally changes unrelated runtime behavior.
- Failure shell omits endpoint diagnostics, making probe regressions harder to
  attribute to provider vs transport vs config.

#### Correct
- Route uses the Python-aligned tool definition and explicit required
  tool-choice override only for this owner path.
- AI abstraction exposes configurable tool choice, while callers without an
  explicit override keep the previous `auto` behavior.
- Route tests assert both the supported path and the plain-text unsupported
  path, because Python treats both as successful probe executions with
  different capability results.

## Scenario: Rust settings API-connection probe contract boundary

### 1. Scope / Trigger
- Trigger: a change touches `backend-rs/src/api/settings.rs`
  `test_api_connection(...)`, the preset-test path that reuses it, or adjacent
  probe helper functions such as `build_probe_endpoint_diagnostics(...)` and
  `build_probe_details(...)`.
- Why this needs code-spec depth: `settings/test` is not a generic “ping the
  model” route. Python already defines a stable request shape, success/error
  shell, normalized endpoint diagnostics, and preset-side probe reuse. Small
  simplifications can silently widen drift while still leaving the route
  apparently “working”.

### 2. Signatures
- `POST /api/settings/test`
  - request body reuses the settings probe shape:
    `api_key?`, `api_base_url?`, `provider?`, `llm_model?`, `temperature?`,
    `max_tokens?`, `api_backup_urls?`, `fallback_strategy?`
  - response shell:
    - `success`
    - `message`
    - `response_time_ms`
    - `provider`
    - `model`
    - optional `response_preview`
    - optional `error`, `error_type`, `suggestions`
    - `details`
- `POST /api/settings/presets/{preset_id}/test` must preserve the same probe
  input shape when it forwards preset config into `test_api_connection(...)`,
  including backup URLs and fallback strategy fields.

### 3. Contracts
- The probe must clamp the effective runtime request to `probe_max_tokens <= 64`
  while preserving the caller-facing `max_tokens` value separately in
  `details.max_tokens`.
- Success-path shell must match Python semantics:
  - `success = true`
  - `message = "API 连接测试成功"`
  - `response_preview` is the response text truncated to the short probe
    preview window
  - `details` includes:
    - `api_available`
    - `model_accessible`
    - `response_valid`
    - `temperature`
    - `max_tokens`
    - `probe_max_tokens`
    - `endpoint_diagnostics`
- `endpoint_diagnostics` must keep the stable normalized probe shell:
  - `primary_endpoint`
  - `backup_endpoints`
  - `configured_endpoint_count`
  - `fallback_strategy`
  - `auto_failover_enabled`
- Backup endpoints must be normalized by trimming whitespace, removing a
  trailing `/`, and deduplicating while preserving order.
- Failure-path shell must keep:
  - `success = false`
  - `error`
  - `error_type`
  - `suggestions`
  - `details.endpoint_diagnostics`
  - when a non-openai-compatible detailed client such as Anthropic already
    knows the HTTP status, it should also carry that status through the
    structured `AIRequestError.status_code` owner path instead of forcing the
    route back onto provider-specific message parsing
- When the route relies on OpenAI-compatible chat-completions candidate
  probing, Python-owned local gateway transport semantics also matter:
  - local `https://127.0.0.1` / `https://localhost` probes may continue to
    an `http://...` variant
  - when running in Docker, loopback candidates may also expand to
    `host.docker.internal`
  - candidate continuation may be triggered by network/TLS transport failures,
    not only HTTP status drift or non-JSON/base-URL-shape failures
- Until Rust owns deeper transport/runtime parity for this route, do not claim
  full Python `request_options` or `transport_diagnostics` parity in the same
  slice unless that ownership is actually implemented and tested.

### 4. Validation & Error Matrix
- Probe request succeeds with a text response -> return the Python-style
  success shell and details payload.
- Probe request times out -> return `success = false`, `message` indicating
  timeout, and preserve endpoint diagnostics.
- Backup URLs / fallback strategy are supplied -> endpoint diagnostics must
  reflect the normalized backup list and failover flag, even if the Rust AI
  client has not yet migrated full fallback execution semantics.
- Local HTTPS gateway candidate fails at the transport layer -> if Python's
  owner contract would continue to a local HTTP or docker-host candidate, Rust
  must not stop on the first transport error.
- Preset test path forwards probe config -> it must not silently drop
  `api_backup_urls` or `fallback_strategy`.

### 5. Good/Base/Bad Cases
- Good: the route owns the stable Python response shell and normalized probe
  diagnostics locally, while leaving deeper transport/client parity to a later
  explicit owner slice.
- Base: one slice migrates request-body compatibility, endpoint diagnostics,
  and success/error shell parity first, without over-claiming that the full
  failover transport implementation is done.
- Good: a later explicit transport-owner slice may narrow local candidate
  expansion or network-error continuation to the Python-owned contract, but it
  must prove that behavior with focused tests instead of only updating route
  wording or suggestion text.
- Bad: the route keeps a Rust-only top-level `probe_max_tokens` field and does
  not move it back into Python-style `details`.
- Bad: preset probe reuse forwards only API key/base/model and silently drops
  backup URLs or fallback strategy from preset config.
- Bad: endpoint diagnostics are rebuilt differently across `settings/test` and
  `check-function-calling`, so the same base URL / backup input yields drifted
  probe metadata.
- Bad: local HTTPS gateway probes fail in Rust because the owner path stops at
  the first TLS/network error, even though Python would continue to a valid
  local HTTP or docker-host candidate.

### 6. Tests Required
- Focused helper tests for:
  - minimal endpoint-diagnostics shell
  - backup URL normalization + manual fallback diagnostics
- Focused route tests for:
  - Python-style `settings/test` success shell and details payload
  - preset probe request adapter keeping backup URLs / fallback strategy
  - local HTTPS gateway probe succeeding via a later HTTP candidate when the
    owner transport contract allows it
  - non-openai-compatible detailed gateway failures preserving
    `details.http_status_code` via the structured error carrier
- Focused client/helper tests for:
  - local gateway candidate expansion order
  - network-error-driven candidate continuation when Python owner semantics
    depend on it
  - Anthropic detailed HTTP failures preserving `status_code` at the client
    owner boundary instead of only in string messages
- Logged-in strangler smoke for route-group cutover should prefer:
  - stable `200 + failure shell` assertions for provider-test lanes
  - shell-level keys such as `success`, `message`, `error`, `error_type`,
    `suggestions`, `details`
  instead of requiring a real upstream provider success result
- Targeted validation commands:
  - `cargo test settings --manifest-path "backend-rs/Cargo.toml" -- --nocapture`
  - `cargo test settings_test_preset_request_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml"`

### 7. Wrong vs Correct
#### Wrong
- Route returns a Rust-local shell that places `probe_max_tokens` at top level,
  omits Python-style `details`, and treats backup/fallback inputs as irrelevant.
- Preset test builds a narrower request than `settings/test`, so the same saved
  config behaves differently depending on which route was used.
- Logged-in strangler smoke requires a live external provider success path, so
  route-group cutover readiness fails for network noise rather than actual
  owner/fallback drift.

#### Correct
- Route returns the Python-style `details` shell, including normalized endpoint
  diagnostics and the clamped probe token value.
- Preset test forwards the same probe input contract, including backup URLs and
  fallback strategy, into the shared API-connection probe owner.
- Logged-in strangler smoke proves the request reaches the real business
  handler by asserting the stable failure shell first, and leaves true
  provider-success parity to a later transport-owner slice.

## Scenario: Rust outline compact requirement / guidance owner boundary

### 1. Scope / Trigger
- Trigger: a change touches
  `backend-rs/src/services/outline_requirement_service.rs`,
  `outline_generation_request_service.rs`, `wizard_service.rs`, or adjacent
  outline prompt-assembly helpers.
- Why this needs code-spec depth: the same user-visible outline create /
  continue flow is split across runtime requirement blocks, compact prompt
  budgets, and guidance-card lanes. Small local edits can silently reintroduce
  Python-owned compact semantics or let value vocabularies drift apart.

### 2. Signatures
- `build_wizard_outline_requirements(...) -> String` owns opening-outline
  runtime requirement assembly for Rust.
- `build_continue_outline_requirements(...) -> String` owns continue-outline
  runtime requirement assembly for Rust.
- `build_compact_outline_guidance_blocks(...) -> Vec<String>` is the compact
  guidance-card owner for outline mode and must be consumed only on
  `compact_mode=true` outline paths.
- Request/workflow callers may pass raw compat strings, but normalization of
  `creative_mode`, `story_focus`, and `quality_preset` belongs to the outline
  requirement owner boundary.

### 3. Contracts
- For `compact_mode=false`:
  - keep the existing runtime requirement merge lane stable
  - `【运行时创作偏好】` may remain as a legacy merged owner block
- For `compact_mode=true` on outline flows:
  - Rust must own the compact guidance-card lane directly
  - do not collapse the lane back into a single merged preference block
  - representative cards should come from Rust-owned builders such as:
    - `【质量预设】`
    - `【结构蓝图】`
    - `【大纲目标卡】`
    - `【大纲结果卡】`
    - `【大纲爽点回收卡】`
    - `【大纲设定落地卡】`
    - `【大纲开篇钩子卡】`
    - `【大纲结尾悬停卡】`
    - `【大纲角色弧光卡】`
    - `【大纲执行清单】`
- Canonical compat value vocabularies must match the active project
  request/frontend contracts:
  - `creative_mode`: `balanced`, `hook`, `emotion`, `suspense`,
    `relationship`, `payoff`
  - `story_focus`: `advance_plot`, `deepen_character`,
    `escalate_conflict`, `reveal_mystery`, `relationship_shift`,
    `foreshadow_payoff`
  - `quality_preset`: `balanced`, `plot_drive`, `immersive`,
    `emotion_drama`, `clean_prose`
- Old aliases may normalize into the canonical values for compatibility, but
  new Rust tests and new prompt ownership must assert the canonical lane.

### 4. Validation & Error Matrix
- `compact_mode=true` + recognized preference values ->
  compact outline requirements include representative guidance cards and still
  respect `OUTLINE_RUNTIME_REQUIREMENT_TOTAL_LIMIT`.
- `compact_mode=true` + legacy alias values ->
  normalize to canonical values before card labeling / branching.
- `compact_mode=false` ->
  preserve the non-compact requirement merge lane; do not silently drop the
  legacy runtime preference block unless the full non-compact guidance owner
  is migrated in the same slice.
- Adding a new compat preference value without updating normalization / labels
  -> treat as a migration regression even if `cargo check` still passes.

### 5. Good/Base/Bad Cases
- Good: Rust compact outline mode emits real guidance cards with focused tests,
  while non-compact mode remains stable and behavior-preserving.
- Base: one slice migrates only the `compact_mode=true` active path and leaves
  the larger non-compact lane untouched.
- Bad: route/request code or unrelated workflow layers recreate compact
  guidance text locally instead of delegating to the requirement owner.
- Bad: compact mode still reports progress through `【运行时创作偏好】` only,
  with no real card-level ownership shift.
- Bad: tests continue asserting old non-canonical values such as
  `relationship_tension` or `tight_prose` after the request/frontend contract
  has moved on.

### 6. Tests Required
- Unit tests in `outline_requirement_service.rs` for:
  - non-compact wizard requirement merge
  - non-compact continue requirement merge
  - compact outline requirements containing representative guidance cards
  - compact outline output staying under the configured total budget
- Focused route/request tests in adjacent request/workflow services proving the
  compact-mode flag still flows through unchanged.
- When value aliases are added or removed, update or add focused normalization
  assertions near the requirement owner instead of relying only on route-level
  smoke.

### 7. Wrong vs Correct
#### Wrong
- `compact_mode=true` only truncates a merged preference block and claims the
  compact guidance lane is migrated.
- Request/workflow code hand-builds compact outline cards while the service
  owner still only knows how to emit `【运行时创作偏好】`.
- Canonical frontend/API values drift, but Rust requirement labels keep
  matching stale values and tests keep asserting those stale literals.

#### Correct
- `outline_requirement_service.rs` owns compact outline guidance cards
  directly, and the create/continue flows only forward explicit inputs.
- Legacy aliases normalize once at the requirement owner boundary, then the
  compact card builders branch on canonical values.
- Tests assert representative card presence and budget behavior on the real
  compact path, not just helper existence.

---

## Change Checklist

- If you changed a route payload, did you update the related Pydantic schema,
  frontend consumer, and tests?
- If you changed a model, did you review migration impact, default semantics,
  and recovery/state consumers?
- If you edited a compat or facade file, did you confirm whether the real
  implementation also needs changes?
- If the change affects long-running tasks, did you inspect persistence,
  resume, polling, and SSE consumers?

---

## Testing Expectations

- Run backend pytest when feasible.
- Reuse the existing split:
  - API tests in `backend/tests/test_api/`
  - service tests in `backend/tests/test_services/`
  - schema tests in `backend/tests/test_schemas/`
- Async tests are normal here; `pytest.ini` is configured with
  `asyncio_mode = auto`.
- Prefer adding or updating targeted tests near the affected area instead of
  one oversized integration test when a narrower regression test is enough.

---

## Common Mistakes

- Editing a compat layer without checking the underlying implementation.
- Treating task/runtime tables as simple CRUD state even though UI recovery and
  polling depend on them.
- Changing request/response fields without tracing frontend consumers.
- Assuming `/health` is enough when readiness and DB warmup semantics matter.
- Refactoring `backend-rs/src/api/router.rs` and accidentally dropping an
  existing `.merge(...)` route group while working on unrelated middleware or
  CORS changes.

---

## Examples

- Thin route, rich service split:
  `backend/app/api/background_tasks.py`,
  `backend/app/services/background_task_manager.py`
- Bootstrap-owned exception handling and readiness:
  `backend/app/bootstrap/app_factory.py`
- Targeted API tests:
  `backend/tests/test_api/test_settings.py`

## Forbidden / Discouraged Patterns

- Do not pile new business logic into `app/api/`.
- Do not use startup-time schema mutation as a shortcut around migrations.
- Do not add new functionality to frozen compatibility facades unless the task
  is explicitly transitional.
