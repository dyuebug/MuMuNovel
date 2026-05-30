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
- `chapter_batch_generation_status_semantics_service.rs` now also owns the
  execution-mode literal as a single-branch contract; the previous
  single/batch `match` no longer provided any meaningful ownership split, so
  the helper now exposes a true constant contract instead of a fake
  task-dependent branch
- `chapter_batch_generation_task_payload_base_service.rs` now owns the shared
  checkpoint override path for resume-style response payloads, so response
  adapters no longer patch nested checkpoint fields in place
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
- single-chapter write-workflow request ownership is now narrower:
  - `chapter_single_generation_write_workflow_service.rs` now owns the
    route-payload -> write-workflow request conversion through one shared
    helper instead of repeating the same conversion in background and stream
    entrypoints
  - this keeps the route-compatible request shape unchanged while reducing
    one more write-lane owner split before runtime launch
- batch write-workflow execution-config ownership is now narrower:
  - `chapter_batch_generation_write_workflow_service.rs` now prepares the
    explicit `AIConfig + provider_payload` execution config before handing off
    to runtime launch for both create and resume flows
  - `chapter_batch_generation_runtime_launch_service.rs` now consumes that
    prepared config as an explicit input and narrows to launch assembly plus
    dispatch, instead of reloading execution config inside the launch owner
  - this keeps create/resume payloads and runtime behavior stable while making
    the write-workflow boundary consistent with single-chapter generation
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

Updated file-level migration map after the latest audit:

| Python file | Rust counterpart | Status | Planning note |
|-------------|-------------------|--------|---------------|
| `chapter_analysis_task_routes.py` | `backend-rs/src/api/chapter_analysis_routes.rs` | Migrated | Python shell can be removed when route parity is no longer needed. |
| `chapter_annotation_routes.py` | `backend-rs/src/api/chapter_crud_routes.rs` + `chapter_annotation_query_service.rs` | Migrated | Rust owns the query boundary; Python is now legacy compatibility surface. |
| `chapter_batch_generation_routes.py` | `backend-rs/src/api/chapter_batch_generation.rs` | Migrated | Main chapter-generation/batch surface is already on Rust. |
| `chapter_draft_routes.py` | `backend-rs/src/api/chapter_analysis_routes.rs` + draft services | Migrated | Draft route logic is already owned by Rust services/routes. |
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
- Future Phase 5 work should prefer removing the remaining legacy shells only
  if the Rust route/service owner already exists and the change reduces drift,
  not if it merely renames wrappers.

## Validation Strategy

- Run `cargo check` after each completed slice with the shared target dir:
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo check`
- Add focused unit tests when extracting or tightening pure helpers.
- Prefer narrow regression protection in touched service files over broad test
  churn.

## Rollback Shape

- Roll back only the latest seam slice if validation fails.
- Do not mix unrelated refactor moves in the same execution batch.
