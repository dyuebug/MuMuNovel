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

## Scenario: Batch Generation Post-Write Guard

### 1. Scope / Trigger

- Trigger: Batch chapter generation advances from one chapter to the next.
- Owner: `backend-rs/src/services/chapter_batch_generation_runtime_state_service/runtime_driver_owner.rs`.

### 2. Signatures

- `BatchGenerationPostWriteGuardPlan::execute(db, task_id) -> Result<BatchGenerationPostWriteGuardOutcome, String>`.
- `BatchGenerationPostWriteGuardPlan::resolve(task_exists, chapter_content_written) -> BatchGenerationPostWriteGuardOutcome`.
- `build_non_applied_generated_result_quality_runtime_state(result) -> Value`.

### 3. Contracts

- If `GeneratedChapterResult.content_applied == false`, route only retryable
  quality-gate results through `BatchGenerationQualityGateRoutingPlan` before
  post-write guard.
- `quality_gate_action=retry` should retry the current chapter through existing
  retry persistence.
- `quality_gate_action=manual_review` is telemetry-only for all chapter
  generation paths and must not persist failed quality-gate terminal fields or
  set `review_required=true`.
- A generated chapter is successful only after the chapter row exists and `chapters.content.trim()` is non-empty.
- A task row missing after generation resolves to `Stop` because the runtime was externally removed or cancelled.
- A chapter row missing resolves to `Stop` because the target no longer exists.
- A chapter row with empty or missing content after an applied result returns
  `Err("章节生成完成后正文未写入")` so the existing generic retry routing handles the current chapter.

### 4. Validation & Error Matrix

- Task exists + non-empty content -> `Continue`.
- Task missing -> `Stop`.
- Chapter missing -> `Stop`.
- Non-applied candidate result + manual review gate -> no quality-gate terminal;
  the manual-review metadata remains telemetry only.
- Non-applied candidate result + retry gate -> retry current chapter.
- Applied candidate result + empty content -> error routed as current-chapter generation failure.

### 5. Good/Base/Bad Cases

- Good: Chapter 2 writes non-empty content, post-write guard continues, then chapter 3 prerequisite check can pass.
- Base: Chapter 2 returns a manual-review candidate, quality telemetry is kept
  but no `需复核` terminal is created.
- Base: Chapter 2 writes empty content after an applied result, post-write guard fails chapter 2 and retries chapter 2.
- Bad: Treating chapter-row existence as success lets the driver advance to chapter 3, which later fails with `前置章节尚未完成: 2 章`.
- Bad: Converting manual-review telemetry into `review_required=true` blocks
  unattended chapter generation.

### 6. Tests Required

- Unit test `should_continue_post_write_guard_only_when_task_exists_and_chapter_content_written`.
- DB-backed test `should_fail_post_write_guard_when_generated_chapter_content_is_empty`.
- Unit test `should_not_project_manual_review_generated_result_into_quality_gate_runtime_state`.
- Run focused runtime-state tests or `cargo test ... <guard test>` plus `cargo check --manifest-path backend-rs/Cargo.toml`.

### 7. Wrong vs Correct

#### Wrong

```rust
let chapter_exists = chapter::Entity::find_by_id(&chapter_id).one(db).await?.is_some();
BatchGenerationPostWriteGuardPlan::resolve(task_exists, chapter_exists)
```

#### Correct

```rust
let content_written = chapter.content.as_ref().is_some_and(|content| !content.trim().is_empty());
if !content_written {
    return Err("章节生成完成后正文未写入".to_string());
}
BatchGenerationPostWriteGuardPlan::resolve(task_exists, true)
```

## Scenario: Chapter Generation Manual Review Policy

### 1. Scope / Trigger

- Trigger: Single-chapter or batch chapter generation receives
  `quality_gate_action=manual_review` from the candidate gateway or follow-up
  analysis.
- Owner:
  `backend-rs/src/services/chapter_single_generation_result_lifecycle_service/lifecycle_owner.rs`
  and
  `backend-rs/src/services/chapter_single_generation_runtime_state_service/terminal_state_owner.rs`
  and
  `backend-rs/src/services/chapter_batch_generation_task_payload_base_service/quality_terminal_status_owner.rs`
  and
  `backend-rs/src/services/chapter_batch_generation_runtime_state_service/runtime_driver_owner.rs`
  and
  `backend-rs/src/services/chapter_batch_generation_read_context_service/stream_state_owner.rs`
  and
  `backend-rs/src/services/chapter_generation_runtime_service/story_repair_quality_context_owner.rs`.

### 2. Contracts

- Chapter generation must not stop in `quality_blocked` or create a failed
  terminal task only because quality metadata says `manual_review`.
- `manual_review` quality metadata remains valid telemetry in history payloads,
  candidate gateway payloads, and quality metrics.
- The generated chapter content is applied like a normal completed generation
  when the action is `manual_review`; batch generation must not create a
  `review_required=true` terminal from manual-review metadata.
- Read/status projection must not convert `manual_review` telemetry back into
  task-card `quality_blocked`, `review_required`, 422 SSE error events, or
  user-facing `需复核` / `人工复核` copy.
- Severe quality analysis should use repair-oriented user-facing copy such as
  `auto_repair`, `需修复`, or `建议继续修复`; `manual_review` remains a legacy
  metadata value only.
- `retry` / `auto_repair` remains the blocking quality gate path for automatic
  repair attempts.

### 3. Tests Required

- Unit test that `generated_result_lifecycle_view(..., Some("manual_review"), ...)`
  returns `content_applied=true`, `attempt_state=applied`, and
  `chapter_status=completed`.
- Unit test that single-generation terminal-state resolution returns `None`
  for both direct manual-review candidate metadata and follow-up analysis
  manual-review metadata.
- Unit test that batch terminal semantics returns `None` for manual-review
  quality runtime state and that non-applied manual-review generated results do
  not create an active repair payload.
- Unit test that batch read-context projection treats manual-review quality
  metadata as telemetry only and does not emit `quality_blocked` or a 422
  manual-review SSE event.

## Scenario: Batch Generation Auto-Recovery Timeout

### 1. Scope / Trigger

- Trigger: Batch task read/status/list queries evaluate whether a `pending` or
  `running` generation task should be automatically recovered as failed.
- Owner:
  `backend-rs/src/services/chapter_batch_generation_read_context_service/task_recovery_owner.rs`
  and read callers in
  `backend-rs/src/services/chapter_batch_generation_read_context_service/`.

### 2. Contracts

- `pending` startup recovery still uses `batch_generation_tasks.created_at`
  and the existing 3-minute startup budget.
- `running` recovery must not use only `batch_generation_tasks.started_at`.
  Multi-chapter generation can legitimately exceed 15 minutes while still
  making progress.
- `running` recovery must use the latest known activity timestamp from
  `batch_generation_snapshots.updated_at`,
  `batch_generation_snapshots.workflow_runtime_state.updated_at`, then fall
  back to `batch_generation_tasks.started_at`.
- Owned task reads and active task list reads should pass already loaded
  snapshots into recovery instead of loading the same snapshot twice.
- Snapshot writes from chapter-start, retry, analysis, candidate, and terminal
  checkpoints are the runtime heartbeat source for long-running batches.

### 3. Tests Required

- Unit test that a running task older than 15 minutes is not recovered when its
  snapshot/runtime heartbeat is recent.
- Unit test that a running task is recovered when both snapshot/runtime
  heartbeat and `started_at` are stale.
- Contract test should expose `running_timeout_basis` so future changes keep
  the heartbeat-based recovery rule visible.
- Run `cargo test --manifest-path backend-rs/Cargo.toml chapter_batch_generation_read_context_service`
  and `cargo check --manifest-path backend-rs/Cargo.toml`.

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

### Fast-Execution Protocol for Rust Migration Rounds

When the user asks to accelerate Python-to-Rust migration, do not spend a new
round re-explaining the whole migration history. Use the latest task
checkpoint as the source of truth and keep analysis bounded.

Required round shape:

1. State the selected package in one sentence.
2. Confirm the Python source map and Rust owner map in at most five bullets.
3. Edit the Rust owner as a whole file, function group, or module package.
4. Run focused tests plus `cargo check` with an explicit non-`C:` target dir
   when requested by the user or environment constraints.
5. Record only the new delta, validation result, next owner boundary, and
   rollback note.

Analysis budget:

- Do not re-read or restate old checkpoints unless a referenced file moved.
- Do not enumerate every migrated Python route in normal implementation
  rounds; use the migration table instead.
- Do not do exploratory whole-backend scans when the active package is already
  named and the owner files are known.
- Use micro-analysis only to verify a concrete boundary, not to delay editing.

Progress accounting:

- Count as progress: a Rust route/service owner becomes more complete, a
  Python fallback can be frozen/repointed/removed, a smoke/rollback gap closes,
  or schema/startup assumptions move into an explicit Rust owner.
- After manifest readiness reaches `python-fallback = 0`, do not use fallback
  count as the primary progress metric. Count whole-module Rust owner closeout,
  logged-in business smoke, rollback/freeze policy, and explicit source-map
  deletion/freeze evidence instead.
- Do not count as progress: renaming wrappers, moving a trivial helper, adding
  tests with no owner change, or editing Python compatibility shells before
  Rust owner validation.
- If a runtime helper bridge is collapsed into a stronger Rust owner, update
  the related wiring/readiness owner in the same migration lane. The readiness
  map must name the real Rust owner files and must not keep deleted
  forwarding-only bridge modules as target files.
- If background task launch parts own task seed, startup snapshot, and runtime
  input materialization, they should also own persistence/dispatch unless a
  route or workflow file adds a real branch, validation boundary, or transport
  contract. Do not leave task insert plus runtime spawn split across a weaker
  write shell when the launch-parts owner is already the coherent Rust
  production owner.
- If a response projection module has only one production/smoke consumer and
  does not own transport shaping, error translation, branch selection, or a
  rollback knob, collapse it into the runtime/launch owner that already owns
  the restored state and payload contract. Do not keep forwarding-only
  response shells as migrated Rust target files.
- If a terminal-state module only computes checkpoint patches, failed-chapter
  entries, or quality-gate terminal labels for one runtime-state persistence
  owner, collapse it into that runtime-state owner. Terminal persistence,
  checkpoint merge, failed-chapter append, and smoke readiness labels should
  name the real runtime owner instead of a forwarding-only terminal shell.
- If a batch status stream only consumes owned read-state and emits the
  route-facing SSE stream projection, collapse it into
  `chapter_batch_generation_read_context_service.rs`. Do not recreate
  `chapter_batch_generation_status_stream_service.rs`; read-state loading,
  stream-state projection, cursor resolution, and SSE event materialization
  are one read-context/status owner boundary unless a new transport, fallback,
  rollback, or route branch appears.
- If a batch owned-task query module only loads the owned task, snapshot, and
  recovered read-state used by status, stream, resume, cancel, and route error
  mapping, collapse it into `chapter_batch_generation_read_context_service.rs`.
  Do not recreate `chapter_batch_generation_owned_task_query_service.rs`; the
  shared task lookup/source/read-state error contract belongs with the
  read-context owner unless it gains a separate schema, fallback, or transport
  boundary.
- If active single-generation smoke/readiness evidence is used for deploy or
  fallback-shrink decisions, its manifest expectation must name the real Rust
  route/workflow/runtime owner chain, not deleted projection shells. Do not keep
  `chapter_single_generation_stream_success_response_service`,
  `chapter_single_generation_background_response_service`, or
  `chapter_single_generation_terminal_state_service` in
  `deploy/strangler-gateway-probes.json` after those contracts have collapsed
  into `chapter_single_generation_stream_workflow_service`,
  `chapter_single_generation_runtime_restore_service`, and
  `chapter_single_generation_runtime_state_service`.
- If the shared candidate executor wiring plan is used for cutover readiness,
  validation must reject retired Rust target files as well as missing current
  owners. Do not keep `chapter_candidate_route_gateway_smoke_service.rs` after
  smoke/readiness probes move into `chapter_candidate_route_gateway_service.rs`,
  and do not keep `chapter_candidate_executor_runtime_adapter_service.rs` after
  runtime quality/provider/record bridges collapse into the production adapter,
  default dependency, quality adapter, provider stream, and record owners.
- The `chapter-candidate-route-gateway-smoke-rust` manifest probe is the
  measurable `chapters` candidate gateway business/cutover readiness signal.
  Keep its `business` profile, dedicated
  `phase5-chapters-candidate-gateway-owner` profile, Rust owner path, Python
  fallback path, fallback-freeze candidate path, and
  `python_candidate_executor_fallback` rollback boundary together unless a
  stronger real route smoke replaces it. Do not downgrade it back to
  deploy-only evidence while the active route still depends on this
  gateway/fallback decision. The fallback-freeze candidate must validate
  `rust_executor_enabled = true` and `fallback_on_rust_error = false`, but
  `python_fallback_removal_ready` must stay false until the active route smoke
  consumes that configuration and the Python compatibility shell is explicitly
  frozen, repointed, or removed.
- The `chapter-single-generation-active-gateway-smoke-rust` manifest probe is
  the measurable active-route consumer for the single-generation gateway
  cutover chain. Once the active-route smoke validates both
  `chapter-single-generation-active-gateway-rust-owner` and
  `chapter-single-generation-active-gateway-fallback-freeze-candidate`, do not
  restore `chapter-single-generation-active-gateway-direct-fallback` as
  readiness evidence. Rollback stays available through deployment/AppConfig
  gateway configuration, not through a health/readiness probe that still
  exercises the Python direct fallback path. The fallback-freeze probe must
  validate the active route consuming `rust_executor_enabled = true`,
  `fallback_on_rust_error = false`, and
  `python_fallback_removal_ready = true`.
- Once active-route fallback-freeze smoke exists, do not keep auth-guard-only
  Python fallback manifest probes for the same single-generation stream and
  background routes. `chapters-generate-background-auth-guard-python-fallback`
  and `chapters-generate-stream-auth-guard-python-fallback` are retired
  route-level fallback probes; the Rust `chapters-generate-background-*` and
  `chapters-generate-stream-*` probes plus
  `chapter-single-generation-active-gateway-smoke-rust` are the active cutover
  evidence. The Python compatibility source file may remain as a rollback
  source map, but the retired probes must not be reintroduced unless the Rust
  route owner or active-route freeze smoke is rolled back in the same change.
- Once single-generation active-route fallback-freeze is the production
  readiness boundary, batch resume single-chapter dispatch must pass an
  explicit `ChapterCandidateRouteGatewayConfig` from the route/AppConfig layer
  into
  `SingleGenerationRuntimeLifecyclePlan::from_runtime_launch_with_gateway_config`.
  Do not use the default single-generation direct-fallback config in
  production resume dispatch. The convenience
  `default_single_generation_candidate_gateway_config` and
  `SingleGenerationRuntimeLifecyclePlan::from_runtime_launch` helpers are
  test-only rollback/source-map helpers unless a deliberate rollback changes
  the production route config in the same change.
- Once the single-generation stream route consumes
  `ChapterCandidateRouteGatewayConfig`, the stream lifecycle owner must expose
  only `SingleGenerationStreamLifecyclePlan::from_runtime_launch_with_gateway_config`.
  Do not reintroduce a stream-side default `from_runtime_launch` constructor or
  test helper that reaches `default_single_generation_candidate_gateway_config`;
  stream tests should pass an explicit fallback-disabled gateway config so the
  production route/AppConfig boundary stays visible.
- Once batch active/stream/resume Rust auth-guard probes and the
  `chapter_batch_generation` Rust route owner are validated, do not keep
  auth-guard-only Python fallback manifest probes for that same route function
  group. `chapters-batch-active-tasks-auth-guard-python-fallback`,
  `chapters-batch-stream-auth-guard-python-fallback`, and
  `chapters-batch-resume-auth-guard-python-fallback` are retired route-level
  fallback probes; the Rust `chapters-batch-active-tasks-*`,
  `chapters-batch-stream-*`, and `chapters-batch-resume-*` probes plus focused
  `chapter_batch_generation` tests are the cutover evidence. The Python batch
  route source may remain as a rollback/source map, but these retired probes
  must not be reintroduced unless the Rust batch route owner or matching
  focused tests are rolled back in the same change.
- Once the regeneration tasks Rust auth-guard probe and focused
  `chapter_regeneration` route/query tests are validated, do not keep the
  auth-guard-only Python fallback manifest probe for
  `/chapters/{chapter_id}/regeneration/tasks`.
  `chapters-regeneration-tasks-auth-guard-python-fallback` is a retired
  route-level fallback probe; `chapters-regeneration-tasks-auth-guard-rust`
  plus the Rust `chapter_regeneration_routes` / `chapter_regeneration_query`
  owner tests are the cutover evidence. Python regeneration route source may
  remain as a rollback/source map, but this retired probe must not be
  reintroduced unless the Rust regeneration route/query owner or matching
  focused tests are rolled back in the same change.
- Once the users route group Rust auth-guard probes cover current user, users
  list, set-admin, and reset-password, do not keep auth-guard-only Python
  fallback manifest probes for those same route functions.
  `users-current-auth-guard-python-fallback`,
  `users-list-auth-guard-python-fallback`,
  `users-set-admin-auth-guard-python-fallback`, and
  `users-reset-password-auth-guard-python-fallback` are retired route-group
  fallback probes; `backend-rs/src/api/users.rs` plus the Rust
  `users-current/list/set-admin/reset-password` probes are the readiness
  evidence. `backend/app/api/users.py` may remain as a rollback/source map,
  but these retired probes must not be reintroduced unless the Rust users route
  owner or matching readiness probes are rolled back in the same change.
- Once the book import route group Rust auth-guard probes cover create-task,
  task-status, preview, cancel, apply, retry-stream, and apply-stream, do not
  keep auth-guard-only Python fallback manifest probes for those same route
  functions. `book-import-create-task-auth-guard-python-fallback`,
  `book-import-task-status-auth-guard-python-fallback`,
  `book-import-preview-auth-guard-python-fallback`,
  `book-import-cancel-auth-guard-python-fallback`,
  `book-import-apply-auth-guard-python-fallback`,
  `book-import-retry-stream-auth-guard-python-fallback`, and
  `book-import-apply-stream-auth-guard-python-fallback` are retired
  route-group fallback probes; `backend-rs/src/api/book_import.rs` plus the
  Rust `book-import-*` probes are the readiness evidence. Python book import
  route/service/schema files may remain as rollback/source maps, but these
  retired probes must not be reintroduced unless the Rust book import route
  owner or matching readiness probes are rolled back in the same change.
- Once the outlines route group Rust auth-guard probes cover project-list,
  list, generate-stream, batch-expand-stream, and create-chapters-from-plans,
  do not keep auth-guard-only Python fallback manifest probes for those same
  route functions. `outlines-project-list-auth-guard-python-fallback`,
  `outlines-list-auth-guard-python-fallback`,
  `outlines-generate-stream-auth-guard-python-fallback`,
  `outlines-batch-expand-stream-auth-guard-python-fallback`, and
  `outlines-create-chapters-from-plans-auth-guard-python-fallback` are retired
  route-group fallback probes; `backend-rs/src/api/outlines.rs` plus the Rust
  `outlines-*` probes are the readiness evidence. Python outlines
  route/model/schema/service files may remain as rollback/source maps, but
  these retired probes must not be reintroduced unless the Rust outlines route
  owner or matching readiness probes are rolled back in the same change.
- Once the characters route group Rust auth-guard probes cover project-list,
  list, generate-stream, export, and import, do not keep auth-guard-only Python
  fallback manifest probes for those same regular route functions.
  `characters-project-list-auth-guard-python-fallback`,
  `characters-list-auth-guard-python-fallback`,
  `characters-generate-stream-auth-guard-python-fallback`,
  `characters-export-auth-guard-python-fallback`, and
  `characters-import-auth-guard-python-fallback` are retired regular fallback
  probes; `backend-rs/src/api/characters.rs` plus the Rust `characters-*`
  regular route probes are the readiness evidence. Do not remove or count
  `characters-validate-import-auth-guard-python-fallback` under this regular
  fallback rule: it is asymmetric evidence paired with
  `characters-validate-import-public-rust` for the public validation route
  policy. Python characters route/model/schema/service files may remain as
  rollback/source maps, but the retired regular probes must not be reintroduced
  unless the Rust characters route owner or matching readiness probes are
  rolled back in the same change.
- Once the auth route group Rust probes cover config, logout, LinuxDo URL
  misconfiguration, current user, password status/set/initialize, refresh,
  callback missing-code, and invalid local/bind login, do not keep Python
  fallback manifest probes for those same auth route functions.
  `auth-logout-public-python-fallback`,
  `auth-user-auth-guard-python-fallback`,
  `auth-password-status-auth-guard-python-fallback`,
  `auth-password-set-auth-guard-python-fallback`,
  `auth-password-initialize-auth-guard-python-fallback`,
  `auth-refresh-auth-guard-python-fallback`,
  `auth-callback-missing-code-python-fallback`,
  `auth-local-login-invalid-credentials-python-fallback`, and
  `auth-bind-login-invalid-credentials-python-fallback` are retired auth
  fallback probes; `backend-rs/src/api/auth.rs`,
  `backend-rs/src/middleware/auth.rs`, and the Rust auth/password service
  owners are the readiness evidence. Python auth route, middleware, user, and
  password files may remain as rollback/source maps, but the retired auth
  probes must not be reintroduced unless the Rust auth route/middleware owner
  or matching readiness probes are rolled back in the same change.
- Once the Rust public `characters-validate-import-public-rust` probe is the
  accepted route policy for `/api/characters/validate-import`, do not restore
  `characters-validate-import-auth-guard-python-fallback` unless that public
  route policy or the Rust characters route owner is rolled back in the same
  change. The Python characters files remain source maps / rollback
  references; they are not active fallback readiness owners.
- Once `backend-rs/src/api/router.rs` serves static assets and the SPA fallback
  through `rust-spa-root`, do not restore `python-fallback-root` as a deploy,
  route-groups, or business readiness owner unless the Rust static/SPA
  fallback route is rolled back in the same change. The root path `/` is
  deployment surface, not a business route group; it should not be counted as
  route-group migration backlog after Rust owns the SPA root probe.
- Once the analysis view and batch-analysis-status Rust auth-guard probes plus
  focused `chapter_analysis` tests are validated, do not keep auth-guard-only
  Python fallback manifest probes for those same analysis/query route function
  groups. `chapters-analysis-auth-guard-python-fallback` and
  `chapters-batch-analysis-status-auth-guard-python-fallback` are retired
  route-level fallback probes; `chapters-analysis-auth-guard-rust`,
  `chapters-batch-analysis-status-auth-guard-rust`, and focused
  `chapter_analysis` tests are the cutover evidence. Python analysis route
  source may remain as a rollback/source map, but these retired probes must
  not be reintroduced unless the Rust analysis route/query owners or matching
  focused tests are rolled back in the same change.

Default validation target:

```powershell
cargo test <owner_filter> --manifest-path "backend-rs/Cargo.toml" --target-dir "E:/Code/ProjectsCode/WorkSpace/Codex/NovelAi/MuMuNovel/.codex-targets/<package-slug>" -- --nocapture
cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "E:/Code/ProjectsCode/WorkSpace/Codex/NovelAi/MuMuNovel/.codex-targets/<package-slug>"
```

## Scenario: Rust generated narrative cleaner owner

### 1. Scope / Trigger
- Trigger: a migration package ports generated-text cleanup helpers from
  `backend/app/services/chapter_generated_text_service.py` or wires Rust
  generation/regeneration/draft owners to generated narrative cleanup.
- Why this needs code-spec depth: generated text is persisted as chapter
  content and later feeds quality analysis, history, SSE result payloads, and
  draft/regeneration views. Letting workflow/meta text leak into Rust
  persistence creates user-visible content pollution even when route payloads
  stay unchanged.

### 2. Signatures
- Rust owner file:
  `backend-rs/src/services/chapter_narrative_cleaner_service.rs`
- Expected owner functions:
  `sanitize_generated_narrative_text(text) -> (String, usize)`.
- Expected owner functions:
  `contains_chapter_workflow_meta_text(text) -> bool`.
- Expected owner functions:
  `trim_text_to_sentence_boundary(text, hard_limit) -> String`.
- Expected production consumer:
  `ChapterGenerationRuntimeContext::build_generated_result(response)
  -> Result<GeneratedChapterResult, String>` in
  `backend-rs/src/services/chapter_generation_runtime_service.rs`.

### 3. Contracts
- Generated narrative cleanup must remove meta-only or workflow lines such as
  code fences, `以下是章节正文：`, process step/log/review text, and AI assistant
  self-descriptions.
- Cleaned generated text must be trimmed and must preserve normal narrative
  paragraphs and blank-line collapsing behavior.
- Single-chapter Rust runtime must sanitize the AI response before creating
  `GeneratedChapterResult`; it must not persist raw AI text with only `.trim()`.
- Empty text after sanitization must become a runtime error before persistence.
- Sentence-boundary trimming must use character-count semantics, not byte
  counts, and should prefer recent sentence punctuation before falling back to
  appending `。`.
- Staged pure helper ports are allowed only when the same Rust owner already
  has a production consumer; an unconsumed trim helper alone is not migration
  completion.

### 4. Validation & Error Matrix
- Normal narrative text with surrounding whitespace -> cleaned result keeps the
  narrative and updates `word_count` from the cleaned text.
- Meta prefix plus narrative body -> prefix is removed, narrative body remains.
- Meta-only text -> runtime returns
  `chapter generation produced empty narrative after sanitization`.
- Cleaned text still containing workflow/meta text -> runtime returns
  `chapter generation produced workflow/meta text`.
- `hard_limit = 0` or text shorter than the limit -> return trimmed original
  text.
- No sentence boundary near the hard limit -> trim to the hard limit, remove
  trailing comma/list punctuation, and append `。` if needed.

### 5. Good/Base/Bad Cases
- Good: Rust generation runtime, draft apply, and regeneration apply all use
  the Rust narrative cleaner before persistence or content application, with
  focused tests for normal, meta-prefixed, and meta-only output.
- Base: one Rust production path consumes the cleaner while the remaining
  Python candidate output path stays as source map for the next package.
- Bad: add Rust trim/sanitize helpers but keep every production generation path
  persisting raw AI output.
- Bad: treat byte length as hard-limit length for Chinese text.
- Bad: change HTTP/SSE payload shape while trying to migrate text cleanup.

### 6. Tests Required
- Unit tests in `chapter_narrative_cleaner_service.rs` for meta-line removal,
  blank/meta-only output, light template polishing, Unicode hard-limit
  trimming, boundary selection, and fallback punctuation.
- Unit tests in `chapter_generation_runtime_service.rs` proving normal AI
  output still builds a `GeneratedChapterResult`, meta prefixes are removed,
  and meta-only output returns the runtime error before persistence.
- Focused stream/background tests when the generation runtime consumer changes,
  because `GeneratedChapterResult` feeds single-stream success projection and
  background lifecycle payloads.
- `cargo check --manifest-path "backend-rs/Cargo.toml"` with an explicit
  target dir when build artifacts must stay out of the workspace.

### 7. Wrong vs Correct
#### Wrong
```rust
fn build_generated_result(&self, response: AIResponse) -> GeneratedChapterResult {
    let cleaned_content = response.content.trim().to_string();
    // Raw workflow/meta lines can still be persisted as chapter content.
    ...
}
```

#### Correct
```rust
fn build_generated_result(&self, response: AIResponse) -> Result<GeneratedChapterResult, String> {
    let (cleaned_content, _) = sanitize_generated_narrative_text(&response.content);
    if cleaned_content.trim().is_empty() {
        return Err("chapter generation produced empty narrative after sanitization".to_string());
    }
    ...
}
```

## Scenario: Rust chapter candidate output stream owner

### 1. Scope / Trigger
- Trigger: a migration package ports stream-output collection from
  `backend/app/services/chapter_candidate_output_service.py`, consumes Rust
  sentence-boundary trimming, or reuses generated stream aggregation in a Rust
  generation/regeneration path.
- Why this needs code-spec depth: streamed generated text feeds candidate
  ranking, runtime progress fields, SSE chunk events, draft previews, and
  max-output guardrails. Byte-count truncation, missed runtime-state updates,
  or unconsumed helper ports create silent migration drift.

### 2. Signatures
- Rust owner file:
  `backend-rs/src/services/chapter_candidate_output_service.rs`.
- Expected owner function:
  `collect_generation_candidate_output(request, on_chunk)
  -> Result<ChapterCandidateOutput, String>`.
- Expected lower-level test hook:
  `collect_generation_candidate_output_from_stream(stream, candidate_index,
  max_output_chars, runtime_state, on_chunk)
  -> Result<ChapterCandidateOutput, String>`.
- Expected request struct fields:
  `ChapterCandidateOutputRequest { ai_service, prompt, system_prompt, tools,
  candidate_index, max_output_chars, runtime_state }`.
- Expected output struct fields:
  `ChapterCandidateOutput { full_content: String, chunks: Vec<String>,
  runtime_state: Option<Value> }`.
- Expected progress callback payload:
  `ChapterCandidateOutputProgress { current_chars: usize, chunk_count: usize }`.
- Required production consumer:
  at least one Rust stream path must consume the owner before the helper is
  counted as migration progress. Current consumer:
  `backend-rs/src/services/chapter_regeneration_stream_launch_service.rs`.

### 3. Contracts
- Candidate index must be normalized to at least `1`.
- When `runtime_state` is present, the owner must initialize and update
  `candidate_index`, `candidate_total`, `candidate_count`, `current_chars`,
  `word_count`, and `chunk_count` through
  `chapter_candidate_runtime_state_service.rs`.
- The output owner must return the updated `runtime_state` snapshot with
  `ChapterCandidateOutput` so generation, word-budget repair, targeted repair,
  default dependency, and provider-stream owners can write provider progress
  back to the active stage request instead of relying on Python polling or
  heartbeat side effects.
- Later record/finalize owners may legitimately overwrite final-progress fields
  such as `current_chars` and `chunk_count` from the selected candidate record.
  Tests that need provider-output pass-through evidence should assert a stable
  provider field or a lower-level generation/repair handoff, not the final
  record-synchronized value.
- Stream chunks must be appended in order, preserving the returned chunk list
  unless final max-output truncation replaces the list with the trimmed full
  content.
- `max_output_chars` must use Unicode character-count semantics and must call
  `trim_text_to_sentence_boundary(...)` when final content exceeds the limit.
- Provider stream errors must propagate as `Err(String)` so the consuming
  transport owner can keep its existing error response shape.
- The stream-output owner must not own SSE event payload construction; routes
  and stream workflow owners keep transport-specific payloads.
- Production callers with an `AIService` should go through
  `collect_generation_candidate_output(...)`; the lower-level
  `collect_generation_candidate_output_from_stream(...)` is for stream reuse,
  tests, and adapters that already own a stream.

### 4. Validation & Error Matrix
- Empty stream -> `full_content = ""`, `chunks = []`, no error.
- Multi-chunk stream -> ordered `full_content` concatenation and ordered
  `chunks`.
- `candidate_index <= 0` -> runtime-state `candidate_index = 1`.
- Existing `candidate_total > candidate_index` -> preserve existing total.
- Non-object runtime-state -> replace with object through the runtime-state
  owner before syncing fields.
- Content longer than `max_output_chars` -> sentence-boundary trimmed output,
  with `chunks = [trimmed_content]`.
- Stream item `Err(error)` -> return the same error string.

### 5. Good/Base/Bad Cases
- Good: a Rust generation or regeneration stream consumes
  `collect_generation_candidate_output(...)`, focused tests cover
  runtime-state sync and Unicode trimming, and the transport owner still owns
  SSE payloads.
- Base: the owner is introduced with one real Rust production consumer while
  Python candidate executor remains the source map for the next package.
- Bad: add a concrete Rust candidate-output facade around `AIService` but never
  call it from a Rust production path.
- Bad: production callers bypass the concrete request owner and hand-build
  provider stream loops in route or workflow files.
- Bad: compute `current_chars` with `String::len()` for Chinese text.
- Bad: move SSE event construction into the candidate-output owner and blur
  transport vs behavior ownership.

### 6. Tests Required
- Unit tests in `chapter_candidate_output_service.rs` for chunk collection,
  callback progress, runtime-state sync, non-object runtime-state replacement,
  stream error propagation, empty stream, and Unicode sentence-boundary
  truncation.
- Focused generation, word-budget repair, targeted repair, provider stream, and
  default dependency tests should cover `runtime_state` pass-through from
  provider output back into the active request. Executor-level tests should also
  cover the final record/finalize sync overwriting final-progress fields.
- Unit tests in `chapter_narrative_cleaner_service.rs` for the trimming helper
  that stream-output uses.
- Focused tests for the consuming stream owner, for example
  `chapter_regeneration_stream_launch_service`, to ensure existing transport
  behavior still compiles and remains owned by the stream layer.
- `cargo check --manifest-path "backend-rs/Cargo.toml"` with an explicit
  target dir when build artifacts must stay outside the workspace.

### 7. Wrong vs Correct
#### Wrong
```rust
let current_chars = full_content.len();
if current_chars >= limit {
    full_content.truncate(limit);
}
```

#### Correct
```rust
let current_chars = full_content.chars().count();
if full_content.chars().count() > limit {
    full_content = trim_text_to_sentence_boundary(&full_content, limit);
}
```

## Scenario: Rust chapter candidate generation workflow owner

### 1. Scope / Trigger
- Trigger: a migration package ports the candidate-pool workflow from
  `backend/app/services/chapter_candidate_generation_service.py` or wires a
  Rust candidate executor to candidate output, candidate record, retry, and
  best-candidate selection owners.
- Why this needs code-spec depth: candidate generation composes AI output
  collection, runtime-state progress, retry prompt/strategy construction,
  retry temperature, record building, and final winner selection. Treating one
  helper as the whole migration hides whether the production candidate executor
  has actually moved to Rust.

### 2. Signatures
- Rust owner file:
  `backend-rs/src/services/chapter_candidate_generation_service.rs`.
- Expected owner function:
  `generate_candidate_pool_workflow(request, dependencies)
  -> Result<ChapterCandidateGenerationResult, String>`.
- Expected request struct fields:
  `base_generate_kwargs`, `base_prompt`, `base_temperature`,
  `target_word_count`, `source`, `generation_label`, `max_candidates`, and
  `runtime_state`.
- Expected dependency callbacks:
  collect output, build candidate record, decide whether another candidate is
  needed, build retry prompt suffix, build retry strategy suffix, resolve retry
  temperature, and select the best candidate.

### 3. Contracts
- `max_candidates` must normalize to at least `1`.
- The workflow must sync candidate runtime-state before the first attempt and
  before each produced candidate, using the Rust candidate runtime-state owner.
- Retry candidates must preserve Python prompt behavior:
  `base_prompt + "\n\n" + retry_suffix`, trimmed after joining.
- Retry suffix must join non-empty prompt and strategy suffix parts with a
  blank line; if the suffix is empty, generation stops.
- Retry temperature must override `temperature` only when a finite JSON number
  can be represented.
- Final selection must use the supplied best-candidate selector, falling back
  to the last produced candidate if the selector returns no winner.
- A staged owner with `#![allow(dead_code)]` must be documented as not yet
  production cutover until a Rust candidate executor consumes it.

### 4. Validation & Error Matrix
- `max_candidates <= 0` -> one candidate is produced and runtime-state total is
  `1`.
- First candidate only -> prompt and base temperature remain unchanged.
- Retry candidate -> prompt contains the retry prompt and strategy suffixes,
  runtime-state shows `rerank_retry / rerank_candidate`, and `rerank_used` is
  true.
- Empty retry suffix after the first candidate -> stop early and select the
  produced candidate.
- Output collection error -> propagate `Err(String)` without selecting a
  partial result.

### 5. Good/Base/Bad Cases
- Good: Rust candidate executor consumes `generate_candidate_pool_workflow(...)`
  together with the Rust candidate output owner, and focused tests cover retry
  prompt, retry temperature, runtime-state, early-stop, and selection fallback.
- Base: the workflow owner is staged and tested as a whole function group, with
  docs explicitly saying Python active path still owns production candidate
  execution.
- Bad: count the staged workflow as production migration while the Python
  candidate executor still calls `chapter_candidate_generation_service.py`.
- Bad: split retry prompt, runtime-state sync, and best-candidate fallback into
  unrelated micro seams without a candidate executor cutover plan.

### 6. Tests Required
- Unit tests in `chapter_candidate_generation_service.rs` for initial
  candidate generation, retry candidate prompt/temperature, empty retry suffix
  early-stop, max-candidate normalization, and selection fallback.
- Existing `chapter_candidate_output_service.rs` tests must remain green when
  the workflow composes output collection.
- `cargo check --manifest-path "backend-rs/Cargo.toml"` with an explicit
  target dir when build artifacts must stay outside the workspace.
- When the Rust candidate executor consumes this owner, add focused tests for
  the executor/wiring path, not just this staged workflow owner.

### 7. Wrong vs Correct
#### Wrong
- Add an unconsumed Rust workflow owner and report that
  `chapter_candidate_generation_service.py` is migrated out of production.

#### Correct
- Report the staged owner as a whole-function-group port, then make the next
  package the Rust candidate executor cutover that consumes this owner.

## Scenario: Rust chapter candidate record owner

### 1. Scope / Trigger
- Trigger: a migration package ports candidate record construction from
  `backend/app/services/chapter_candidate_record_service.py` or wires Rust
  candidate generation to a Rust-owned candidate record builder.
- Why this needs code-spec depth: candidate records carry sanitized generated
  content, quality metrics, quality-gate decisions, selection metadata,
  candidate chunks, and retry/finalization inputs. A partial port can make the
  Rust workflow appear complete while still depending on Python record-shape
  semantics.

### 2. Signatures
- Rust owner file:
  `backend-rs/src/services/chapter_candidate_record_service.rs`.
- Expected owner function:
  `build_generation_candidate_record(request, quality_evaluator,
  quality_gate_plan_builder, log_warning)
  -> Result<Value, String>`.
- Expected request struct fields:
  `full_content`, `candidate_chunks`, `target_word_count`, `source`,
  `generation_label`, `candidate_index`, `candidate_offset`,
  `generation_path`, and `attempt_kind`.
- Required composition point:
  `chapter_candidate_generation_service.rs` should be able to use the record
  owner as its `build_generation_candidate_record_fn`.

### 3. Contracts
- Generated text must be sanitized through
  `chapter_narrative_cleaner_service.rs` before quality evaluation.
- Removed workflow/meta lines should emit the optional warning callback with
  the generation label and candidate index.
- Empty sanitized content returns
  `<generation_label> generated empty narrative after sanitization`.
- Sanitized content that still contains workflow/meta text returns
  `<generation_label> generated workflow/meta text`.
- `word_count` uses Unicode character-count semantics.
- Quality-gate builder must run twice:
  first on raw quality metrics, then again after candidate selection metadata
  is attached.
- If the second quality-gate plan is empty or not an object, fall back to the
  initial normalized plan.
- Quality-gate normalization must preserve Python word-budget pressure behavior:
  an `allow_save` gate may become `auto_repair` when content is far outside the
  target window.
- The final record must attach `candidate_selection` into
  `quality_metrics` and also flatten the same selection metadata fields onto
  the candidate record.

### 4. Validation & Error Matrix
- Normal narrative content -> returns a candidate record with sanitized
  `full_content`, `summary_preview`, `candidate_chunks`, quality metrics,
  normalized quality-gate plan, and flattened selection metadata.
- Initial quality-gate builder output without selection metadata -> second
  builder call receives `quality_metrics.candidate_selection`.
- Enriched quality-gate builder returns empty object -> final plan falls back
  to the initial normalized plan.
- Meta-only generated content -> returns the empty-sanitized error and logs
  removed meta lines when a warning callback is provided.
- Short generated content with large target window and `allow_save` gate ->
  selection metadata may report `quality_gate_decision = auto_repair`.

### 5. Good/Base/Bad Cases
- Good: candidate generation composes the Rust record owner, focused tests
  prove enriched selection metadata and quality-gate fallback behavior, and
  executor cutover can reuse the same record boundary.
- Base: the record owner is staged and tested while Python active-path
  candidate execution still owns production record construction.
- Bad: duplicate record-shape assembly inside the generation workflow instead
  of delegating to the record owner.
- Bad: port only text sanitization and leave quality-gate normalization or
  candidate-selection metadata in Python.
- Bad: treat staged record owner registration as proof that
  `chapter_candidate_record_service.py` has left the active path.

### 6. Tests Required
- Unit tests in `chapter_candidate_record_service.rs` for enriched selection
  metadata, empty enriched quality-gate fallback, meta-only sanitization error,
  warning callback behavior, and word-budget quality-gate normalization.
- Unit tests in `chapter_candidate_generation_service.rs` proving the
  generation workflow can compose the Rust record owner as its record-builder
  dependency.
- `cargo check --manifest-path "backend-rs/Cargo.toml"` with an explicit
  target dir when build artifacts must stay outside the workspace.
- When the Rust candidate executor consumes the record owner, add focused
  executor/wiring tests that assert the production candidate path uses the Rust
  record boundary.

### 7. Wrong vs Correct
#### Wrong
- Keep local JSON record assembly inside generation/executor tests and count
  the record file as migrated because a sanitizer helper exists.

#### Correct
- Put sanitized content, quality-gate normalization, candidate-selection
  metadata, and final candidate record shape in one Rust owner, then compose it
  from generation/executor owners.

## Scenario: Rust chapter candidate finalize owner

### 1. Scope / Trigger
- Trigger: a migration package ports final-candidate resolution from
  `backend/app/services/chapter_candidate_finalize_service.py` or wires a Rust
  candidate executor to the final selection, word-budget repair promotion, and
  final runtime-state sync path.
- Why this needs code-spec depth: finalization decides which candidate is
  saved, how quality-gate and candidate-selection metadata are attached, and
  which candidate progress fields reach runtime checkpoints. A staged helper
  without executor cutover must not be reported as production retirement.

### 2. Signatures
- Rust owner file:
  `backend-rs/src/services/chapter_candidate_finalize_service.rs`.
- Expected owner functions:
  `resolve_final_candidate_state(request, selected_candidate, candidates,
  quality_gate_plan_builder, dependencies) -> ChapterCandidateFinalizeState`.
- Expected owner functions:
  `maybe_promote_best_word_budget_repair_candidate(request, state,
  quality_gate_plan_builder, dependencies) -> ChapterCandidateFinalizeState`.
- Expected owner functions:
  `finalize_selected_candidate_result(request, state)
  -> (Value, ChapterCandidateRuntimeFinalizeSyncInput)`.
- Expected direct Rust dependency:
  final runtime-state synchronization must go through
  `chapter_candidate_runtime_state_service.rs`.

### 3. Contracts
- Final attempt labels must resolve through the Rust candidate runtime-state
  owner when the candidate record does not already carry explicit labels.
- Final generation path must preserve Python behavior:
  `single_pass`, `rerank_retry`, or `word_budget_repair`.
- Quality-gate plan must be rebuilt for final metrics, normalized, copied back
  into `quality_metrics.quality_gate`, and then enriched with final
  candidate-selection metadata.
- Candidate-selection metadata must be attached both into
  `quality_metrics.candidate_selection` and onto the selected candidate's
  flattened metadata fields.
- When the selected candidate is not saveable, the owner may promote the best
  word-budget repair candidate if the injected preference callback chooses it.
- Final runtime-state sync must update candidate index, total, character
  count, word count, chunk count, generation path, attempt kind, rerank flag,
  word-budget repair flag, and winner candidate index.
- Rerank-heavy formulas remain injectable until the Rust rerank/executor
  package cuts over; document staged status clearly while those callbacks are
  still supplied from the future executor boundary.

### 4. Validation & Error Matrix
- Initial selected candidate with missing labels -> final labels resolve to
  `single_pass / initial_candidate`.
- Rerank selected candidate -> final labels and metadata show
  `rerank_retry / rerank_candidate` and `rerank_used = true`.
- Word-budget repair selected candidate -> final labels and metadata show
  `word_budget_repair / word_budget_repair` and
  `word_budget_repair_used = true`.
- Final gate decision is `allow_save` -> do not promote a repair candidate.
- Final gate decision is not `allow_save` and a preferred repair candidate
  exists -> promote it and rebuild final metadata/gate state.
- Finalization result must preserve candidate list and selected-candidate JSON
  shape while returning a sync input for the runtime-state owner.

### 5. Good/Base/Bad Cases
- Good: Rust candidate executor composes generation, record, finalize,
  runtime-state, and output owners, with focused tests for final winner
  metadata and runtime sync.
- Base: finalize owner is staged and tested as a whole finalization function
  group while Python active-path candidate execution still owns production
  executor wiring.
- Bad: count `chapter_candidate_finalize_service.py` as production-migrated
  only because a Rust finalize module exists.
- Bad: duplicate final quality-gate or candidate-selection metadata assembly
  inside executor tests instead of delegating to the finalize owner.
- Bad: bypass the Rust runtime-state owner and patch final candidate progress
  fields by hand.

### 6. Tests Required
- Unit tests in `chapter_candidate_finalize_service.rs` for final metadata
  resolution, final runtime-state sync input, word-budget repair promotion, and
  fallback label/path behavior.
- Existing candidate generation and record owner tests must remain green
  because finalize composes their candidate shape.
- `cargo check --manifest-path "backend-rs/Cargo.toml"` with an explicit
  target dir when build artifacts must stay outside the workspace.
- When the Rust candidate executor consumes this owner, add focused
  executor/wiring tests proving production candidate execution uses the Rust
  finalization boundary.

### 7. Wrong vs Correct
#### Wrong
- Add a Rust finalize module with local JSON assertions and report the Python
  candidate executor path as retired.

#### Correct
- Keep final selection metadata, word-budget repair promotion, final
  quality-gate normalization, and runtime-state sync in one Rust owner; report
  it as staged until the Rust candidate executor consumes it.

## Scenario: Rust chapter candidate word-budget repair owner

### 1. Scope / Trigger
- Trigger: a migration package ports word-budget repair orchestration from
  `backend/app/services/chapter_candidate_word_budget_repair_service.py` or
  wires a Rust candidate executor to a Rust-owned repair pass.
- Why this needs code-spec depth: the repair pass rebuilds prompts, changes
  generation limits, syncs candidate runtime state, creates an extra candidate,
  attaches repair-seed metadata, and may replace the selected winner. A partial
  port can silently drift from Python candidate executor behavior.

### 2. Signatures
- Rust owner file:
  `backend-rs/src/services/chapter_candidate_word_budget_repair_service.rs`.
- Expected owner function:
  `maybe_apply_word_budget_repair_workflow(request, selected_candidate,
  candidates, dependencies) -> ChapterCandidateWordBudgetRepairResult`.
- Expected output-collection input:
  `ChapterCandidateWordBudgetRepairOutputCollectInput { generate_kwargs,
  candidate_index, max_output_chars }`.
- Expected record-build input:
  `ChapterCandidateWordBudgetRepairRecordBuildInput { full_content,
  candidate_chunks, target_word_count, source, generation_label,
  candidate_index, candidate_offset, generation_path, attempt_kind }`.
- Required direct Rust dependency:
  runtime-state attempt labels and sync must go through
  `chapter_candidate_runtime_state_service.rs`.

### 3. Contracts
- If `should_apply_word_budget_repair_fn` returns false, the owner returns the
  original selected candidate and candidate list with
  `word_budget_repair_used = false`.
- Repair prompt must join base prompt, repair suffix, and the previous draft
  block with blank lines, preserving the Python `Previous draft to rewrite`
  wrapper.
- Repair generation kwargs must override `prompt`, `temperature`, and
  `max_tokens` using injected formula callbacks.
- Repair attempt labels must resolve to
  `word_budget_repair / word_budget_repair`.
- Runtime-state sync must initialize the repair candidate with zero chars and
  chunks, `rerank_used = false`, and `word_budget_repair_used = true`.
- Repair output collection must receive the resolved max-output char limit.
- Repair candidate records must be built through the record owner callback and
  then enriched with repair seed candidate metadata.
- Repair failures are warning-style fallback in Python; Rust staged owner must
  preserve that behavior by returning the original selected candidate and not
  marking repair as used when the repair pass fails.

### 4. Validation & Error Matrix
- Repair not needed -> no output collection and no candidate mutation.
- Empty repair suffix -> original selected candidate remains unchanged.
- Output collection error -> original selected candidate remains unchanged.
- Kept repair candidate -> append it to candidates and set
  `word_budget_repair_used = true`.
- Preferred repair candidate -> selected candidate becomes the repair
  candidate.
- Non-preferred but reranked winner exists -> selected candidate becomes the
  reranked winner.
- Repair seed metadata must include positive `repair_seed_candidate_index` and
  optional seed generation path / attempt kind when present.

### 5. Good/Base/Bad Cases
- Good: Rust candidate executor consumes this owner together with generation,
  record, finalize, output, and runtime-state owners.
- Base: the owner is staged and tested as a whole repair workflow while Python
  active-path candidate execution still owns production executor wiring.
- Bad: count `chapter_candidate_word_budget_repair_service.py` as retired
  before the Rust candidate executor consumes this owner.
- Bad: duplicate prompt construction, runtime-state sync, or repair-seed
  metadata inside executor tests instead of delegating to the repair owner.

### 6. Tests Required
- Unit tests in `chapter_candidate_word_budget_repair_service.rs` for skip,
  prompt/kwargs construction, runtime-state sync, repair metadata, selected
  candidate replacement, and repair-failure fallback.
- Existing generation, record, finalize, output, and runtime-state owner tests
  must remain green when this owner is registered.
- `cargo check --manifest-path "backend-rs/Cargo.toml"` with an explicit
  target dir when build artifacts must stay outside the workspace.
- When the Rust candidate executor consumes this owner, add focused
  executor/wiring tests proving production candidate execution uses the Rust
  repair boundary.

### 7. Wrong vs Correct
#### Wrong
- Port only repair prompt construction and keep runtime sync, record build,
  and selected-candidate replacement in Python.

#### Correct
- Keep repair prompt construction, generation kwargs, runtime-state sync,
  output collection input, record-build input, repair-seed metadata, and winner
  replacement in one Rust owner; report it as staged until executor cutover.

## Scenario: Rust chapter candidate targeted final repair owner

### 1. Scope / Trigger
- Trigger: a migration package ports targeted final repair orchestration from
  `backend/app/services/chapter_candidate_targeted_final_repair_service.py` or
  wires a Rust candidate executor to a Rust-owned targeted repair pass.
- Why this needs code-spec depth: targeted final repair can run before and
  after finalize, can adopt a new winner, or can defer a follow-up repair seed.
  A partial port can make executor cutover look ready while still depending on
  Python repair orchestration.

### 2. Signatures
- Rust owner file:
  `backend-rs/src/services/chapter_candidate_targeted_final_repair_service.rs`.
- Expected owner function:
  `execute_targeted_final_repair_pass_workflow(request, selected_candidate,
  candidates, dependencies) -> ChapterCandidateTargetedFinalRepairResult`.
- Expected output-collection input:
  `ChapterCandidateTargetedFinalRepairOutputCollectInput { generate_kwargs,
  candidate_index, max_output_chars }`.
- Expected record-build input:
  `ChapterCandidateTargetedFinalRepairRecordBuildInput { full_content,
  candidate_chunks, target_word_count, source, generation_label,
  candidate_index, candidate_offset, generation_path, attempt_kind }`.
- Required direct Rust dependency:
  runtime-state sync must go through
  `chapter_candidate_runtime_state_service.rs`.

### 3. Contracts
- Targeted repair generation path and attempt kind must be
  `targeted_quality_repair`.
- Repair prompt must join base prompt, targeted repair suffix, and the previous
  draft block with blank lines, preserving the Python
  `Previous draft to rewrite` wrapper.
- Repair generation kwargs must override `prompt`, `temperature`, and
  `max_tokens` using injected formula callbacks.
- Runtime-state sync must initialize the repair candidate with zero chars and
  chunks, `rerank_used = false`, and `word_budget_repair_used = false`.
- Repair output collection must receive the resolved max-output char limit.
- Repair candidate records must be built through the record owner callback and
  then enriched with repair seed candidate metadata.
- If keep/adopt/prefer callbacks all allow the candidate, the selected winner
  becomes the targeted repair candidate.
- If the candidate is kept but not adopted, follow-up seed deferral is allowed
  only when `allow_followup_seed_defer` is true and the follow-up callback
  accepts the repair candidate.
- Repair failures must preserve Python warning-style fallback by returning the
  original selected candidate and candidate list.

### 4. Validation & Error Matrix
- Empty suffix -> original selected candidate and candidates remain unchanged.
- Output collection error -> original selected candidate and candidates remain
  unchanged.
- Kept/adopted/preferred repair candidate -> append it and select it as winner.
- Kept but not adopted repair candidate with follow-up allowed -> append it and
  return it as `deferred_followup_targeted_repair_seed_candidate`.
- Kept but not adopted repair candidate without follow-up allowed -> append it
  but keep the original winner.
- Repair seed metadata must include positive `repair_seed_candidate_index` and
  optional seed generation path / attempt kind when present.

### 5. Good/Base/Bad Cases
- Good: Rust candidate executor consumes targeted final repair together with
  output, generation, record, word-budget repair, finalize, and runtime-state
  owners.
- Base: the owner is staged and tested as a whole targeted repair workflow
  while Python active-path candidate execution still owns production executor
  wiring.
- Bad: count `chapter_candidate_targeted_final_repair_service.py` as retired
  before the Rust candidate executor consumes this owner.
- Bad: duplicate prompt construction, runtime-state sync, seed metadata, adopt
  checks, or follow-up deferral inside executor tests instead of delegating to
  this owner.

### 6. Tests Required
- Unit tests in `chapter_candidate_targeted_final_repair_service.rs` for
  adopt, follow-up deferral, repair-failure fallback, prompt/kwargs
  construction, runtime-state sync, and repair seed metadata.
- Existing generation, record, finalize, word-budget repair, output, and
  runtime-state owner tests must remain green when this owner is registered.
- `cargo check --manifest-path "backend-rs/Cargo.toml"` with an explicit
  target dir when build artifacts must stay outside the workspace.
- When the Rust candidate executor consumes this owner, add focused
  executor/wiring tests proving production candidate execution uses the Rust
  targeted repair boundary.

### 7. Wrong vs Correct
#### Wrong
- Port only targeted repair suffix handling and keep adopt/defer/runtime sync
  in Python executor code.

#### Correct
- Keep targeted repair prompt construction, generation kwargs, runtime-state
  sync, output collection input, record-build input, repair-seed metadata,
  adopt/prefer checks, and follow-up deferral in one Rust owner; report it as
  staged until executor cutover.

## Scenario: Rust chapter candidate runtime-state owner

### 1. Scope / Trigger
- Trigger: a migration package ports pure Python candidate runtime-state
  helpers from `backend/app/services/chapter_candidate_runtime_state_service.py`
  or adjacent candidate-generation wrappers into `backend-rs`.
- Why this needs code-spec depth: candidate runtime fields flow through stream
  progress, checkpoint payloads, quality-gate metadata, and batch/single
  status views. A silent field-shape drift can break API compatibility even
  when no HTTP route changes.

### 2. Signatures
- Rust owner file:
  `backend-rs/src/services/chapter_candidate_runtime_state_service.rs`
- Expected owner functions:
  `resolve_generation_attempt_labels(candidate_index, is_word_budget_repair)`
  returns `generation_path` plus `attempt_kind`.
- Expected owner functions:
  `build_chapter_candidate_runtime_state(max_candidates)` returns JSON
  runtime-state defaults.
- Expected owner functions:
  `snapshot_chapter_candidate_runtime_state(runtime_state, default_total)`
  returns a normalized snapshot struct.
- Expected owner functions:
  `sync_chapter_candidate_runtime_state(runtime_state, candidate_index,
  candidate_total, patch)` mutates an optional runtime-state JSON object.
- Expected owner functions:
  `insert_python_query_snapshot_candidate_runtime_fields(checkpoint)` preserves
  Python-query checkpoint diagnostic fields for Rust task payload projection.

### 3. Contracts
- Attempt labels must remain:
  `1 -> single_pass / initial_candidate`, `>1 -> rerank_retry /
  rerank_candidate`, word-budget repair -> `word_budget_repair /
  word_budget_repair`.
- Runtime-state defaults must include:
  `candidate_total`, `candidate_count`, `candidate_index`, `current_chars`,
  `word_count`, `chunk_count`, `generation_path`, `attempt_kind`,
  `rerank_used`, `word_budget_repair_used`, `winner_candidate_index`.
- Snapshot normalization must clamp candidate indexes/counts to positive
  values, clamp char/chunk counts to non-negative values, trim string fields,
  and preserve Python-style truthiness where the Python snapshot API did so.
- Python-query checkpoint projection must preserve the existing payload shape:
  missing raw fields become `null`, and boolean diagnostic fields are copied
  only when already JSON bools; non-bool values become `null`.
- A staged Rust owner may keep pure functions before full production
  consumption only if at least one function is consumed by a Rust production
  path and the remaining functions are tested as the next migration entrypoint.

### 4. Validation & Error Matrix
- Missing candidate raw field -> checkpoint contains `null`.
- Non-bool `rerank_used` or `word_budget_repair_used` in checkpoint projection
  -> checkpoint contains `null`.
- `candidate_index < 1` in snapshot/sync -> normalized to `1`.
- `candidate_total < candidate_index` -> normalized to `candidate_index`.
- Blank `generation_path` or `attempt_kind` -> default fallback string.
- `runtime_state = None` during sync -> no mutation and no error.

### 5. Good/Base/Bad Cases
- Good: port the whole pure runtime-state helper family into one Rust owner,
  consume the checkpoint insertion helper from payload projection, and add
  focused Rust tests for all candidate fields.
- Base: port one helper group only when it is immediately consumed by an
  existing Rust service and leaves the remaining Python fallback unchanged.
- Bad: add a Rust file with unconsumed helpers and report it as migration
  progress, or normalize checkpoint fields differently from the Python-query
  payload contract.

### 6. Tests Required
- Unit tests for attempt-label resolution.
- Unit tests for default runtime-state construction.
- Unit tests for snapshot normalization, including string trimming and numeric
  clamping.
- Unit tests for sync mutation, including `None` runtime-state behavior.
- Regression tests in the consuming payload owner proving checkpoint field
  shape remains unchanged.
- `cargo check --manifest-path "backend-rs/Cargo.toml"` with an explicit
  target dir when build artifacts must stay out of the workspace.

### 7. Wrong vs Correct
#### Wrong
```rust
// A new Rust helper exists, but no production owner consumes it.
pub(crate) fn resolve_generation_attempt_labels(...) { ... }
```

#### Correct
```rust
use crate::services::chapter_candidate_runtime_state_service::{
    insert_python_query_snapshot_candidate_runtime_fields,
};

fn insert_python_query_snapshot_runtime_fields(checkpoint: &mut Map<String, Value>) {
    insert_python_query_snapshot_candidate_runtime_fields(checkpoint);
    // Non-candidate diagnostic fields remain owned by the batch payload module.
}
```

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
- Single-route workflow-start collapse contract: if the single-chapter
  background and stream lanes already expose public workflow-start owners that
  can consume `SingleChapterGenerationRouteRequest` directly, do not preserve
  route-local `build_single_chapter_generation_request_from_route_payload(...)`
  handoffs or neighboring public-start wrappers that only replay
  `route payload -> request -> workflow start`. Collapse that route-start
  normalization back into the corresponding background/stream owner so the
  route stays transport-only and each workflow-start owner keeps its own
  request compatibility boundary.
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
- Batch task-view file-collapse contract: if the remaining
  `chapter_batch_generation_task_view_query_service` file only owns:
  - active-task-list route query DTO / request validation
  - active-task-list read start
  - active-project read start
  - final active-task-list / active-project payload wrappers
  and its production dependencies already terminate inside
  `chapter_batch_generation_read_context_service`, do not preserve that extra
  file boundary. Collapse the whole file back into the read-context owner,
  move route-facing query DTO/errors with it, repoint route and error-mapper
  imports directly to the surviving read owner, and keep focused regression
  tests for route-query bounds plus active payload wrapper shape beside that
  surviving owner.
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
- Chapter-generation snapshot owner collapse contract: if the shared
  chapter-generation snapshot lane already has one real owner for runtime-state
  merge/replace semantics, quality-field backfill, and final persisted snapshot
  writes, do not preserve a neighboring
  `chapter_generation_snapshot_query_service` file that only reopens
  `load_chapter_generation_snapshot(...)` and
  `load_chapter_generation_snapshot_map(...)` through another module boundary.
  Collapse those read helpers back into the surviving shared snapshot owner so
  batch read/runtime lanes and single prepare/runtime lanes consume one
  `snapshot read -> merge/backfill -> snapshot write` chain directly.
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
- Batch status-stream event facade collapse contract: if the batch
  status-stream owner already owns the poll loop, owned task+snapshot read,
  stream-state materialization, and final SSE send cadence, do not preserve a
  neighboring `chapter_batch_generation_status_stream_event_service` file that
  only reopens connected/task-not-found/timeout payload builders, heartbeat/data
  wrappers, cursor change detection, or `BatchGenerationStreamState`
  `events(...)` / `analysis_started_event(...)` / `terminal_events(...)`
  projection through another module boundary. Collapse that file back into the
  batch status-stream owner, keep focused tests on connected/heartbeat/error
  payloads plus cursor close/continue semantics beside the stream owner, and
  let the status-stream poll lane consume the full SSE event contract directly.
- Batch stream-semantics facade collapse contract: if the batch
  status-stream owner already owns the poll loop, owned task+snapshot read,
  cursor observation, and final SSE emission cadence, do not preserve a
  neighboring `chapter_batch_generation_stream_semantics_service` file that
  only reopens `BatchGenerationStreamState`,
  `BatchGenerationStreamObservationKey`,
  `BatchGenerationStreamTerminalKind`,
  `BatchGenerationResolvedStreamStatus`,
  `from_task_state(...)`,
  `from_task_state_with_quality_context(...)`,
  `observation_key(...)`, quality-gate projection, or resolved
  default/event/terminal helpers through another module boundary. Collapse
  that file back into the batch status-stream owner, keep focused tests on
  stream-state materialization, terminal-kind semantics, quality-gate
  projection, and observation-key change detection beside the stream owner,
  and let the status-stream poll lane consume the full stream-semantics
  contract directly.
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
- Single-stream success-response file-collapse contract: if a standalone
  stream success response module is consumed only by the single-chapter stream
  workflow and active smoke readiness evidence, and it does not own route
  normalization, transport branching, error mapping, feature flags, or a
  rollback knob, collapse it back into the stream workflow owner. Keep focused
  tests for quality metrics events, quality-gate events, result payload shape,
  analysis-started event order, story-runtime contract attachment, and
  complete/result/done emission beside the stream workflow owner.
- Single-stream prepare-owner contract: if a single-chapter stream lane only
  needs the runtime launch input produced by the single-generation prepare
  boundary, it should consume that prepare owner directly instead of routing
  the same launch-input projection back through the neighboring background
  write workflow service.
- Single-stream entry/lifecycle split contract: if a single-chapter stream lane
  already has one route-facing owner for `route payload -> request -> runtime
  launch input` and one lifecycle owner for `runtime launch input -> spawn /
  progress / success / failure SSE`, keep those responsibilities in separate
  files. The route must import the entry owner only; the lifecycle file must
  accept prepared runtime launch input directly and must not reopen route
  payload normalization or request materialization.
- Single-restored-launch direct materialization contract: if the
  single-generation restored-launch owner already owns request validation,
  chapter-target ownership, startup snapshot planning, response payload
  assembly, and runtime launch input, neighboring stream/background workflow
  lanes should consume owner-provided direct runtime/background materialization
  instead of reopening `prepare(...).into_runtime_launch_input()` or
  `prepare_from_target(...).into_background_launch_parts(...)` as local
  handoff chains.
- Single-request facade collapse contract: if the single-generation prepare
  boundary already owns request validation, chapter-target loading, restored
  runtime launch materialization, background launch-parts projection, task
  seed projection, and task-view payload projection, do not preserve a
  neighboring `chapter_single_generation_request_service` file only for route
  request structs, request normalization, compat options, request bounds, or
  prepare error enums. Collapse those request contracts back into the prepare
  owner, keep focused tests on strict route request fields, null/default bool
  semantics, bounds/choice validation, compat option projection, and error
  detail mapping, and let route/stream/background/runtime callers consume the
  prepare-owner request contract directly.
- Single-runtime-seed facade collapse contract: if the single-generation
  prepare boundary already owns request-bound validation, execution-config
  materialization from request runtime-state, restored runtime -> startup
  snapshot / runtime launch assembly, and background launch-parts projection,
  do not preserve a neighboring `chapter_single_generation_runtime_seed_service`
  file that only forwards those same restored-launch products through one more
  module boundary; collapse that file back into the prepare owner and let
  stream/background/resume callers consume the prepare-owner materialization
  functions directly.
- Single-runtime-restore facade collapse contract: if the single-generation
  prepare boundary already owns request validation, chapter-target loading,
  execution-config materialization, restored quality/runtime-state seed
  recovery, startup snapshot planning, and final runtime/background launch
  materialization, do not preserve a neighboring
  `chapter_single_generation_runtime_restore_service` file that only forwards
  `merge_single_generation_runtime_state(...)`,
  `SingleGenerationStartupSnapshotPlan`,
  `RestoredSingleGenerationRuntimeState`,
  `restore_single_generation_runtime_state(...)`, recent-history quality
  fallback, or restored compat-option projection through one more module
  boundary. Collapse that file back into the prepare owner, keep focused tests
  on restored seed-source selection, startup snapshot quality projection, and
  restored launch materialization, and let stream/background/runtime callers
  consume the prepare-owner restore contract directly.
- Single-task-seed facade collapse contract: if the single-generation prepare
  boundary already owns chapter-target metadata, background response payload
  projection, and final background launch-parts materialization, do not
  preserve a neighboring `chapter_single_generation_task_seed_service` file
  that only forwards `SingleGenerationTaskPersistenceSeed` /
  `task seed -> active model` projection through one more module boundary;
  collapse that file back into the prepare owner and let background-write
  callers consume the prepare-owner task-seed materialization directly.
- Single-task-view payload facade collapse contract: if the single-generation
  prepare boundary already owns request validation, chapter target metadata,
  background response payload projection, and background launch-parts
  materialization, do not preserve a neighboring
  `chapter_single_generation_task_view_payload_service` file only for
  active-status constants, estimated-minute projection, runtime payload base,
  or `batch_generation_task::Model -> task-view payload` projection. Collapse
  those payload helpers back into the prepare owner, keep focused tests on the
  payload base/status/minutes/task-state projection contract, and let
  background-write consume the prepare-owner helpers directly.
- Single-task-stage facade collapse contract: if the single-generation runtime
  boundary already owns runtime launch orchestration, task-stage persistence,
  checkpoint projection, and outcome handoff, do not preserve a neighboring
  `chapter_single_generation_task_stage_service` file that only forwards
  `SingleGenerationTaskStage`, task timestamp updates, and active-model
  mutation helpers through one more module boundary; collapse that file back
  into the runtime-state owner and let runtime/outcome callers consume the
  runtime-owner task-stage contract directly.
- Single-runtime-outcome facade collapse contract: if the single-generation
  runtime boundary already owns runtime launch orchestration, task-stage
  mutation, checkpoint projection, and the only remaining neighbor file
  exists to persist generated-result success, failed-generation status,
  quality-blocked/manual-review snapshots, or follow-up analysis labels, do
  not preserve a separate `chapter_single_generation_runtime_outcome_service`
  file for that tail work. Collapse those outcome semantics back into the
  runtime-state owner, keep focused tests on manual-review label resolution,
  disabled-analysis short-circuit behavior, and success/failure outcome
  persistence shape, and let the runtime lifecycle consume the outcome
  contract in the same owner file.
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
- Batch task-model facade collapse contract: if the batch create
  write-workflow lane already owns access-checked workflow preparation,
  startup snapshot planning, runtime launch assembly, response payload
  projection, and final task insert orchestration, do not preserve a
  neighboring `chapter_batch_generation_task_model_service` file that only
  reopens `BatchGenerationTaskPersistenceSeed`,
  `into_active_model(...)`, or a focused test helper for pending-task
  `ActiveModel` materialization through another module boundary. Collapse
  that file back into the batch write-workflow owner, keep persistence-seed
  and `ActiveModel` assembly semantics beside the write owner, and let any
  batch/single tests consume the write-workflow-facing helper directly.
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
- Shared quality runtime-context algorithm owner contract: if batch and
  single-generation lanes need the same lower-level
  `summary/history/state -> runtime quality context` algorithms, do not keep
  parallel copies of bounded history append, summary-history rebuild, summary
  reconstruction, or fallback history-context merge logic inside separate
  batch/single files. Move those truly shared lower-level algorithms into the
  chapter-generation-scoped quality runtime-context owner, let batch/single
  wrappers keep only scope-specific entrypoint and payload-shape concerns, and
  prove any remaining batch-specific ordering/fallback differences with focused
  regression tests.
- Batch quality runtime-context facade collapse contract: if the batch runtime,
  resume, write-workflow, payload, and quality-status lanes already consume a
  coherent batch quality runtime-context contract, do not preserve a
  neighboring `chapter_batch_generation_quality_runtime_context_service` file
  that only reopens batch summary/history rebuild helpers, batch
  snapshot/runtime-state restore helpers, batch payload application helpers,
  or batch current-quality append/preserve helpers through another module
  boundary. Collapse that file back into the shared
  `chapter_generation_quality_runtime_context_service` owner, keep the
  batch-facing API names beside the shared owner, and prove batch-scope
  ordering/state compatibility with focused regression tests on the shared
  quality runtime-context file.
- Shared quality-gate semantics owner contract: if batch and
  single/story-repair lanes need the same lower-level
  `failed chapter or quality context -> manual review | retry label`
  semantics, do not preserve a batch-named quality-status file as the de
  facto shared parser owner and do not add another single-only forwarding
  facade; instead, move that truly shared lower-level quality-gate decision /
  label owner into a chapter-generation-scoped service and let batch terminal
  semantics plus single/story-repair consumers depend on that shared boundary
  directly. Keep only module-specific terminal status or workflow semantics in
  the batch/single owner files, and prove retry-budget / exhausted-auto-repair
  fallback behavior with focused regression tests on the shared owner.
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
- Single-background workflow public-entry collapse contract: if the
  single-generation background write lane already keeps one explicit
  workflow-entry owner for `route payload -> existing payload | prepared launch
  -> persist_and_dispatch`, do not preserve a neighboring public free function
  such as `start_owned_single_generation_background_write_entry(...)` that only
  rebuilds `route payload -> request -> workflow-entry.start(...)` through one
  more module-level hop. Collapse that empty handoff back into the surviving
  workflow-entry owner and let the route call that owner boundary directly.
- Single-existing-background query file-collapse contract: if the
  single-chapter background write lane already owns the branch decision between
  `existing payload` and `prepared launch`, do not keep the full
  `active task query -> recovered read-state -> existing-background payload`
  owner chain inline in the write-workflow file; collapse that query/load/
  projection contract into one dedicated single-generation query owner file.
- Single-existing-background query vs write owner split contract: when the
  dedicated single-generation existing-background query owner has been lifted
  out into its own file, keep the read-side chain there
  (`active task query -> recovery -> snapshot/read-state -> compat payload`)
  and keep the write workflow file focused on only
  `load target -> choose existing payload vs launch -> persist/disptach`.
  Do not drift back to a mixed owner where read-side payload projection tests
  live beside write-lane persistence tests.
- Single-task-view payload owner split contract: when single-generation
  read-side task payload semantics already form one coherent chain
  (`active statuses -> stage-code mapping -> runtime payload base -> task-view
  payload projection`), do not keep that chain inside the prepare owner.
  Lift it into one dedicated single-generation task-view payload owner file,
  keep focused tests on payload shape and minute-estimation there, and let
  prepare/query/write owners consume that read-side owner directly.
- Single-task-view payload re-collapse contract: if the dedicated
  `chapter_single_generation_task_view_payload_service` file no longer owns an
  independent route/query/fallback boundary and its only production consumer is
  the surviving single-generation prepare/query chain, do not preserve that
  extra file only for active-status constants, estimated-minute projection,
  runtime payload base, or `task -> task-view payload` projection. Collapse the
  full payload helper chain back into
  `chapter_single_generation_prepare_service`, keep focused payload shape /
  minute-estimation tests beside the surviving prepare owner, and let the
  dedicated existing-background query owner continue consuming those helpers
  without mixing read-side recovery logic back into the write workflow file.
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
- Single-runtime public-start collapse contract: if the single-generation
  runtime lane already keeps one explicit lifecycle owner for
  `runtime launch input -> prepare -> execute generation -> persist terminal
  outcome`, do not preserve a neighboring public free function such as
  `dispatch_single_chapter_generation_runtime(...)` that only replays
  `runtime launch input -> lifecycle.from_runtime_launch().spawn(...)`
  through one more module-level hop. Collapse that empty handoff back into the
  surviving lifecycle owner and let background-write / resume callers hand
  launch input to that owner directly.
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
- Batch runtime-checkpoint facade collapse contract: if the batch runtime
  lane already owns queued snapshot planning, resume reset planning, runtime
  dispatch, per-step lifecycle progression, and final checkpoint persistence,
  do not preserve a neighboring
  `chapter_batch_generation_runtime_checkpoint_service` file that only
  reopens checkpoint stage enums, pending/runtime checkpoint payload
  projection, progress calculation, or failure-message helpers through
  another module boundary. Collapse that file back into the batch
  runtime-state owner, keep the checkpoint contract tests beside the runtime
  lifecycle owner, and let resume/runtime consumers depend on the runtime
  owner directly.
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
- Single-stream lifecycle public-start collapse contract: if the
  single-generation stream lane already keeps one explicit stream-entry owner
  for `route payload -> request -> runtime launch input` and one explicit
  lifecycle owner for `runtime launch input -> spawn / progress / success /
  failure SSE`, do not preserve a neighboring public free function such as
  `spawn_owned_single_generation_stream_from_runtime_launch(...)` that only
  replays `runtime launch input -> lifecycle.from_runtime_launch().spawn(...)`
  through one more module-level hop. Collapse that empty handoff back into the
  surviving lifecycle owner and let the stream-entry owner call that lifecycle
  boundary directly.
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
- Single-prepare public-entry wrapper collapse contract: once the
  single-generation restored-launch owner already exposes direct public
  entrypoints for runtime launch input or background launch parts, do not keep
  neighboring free functions that only forward one call into that same owner.
  Stream-entry and write-workflow owners should call the surviving prepare
  owner directly.
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
  runtime lane already owns chapter-scoped pending-checkpoint merge semantics
  plus the resulting quality/runtime restore payloads, that startup snapshot
  owner should live on the single-generation restored-runtime owner chain
  instead of remaining in a neighboring single-only snapshot facade or in the
  batch snapshot file as a chapter-only branch.
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
- Batch-snapshot facade collapse contract: if the batch runtime owner already
  owns queued snapshot planning, resume snapshot reset planning, runtime
  checkpoint merge semantics, and all remaining batch snapshot write call
  sites, do not preserve a neighboring
  `chapter_batch_generation_snapshot_service` file that only reopens
  `queued/resume plan -> merge/write persistence` through another module
  boundary. Collapse that file back into the batch runtime-state owner, keep
  focused tests on queued snapshot planning, resume snapshot reset planning,
  and runtime-state merge semantics beside the runtime owner, and let batch
  create/resume/status/cancel callers consume the runtime-owner snapshot
  contract directly.
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
- Single-background existing read-state/quality-status file-collapse contract:
  if the single-chapter background write lane already owns chapter target
  loading, existing-task branch selection, existing-background payload
  assembly, and background launch persistence/dispatch, do not preserve
  neighboring `chapter_single_generation_existing_background_read_state_service`
  or `chapter_single_generation_quality_status_service` files only for active
  task recovery, snapshot loading, chapter-id matching, or
  `snapshot/runtime state -> quality payload fields` projection. Collapse
  those local read-state and quality-status semantics into
  `chapter_single_generation_background_write_entry_service` and keep focused
  tests on the final existing-background payload shape.
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
- `memories` route-owner business smoke should use an empty search query plus
  memory-type / importance filters when the goal is to prove the SQL fallback
  business path without AI settings. A non-empty query enters the
  vector/embedding path and must be treated as a separate settings/vector
  integration smoke with explicit provider/settings setup.
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

## Scenario: Python single-generation fallback existing-task reuse boundary

### 1. Scope / Trigger
- Trigger: a change touches
  `backend/app/services/single_chapter_background_generation_service.py`,
  `backend/app/api/chapter_generation_routes.py`, or the Rust/Python boundary
  for single-generation existing-background task reuse.
- Why this needs code-spec depth: even when the Rust owner already exists, the
  remaining Python fallback shell still participates in real request handling
  and route-group comparison. Payload drift or timestamp drift here can make
  the fallback shell report the wrong task lifecycle semantics.

### 2. Signatures
- `POST /api/chapters/{chapter_id}/generate-background`
- `load_existing_single_chapter_background_task_payload(...) -> Optional[Dict[str, Any]]`
- `recover_stale_single_chapter_background_task_if_needed(task) -> bool`
- `single_chapter_background_task_contains_chapter(task, chapter_id) -> bool`

### 3. Contracts
- Existing-background short-circuit must preserve the current response shell:
  - `task_id`
  - `chapter_id`
  - `status`
  - `message`
  - `estimated_time_minutes`
  - optional `active_story_repair_payload`
- Single-generation active-task reuse must treat both chapter-id payload shapes
  as equivalent:
  - `["chapter-id"]`
  - `[{"id": "chapter-id"}]`
- Task stale-recovery thresholds remain:
  - `pending` older than 3 minutes -> recover to failed
  - `running` older than 15 minutes -> recover to failed
- DB timestamps currently arrive as naive UTC values in the Python fallback
  path, so stale-recovery comparisons must use the same naive UTC basis.
  Do not compare those DB timestamps directly against local wall-clock
  `datetime.now()`.

### 4. Validation & Error Matrix
- Active `pending/running` task + matching chapter id -> reuse existing task
  payload; do not create a second task.
- Active task + object-style `chapter_ids` payload -> reuse existing task
  payload exactly as string-style payload would.
- Fresh `pending` task created from current DB defaults -> must stay active;
  do not auto-recover it just because local time zone differs from DB time
  storage.
- Truly stale `pending/running` task beyond the threshold -> may be recovered
  to failed before reuse evaluation continues.

### 5. Good/Base/Bad Cases
- Good: Python fallback shell reuses the same active task that the Rust owner
  would treat as active, even when `chapter_ids` is object-shaped and DB
  timestamps are naive UTC.
- Base: the shell still exists as fallback, but it is thin and behaviorally
  aligned with the Rust owner contract.
- Bad: Python shell only matches string arrays while Rust already accepts both
  string and object arrays.
- Bad: Python shell compares DB naive UTC timestamps to local wall-clock time
  and immediately marks fresh tasks as stale.

### 6. Tests Required
- Focused service tests for:
  - string-style `chapter_ids` match
  - object-style `chapter_ids` match
  - fresh `pending` task with current naive UTC timestamp is not auto-recovered
- Focused API regression test for:
  - same chapter background generation request reuses the existing active task
    and returns the same `task_id`

### 7. Wrong vs Correct
#### Wrong
- Treat the remaining Python fallback shell as “frozen” and stop updating it
  after Rust owner code changes.
- Match only `["chapter-id"]` and ignore `[{"id": "chapter-id"}]`.
- Use local `datetime.now()` for stale-recovery against DB naive UTC values.

#### Correct
- Keep the remaining Python fallback shell aligned with the active Rust owner
  contract until the shell is fully retired.
- Accept both string-style and object-style `chapter_ids` payload shapes.
- Compare stale-recovery against the same naive UTC basis used by the current
  DB timestamp values.

## Scenario: Rust outline compact requirement / guidance owner boundary

## Scenario: Route-group gateway fallback collapse completion boundary

### 1. Scope / Trigger
- Trigger: a change retires active same-path Python fallback for a Rust-owned
  route group by editing gateway ownership, strangler probes, or route-group
  rollback docs.
- Why this needs code-spec depth: a route group can look “mostly migrated”
  while the deploy manifest still preserves active same-path Python probes.
  Without an explicit completion rule, migration tracking drifts and future
  slices keep counting already-collapsed groups as unfinished fallback work.

### 2. Signatures
- Gateway owner files:
  - `deploy/nginx/mumunovel.conf`
  - `deploy/nginx/mumunovel-docker.conf`
- Probe manifest:
  - `deploy/strangler-gateway-probes.json`
- Validation commands:
  - `python backend/tools/run_strangler_gateway_smoke.py --manifest deploy/strangler-gateway-probes.json --validate-manifest-only --output <tmp-file>`
  - `python -m pytest backend/tests/test_tools/test_run_strangler_gateway_smoke.py -q`
  - `python backend/tools/run_strangler_gateway_smoke.py --route-group <group> --readiness-summary-only --output <tmp-file>`
  - `python backend/tools/run_strangler_gateway_smoke.py --profile phase5-p1-fallback --route-group <group> --output <tmp-file>`

### 3. Contracts
- After a route-group fallback collapse completes:
  - gateway comments must declare Rust as the active API owner
  - manifest must keep only the active Rust owner probes for that group
  - same-path Python fallback probes for that group must be removed from active
    `phase5-p0-fallback` or `phase5-p1-fallback` coverage
- Rollback contract changes from “always-on same-path fallback exists” to
  “Python execution requires an explicit gateway rollback step”.
- Architecture/runbook/checkpoint docs must switch the group wording from
  active fallback evidence to explicit rollback boundary + stronger smoke next
  work.

### 4. Validation & Error Matrix
- Manifest invalid after probe removal -> stop and fix the manifest shape first.
- Route-group readiness still reports `has_python_fallback = true` ->
  fallback probes were not fully retired; cutover is incomplete.
- `phase5-p1-fallback` or `phase5-p0-fallback` still matches probes for the
  collapsed route group -> cutover is incomplete.
- `phase5-*` fallback command returns `no probes matched route_groups=[...]` ->
  this is the expected success signal after fallback retirement, not a failure.

### 5. Good/Base/Bad Cases
- Good:
  - readiness summary for the route group shows only Rust owner probes
  - fallback profile returns `no probes matched route_groups=[...]`
  - docs explicitly say rollback requires gateway retargeting first
- Base:
  - Rust owner probes exist and pass, but docs still say `/api/...` can point
    back to Python by default; this is only partial completion
- Bad:
  - route group is reported as “migrated” while active Python fallback probes
    are still present in the manifest
  - docs say fallback is explicit rollback, but readiness still reports
    `has_python_fallback = true`

### 6. Tests Required
- Manifest validation must pass after probe removal.
- `backend/tests/test_tools/test_run_strangler_gateway_smoke.py` must pass.
- Route-group readiness output must assert:
  - `owner_counts` contains only `rust`
  - `has_python_fallback = false`
- Fallback profile check must assert the command exits with the documented
  `no probes matched route_groups=[...]` result for the collapsed group.

### 7. Wrong vs Correct
#### Wrong
- Keep the Python fallback probes active “just in case” while also marking the
  group as fully cut over.
- Treat fallback-profile `no probes matched` as a regression and re-add stale
  Python probes to silence the command.

#### Correct
- Remove the active same-path Python probes once the route-group owner is
  explicitly Rust and rollback has become a gateway action.
- Treat readiness Rust-only + fallback-profile `no probes matched` as the
  completion signal, then move the remaining work to stronger business smoke.

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

## Scenario: Rust chapter candidate executor owner

### 1. Scope / Trigger
- Trigger: a migration package ports candidate orchestration from
  `backend/app/services/chapter_candidate_executor_service.py` or wires Rust
  candidate generation, repair, finalize, and runtime-state owners into one
  executor-level owner.
- Why this needs code-spec depth: the executor is the boundary that decides
  whether candidate owners are merely staged helpers or a coherent replacement
  path. Counting generation / repair / finalize helpers without an executor
  owner hides the fact that Python still owns the active orchestration.

### 2. Signatures
- Rust owner file:
  `backend-rs/src/services/chapter_candidate_executor_service.rs`.
- Expected owner function:
  `generate_best_ranked_candidate_workflow(request, dependencies)
  -> Result<Value, String>`.
- Expected request struct:
  `ChapterCandidateExecutorRequest { base_generate_kwargs,
  target_word_count, source, generation_label, max_candidates,
  runtime_state }`.
- Expected dependency bundle:
  `ChapterCandidateExecutorDependencies { generate_candidate_pool_fn,
  maybe_apply_word_budget_repair_fn, execute_targeted_final_repair_pass_fn,
  resolve_candidate_finalize_state_fn,
  finalize_selected_candidate_result_fn,
  should_apply_targeted_final_repair_fn,
  should_apply_followup_targeted_final_repair_fn,
  select_targeted_final_repair_seed_candidate_fn }`.
- Current staged composed Rust owners:
  `chapter_candidate_generation_service.rs`,
  `chapter_candidate_record_service.rs`,
  `chapter_candidate_word_budget_repair_service.rs`,
  `chapter_candidate_targeted_final_repair_service.rs`,
  `chapter_candidate_finalize_service.rs`,
  `chapter_candidate_runtime_state_service.rs`, and
  `chapter_candidate_output_service.rs`.

### 3. Contracts
- The executor must preserve the Python stage order:
  generation -> word-budget repair -> optional pre-finalize targeted repair ->
  finalize with word-budget repair promotion -> optional post-finalize
  targeted repair -> optional follow-up targeted repair -> final finalize.
- `base_prompt` and `base_temperature` must be resolved once from
  `base_generate_kwargs` and forwarded consistently into generation and repair
  owners.
- Runtime state must be handed off through every owner request and restored to
  the executor request after each stage.
- Post-finalize targeted repair seed selection must prefer:
  current selected candidate when follow-up applies, then deferred seed, then
  no new seed when the winner is already targeted quality repair, then the
  injected seed selector.
- Rerank-heavy formulas remain injectable until the Rust rerank package or
  production cutover adapter explicitly owns them.
- A staged executor owner is not production cutover by itself. Do not mark
  `chapter_candidate_executor_service.py` as retired until Rust default wiring
  and the active generation path consume this owner.

### 4. Validation & Error Matrix
- Generation callback fails -> executor returns `Err(String)` before running
  later repair/finalize stages.
- Invalid or missing temperature -> default to `0.8`.
- Selected candidate needs targeted repair before finalize -> pre-finalize
  targeted pass runs with `targeted-repair` suffix and may defer a follow-up
  seed.
- Finalized winner needs follow-up -> post-finalize repair runs first, then
  follow-up repair runs after another finalize-state resolution.
- Finalized winner is already targeted quality repair and follow-up is false
  -> no new post-finalize seed is selected.
- Deferred targeted seed exists and follow-up is false -> deferred seed wins
  over selecting a new seed.

### 5. Good/Base/Bad Cases
- Good: Rust executor composes all staged candidate owners, focused tests prove
  the stage chain, and docs still label the owner as staged until production
  wiring consumes it.
- Base: executor owner exists with injectable rerank formulas and validated
  adjacent owner tests, while Python remains the active orchestration fallback.
- Bad: adding more isolated candidate helpers without an executor owner and
  reporting them as active-path migration.
- Bad: changing candidate order, follow-up repair conditions, runtime-state
  handoff, or finalization behavior while claiming a behavior-preserving port.

### 6. Tests Required
- Unit tests in `chapter_candidate_executor_service.rs` for:
  - full stage chain with pre-targeted, post-finalize, and follow-up repair
  - deferred post-finalize targeted seed priority
  - skipping new seed when the finalized winner is already targeted repair
- Adjacent owner regression tests for generation, record, word-budget repair,
  targeted final repair, and finalize owners.
- `cargo check --manifest-path "backend-rs/Cargo.toml"` with a dedicated
  target dir when build artifacts must stay out of the workspace.
- When production wiring changes, add route/service smoke or API parity
  validation for the active candidate execution path.

### 7. Wrong vs Correct
#### Wrong
```rust
// Adds one more helper but leaves the Python executor as the only composition
// boundary, then reports candidate executor migration as complete.
fn select_seed_only(...) -> Value { ... }
```

#### Correct
```rust
// The executor owns the stage order while high-risk formula choices remain
// injected until the production cutover package takes them over.
let result = generate_best_ranked_candidate_workflow(&mut request, &mut dependencies).await?;
```

---

## Scenario: Rust chapter candidate executor wiring owner

### 1. Scope / Trigger
- Trigger: a migration package ports the dependency graph from
  `backend/app/services/chapter_candidate_executor_wiring_service.py` or
  prepares the candidate executor package for production cutover.
- Why this needs code-spec depth: wiring is where staged owners become either
  a real cutover path or another pile of helpers. The Rust wiring owner must
  show which dependencies are Rust-owned and which rerank formulas remain
  external blockers.

### 2. Signatures
- Rust owner file:
  `backend-rs/src/services/chapter_candidate_executor_wiring_service.rs`.
- Expected plan builder:
  `build_default_chapter_candidate_executor_wiring_plan()
  -> ChapterCandidateExecutorWiringPlan`.
- Expected validator:
  `validate_candidate_executor_wiring_plan(plan) -> Result<(), String>`.
- Expected readiness resolver:
  `resolve_candidate_executor_wiring_readiness(plan)
  -> ChapterCandidateExecutorWiringReadiness`.

### 3. Contracts
- The wiring plan must include these stages: `generation`,
  `word_budget_repair`, `targeted_final_repair`, `finalize`, and `executor`.
- Rust-owned dependencies must point at their Rust owner files, not back to the
  Python wiring source.
- Rerank-heavy formulas from `chapter_candidate_rerank_service.py` must remain
  marked as external formula callbacks only until a Rust rerank owner or
  cutover adapter takes them over.
- Once `chapter_candidate_rerank_service.rs` owns those formulas, the wiring
  graph must mark the formula names as Rust-owned dependencies and expose zero
  external formula blockers.
- A wiring-plan owner is staged readiness work. It does not retire
  `chapter_candidate_executor_wiring_service.py` or make the active generation
  path consume Rust by itself.
- If the next package creates executable default dependencies, it must replace
  the external formula blockers with Rust owners or explicitly document the
  remaining fallback/callback boundary.

### 4. Validation & Error Matrix
- Missing required stage -> validation returns
  `missing candidate executor wiring stage: <stage>`.
- Stage without owner file -> validation fails before cutover is reported.
- Stage without dependencies -> validation fails because it cannot prove a real
  owner graph.
- Rust-owned output, record, runtime-state, or executor dependencies appearing
  as formula blockers -> migration tracking regression.
- External formula dependency count dropping without a matching Rust owner or
  cutover adapter -> documentation drift.
- `select_best_generation_candidate` or targeted/word-budget formula names
  still pointing to Python after a Rust rerank owner exists -> cutover
  readiness regression.

### 5. Good/Base/Bad Cases
- Good: wiring plan lists every candidate executor stage, separates Rust-owned
  owners from external formula callbacks, and tests either cutover blockers or
  the absence of blockers after the Rust rerank owner exists.
- Base: plan owner exists and is validated, while production wiring still uses
  Python.
- Bad: report the Python wiring file as retired because a Rust wiring plan
  exists.
- Bad: hide rerank formula dependencies inside opaque strings without
  readiness counts or validation.

### 6. Tests Required
- Unit tests in `chapter_candidate_executor_wiring_service.rs` for:
  - full stage coverage
  - rerank formulas marked as cutover blockers before Rust rerank ownership,
    or Rust-owned dependencies after `chapter_candidate_rerank_service.rs`
    exists
  - default plan validation
  - missing-stage validation failure
  - Rust-owned dependencies excluded from formula blockers
- Adjacent executor tests must still pass after module registration.
- `cargo check --manifest-path "backend-rs/Cargo.toml"` with a dedicated
  target dir.

### 7. Wrong vs Correct
#### Wrong
```rust
// A vague flag that says wiring is ready, without exposing the dependency
// graph or the remaining Python formula blockers.
const CANDIDATE_WIRING_READY: bool = true;
```

#### Correct
```rust
let plan = build_default_chapter_candidate_executor_wiring_plan();
validate_candidate_executor_wiring_plan(&plan)?;
let readiness = resolve_candidate_executor_wiring_readiness(&plan);
assert!(!readiness.cutover_blockers.is_empty());
```

---

## Scenario: Rust chapter candidate rerank owner

### 1. Scope / Trigger
- Trigger: a migration package ports formula-heavy candidate ranking,
  retry, word-budget repair, targeted final repair, and selection metadata
  logic from `backend/app/services/chapter_candidate_rerank_service.py`.
- Why this needs code-spec depth: the candidate executor cannot become a real
  Rust cutover path while selection, repair, retry, and seed formulas remain
  opaque Python callbacks. A partial helper port would hide the remaining
  production blocker.

### 2. Signatures
- Rust owner file:
  `backend-rs/src/services/chapter_candidate_rerank_service.rs`.
- Expected owner functions include:
  `select_best_generation_candidate(...)`,
  `should_generate_additional_candidate(...)`,
  `normalize_candidate_quality_gate_plan(...)`,
  `build_candidate_selection_metadata(...)`,
  `attach_candidate_selection_metadata(...)`,
  `build_candidate_pool_summary(...)`.
- Expected word-budget functions include:
  `should_apply_word_budget_repair(...)`,
  `build_word_budget_repair_suffix(...)`,
  `resolve_word_budget_repair_temperature(...)`,
  `resolve_word_budget_repair_max_tokens(...)`,
  `resolve_word_budget_repair_char_limit(...)`,
  `should_keep_word_budget_repair_candidate(...)`,
  `should_prefer_word_budget_repair_candidate(...)`.
- Expected targeted final repair functions include:
  `should_apply_targeted_final_repair(...)`,
  `should_apply_followup_targeted_final_repair(...)`,
  `build_targeted_final_repair_suffix(...)`,
  `resolve_targeted_final_repair_temperature(...)`,
  `resolve_targeted_final_repair_max_tokens(...)`,
  `resolve_targeted_final_repair_char_limit(...)`,
  `should_keep_targeted_final_repair_candidate(...)`,
  `should_adopt_targeted_final_repair_candidate(...)`,
  `should_prefer_targeted_final_repair_candidate(...)`,
  `select_targeted_final_repair_seed_candidate(...)`.

### 3. Contracts
- Target word bounds, severe word-budget pressure, and quality-gate
  normalization must preserve Python behavior for `allow_save -> auto_repair`
  under severe over/under target pressure.
- Best-candidate ranking must preserve the Python order:
  quality-gate priority, selection score, overall score, word-count fit score,
  then lower candidate index as the tie-breaker.
- Word-budget repair keep/prefer decisions must account for target-window
  fit, severe over-budget pressure, quality drop, failed metric count, and
  substantial/decisive word-count improvement.
- Targeted final repair seed eligibility must preserve manual-review,
  target-window, continuity-warning, score-floor, allowed focus-area, and
  focused polish-shape gates.
- Prompt suffix ports may focus on behaviorally important instruction lines
  rather than byte-for-byte text, but tests must assert the key pass labels,
  focus-specific lines, and target-window instructions.
- This owner is staged until Rust default executor wiring or a production
  adapter consumes it. Do not report
  `chapter_candidate_rerank_service.py` as active-path retired solely because
  the Rust formula owner exists.

### 4. Validation & Error Matrix
- Severe word-count pressure with an `allow_save` gate -> normalized gate has
  `decision = auto_repair`, `allow_save = false`, and
  `can_auto_repair = true`.
- Candidate pool ranking with equal quality scores -> lower candidate index
  wins the tie.
- Word-budget repair candidate closer to target with acceptable quality drop
  -> keep/prefer returns true.
- Manual-review targeted candidate with allowed focus areas and score floor
  -> targeted repair and follow-up formulas accept it when attempt kind allows.
- Wiring readiness after this owner exists -> rerank formula names point to
  `backend-rs/src/services/chapter_candidate_rerank_service.rs` and external
  formula blocker count is zero.

### 5. Good/Base/Bad Cases
- Good: port the whole rerank formula group, update executor wiring readiness
  to Rust-owned dependencies, and validate rerank, wiring, executor, and
  `cargo check`.
- Base: staged Rust owner exists and wiring blockers are removed, while
  Python active path remains unchanged until production adapter/default
  dependency builder work lands.
- Bad: port only suffix text or only selection ranking and claim the Python
  rerank service is migrated.
- Bad: mark wiring blockers as gone without a Rust function group that tests
  word-budget and targeted final repair decisions.

### 6. Tests Required
- Unit tests in `chapter_candidate_rerank_service.rs` for quality-gate
  normalization, selection metadata, best-candidate ranking, retry decision,
  word-budget repair apply/keep/prefer/max-token/char-limit, targeted final
  repair apply/follow-up/seed/keep/adopt/prefer, suffix key lines, pool
  summary, and retry temperature.
- Unit tests in `chapter_candidate_executor_wiring_service.rs` proving rerank
  formulas are Rust-owned dependencies after the owner exists.
- Adjacent executor tests must remain green.
- `cargo check --manifest-path "backend-rs/Cargo.toml"` with a dedicated
  target dir when build artifacts must stay out of the workspace.

### 7. Wrong vs Correct
#### Wrong
```rust
// Only ranking moved; repair and seed decisions still live in Python callbacks.
fn select_best_generation_candidate(...) -> Option<Value> { ... }
```

#### Correct
```rust
// The whole formula group is owned together, and wiring can now point all
// rerank-heavy dependency names at the Rust owner.
let readiness = resolve_candidate_executor_wiring_readiness(&plan);
assert_eq!(readiness.external_formula_dependency_count, 0);
```

## Scenario: Rust chapter candidate executor default dependency owner

### 1. Scope / Trigger
- Trigger: a migration package ports the default dependency builder from
  `backend/app/services/chapter_candidate_executor_wiring_service.py` or makes
  the staged Rust candidate executor consume Rust rerank formulas directly.
- Why this needs code-spec depth: default wiring is the boundary between
  staged owners and active cutover readiness. If it is only documented as a
  dependency graph, the Rust executor still cannot prove that generation,
  repair, rerank, and finalize owners compose into one executable package.

### 2. Signatures
- Rust owner file:
  `backend-rs/src/services/chapter_candidate_executor_default_dependency_service.rs`.
- Expected owner function:
  `generate_best_ranked_candidate_with_default_dependency_wiring(request,
  collect_output, build_record, quality_gate_plan_builder) -> Result<Value,
  String>`.
- Expected injected boundaries:
  `ChapterCandidateDefaultOutputCollectInput` and
  `ChapterCandidateDefaultRecordBuildInput`.
- Expected wiring-plan consumer:
  `backend-rs/src/services/chapter_candidate_executor_wiring_service.rs`
  lists the default dependency owner as a Rust target and executor-stage
  dependency.

### 3. Contracts
- The owner must compose generation, word-budget repair, targeted final
  repair, finalize, and rerank formulas in Rust.
- Provider output collection, candidate record construction, and quality gate
  plan construction stay injectable until the active production adapter owns
  provider/runtime integration.
- Rerank formula calls must go through
  `chapter_candidate_rerank_service.rs`, not Python formula callbacks.
- Runtime state must continue to be passed through the staged generation,
  repair, targeted repair, and finalize owners.
- This owner is staged until the active Python generation path consumes it.
  Do not report `chapter_candidate_executor_wiring_service.py` or
  `chapter_candidate_executor_service.py` as active-path retired solely
  because executable Rust default wiring exists.

### 4. Validation & Error Matrix
- One-candidate request with word-budget pressure -> default wiring can
  generate, repair, finalize, and return the repair winner.
- Multi-candidate request -> default retry suffix formula is applied before
  later repair/finalize stages.
- Wiring readiness -> target map includes the default dependency owner and
  executor-stage dependencies reference it.
- `cargo check` must pass with the dedicated target dir used for this package.

### 5. Good/Base/Bad Cases
- Good: executable Rust default dependency owner composes the staged candidate
  package and validates adjacent wiring/executor/rerank tests.
- Base: staged owner exists, but provider output and production route adapter
  remain explicit injection/fallback boundaries.
- Bad: only add a readiness entry and claim the Python default wiring file is
  migrated.
- Bad: hide provider output or quality gate construction behind hard-coded
  test closures and remove the explicit production cutover boundary.

### 6. Tests Required
- Unit tests in
  `chapter_candidate_executor_default_dependency_service.rs` proving default
  wiring executes the package and applies default retry/rerank formulas.
- Unit tests in `chapter_candidate_executor_wiring_service.rs` proving the
  default owner is part of the Rust target/dependency map.
- Adjacent executor and rerank tests must remain green.
- `cargo check --manifest-path "backend-rs/Cargo.toml"` with a dedicated
  target dir when build artifacts must stay out of the workspace.

## Scenario: Rust chapter candidate executor runtime adapter owner

### 1. Scope / Trigger
- Trigger: a migration package reduces the remaining Python active-path
  injection surface for candidate executor provider output, record build, or
  quality adapter callbacks.
- Why this needs code-spec depth: the default dependency owner is executable,
  but it still needs runtime adapters before production can consume it without
  recreating Python callback assembly in routes or compat shells.

### 2. Signatures
- Rust owner file:
  `backend-rs/src/services/chapter_candidate_executor_runtime_adapter_service.rs`.
- Rust provider stream owner file:
  `backend-rs/src/services/chapter_candidate_provider_stream_service.rs`.
- Rust quality adapter and runtime callback owner file:
  `backend-rs/src/services/chapter_candidate_quality_adapter_service.rs`.
- Rust default dependency record mapping owner file:
  `backend-rs/src/services/chapter_candidate_executor_default_dependency_service.rs`.
- Expected owner functions:
  `generate_best_ranked_candidate_with_runtime_adapters(...)`.
- Expected owner functions:
  `collect_default_generation_candidate_output(...)` and
  `resolve_default_candidate_provider_stream_request(...)` in the provider
  stream owner.
- Expected owner functions:
  `build_default_generation_candidate_record(...)` in the default dependency
  owner.
- Expected owner functions:
  `generate_best_ranked_candidate_with_runtime_quality_adapters(...)`.
- Expected owner functions:
  `build_runtime_quality_adapter_callbacks(...)` and
  `with_locked_callback(...)` in the shared quality adapter owner.

### 3. Contracts
- Runtime adapter must consume the Rust default dependency owner instead of
  reassembling generation, repair, finalize, or rerank dependencies.
- Provider request resolution must preserve prompt, optional system prompt,
  tools payload, temperature override, max-token override, candidate index, and
  max-output character limit. This logic belongs in
  `chapter_candidate_provider_stream_service.rs`, not in the runtime adapter.
- Provider request resolution must reject non-finite temperature values before
  constructing `AIService`. The error string must remain
  `candidate provider temperature must be a finite number`.
- `temperature` and `max_tokens` overrides may arrive as JSON numbers or
  strings. `temperature` must parse to a finite `f64`; `max_tokens` must parse
  to a positive `u32`.
- Candidate record construction must call `chapter_candidate_record_service.rs`
  through the default dependency owner and propagate record owner errors.
- Default dependency record input mapping belongs beside
  `ChapterCandidateDefaultRecordBuildInput` in
  `chapter_candidate_executor_default_dependency_service.rs`, not in the
  executor runtime adapter or a forwarding-only record bridge module.
- Generation, word-budget repair, targeted repair, and default dependency
  owners must treat record callbacks as `Result<Value, String>` so sanitized
  empty content or workflow/meta text can stop the executor instead of being
  hidden behind a fake candidate record.
- Quality evaluator and quality gate plan builder may remain injectable for
  low-level tests, but production-oriented runtime wiring should prefer
  `ChapterCandidateQualityAdapter` through
  `generate_best_ranked_candidate_with_runtime_quality_adapters(...)`.
- Runtime quality callback materialization and poisoned callback-lock recovery
  belong in `chapter_candidate_quality_adapter_service.rs`; do not rebuild
  those closures in routes, compat shells, production adapter, the executor
  runtime adapter, or a forwarding-only bridge module.
- This owner is still staged until a route/production adapter consumes it.
  Do not report Python candidate executor active path as retired solely
  because runtime adapters exist.

### 4. Validation & Error Matrix
- Provider kwargs with prompt/system prompt/tools/temperature/max_tokens ->
  Rust provider request resolves those values and overrides `AIConfig` safely.
- String temperature/max_tokens overrides -> Rust provider request parses them
  and applies them before streaming.
- Non-finite temperature override -> returns an adapter error before provider
  streaming.
- Invalid tools payload -> returns an adapter error before provider streaming.
- Normal candidate record input -> Rust record owner returns enriched
  selection metadata.
- Empty/sanitized-invalid candidate content -> record owner error propagates
  through the adapter-capable callback contract.
- Quality adapter input -> runtime adapter builds evaluator and gate-plan
  callbacks from `ChapterCandidateQualityAdapter` rather than requiring route
  or compat code to assemble two Python-style closures.
- Poisoned callback lock -> `with_locked_callback(...)` recovers through the
  shared quality adapter owner and continues invoking the callback.
- Existing generation, word-budget repair, targeted repair, default dependency,
  wiring, and record owner tests remain green.

### 5. Good/Base/Bad Cases
- Good: runtime adapter consumes default dependency owner, provider stream
  owner, shared quality adapter/callback owner, and record owner, while
  validation proves record errors and callback behavior remain stable.
- Base: staged runtime adapter exists but active Python route still calls the
  Python executor until a rollback-aware production adapter lands.
- Bad: parse provider kwargs in a route or compat shell and call the Rust
  executor with ad-hoc closures.
- Bad: move provider request parsing or runtime quality callback materialization
  back into `chapter_candidate_executor_runtime_adapter_service.rs` after
  dedicated Rust owners exist.
- Bad: reintroduce `chapter_candidate_runtime_callback_bridge_service.rs` as a
  forwarding-only module after the callback contract has been absorbed into
  `chapter_candidate_quality_adapter_service.rs`.
- Bad: move default record input mapping back into
  `chapter_candidate_executor_runtime_adapter_service.rs` after the default
  dependency owner owns the mapping.
- Bad: reintroduce `chapter_candidate_runtime_record_bridge_service.rs` as a
  forwarding-only module after the default record mapping has been absorbed
  into `chapter_candidate_executor_default_dependency_service.rs`.
- Bad: keep record callbacks infallible and unwrap record errors in tests.

### 6. Tests Required
- Unit tests in
  `chapter_candidate_provider_stream_service.rs` for provider request
  resolution, finite temperature validation, max-token validation, tools
  parsing, and max-output conversion.
- Unit tests in `chapter_candidate_quality_adapter_service.rs` for
  quality-adapter callback bridging and poisoned-lock recovery.
- Unit tests in `chapter_candidate_executor_default_dependency_service.rs` for
  default record building and record error propagation.
- Regression tests in generation, word-budget repair, targeted repair, default
  dependency, wiring, and record services after changing callback contracts.
- Unit tests in `chapter_candidate_executor_wiring_service.rs` must prove that
  provider stream, quality callback, and default record dependencies point to
  their real Rust owner files. They should also keep deleted forwarding-only
  bridge files out of the Rust target map.
- `cargo check --manifest-path "backend-rs/Cargo.toml"` with a dedicated
  target dir when build artifacts must stay out of the workspace.

## Scenario: Rust chapter candidate executor production adapter owner

### 1. Scope / Trigger
- Trigger: a migration package prepares active chapter generation routes to
  consume the Rust candidate executor package while keeping rollback to the
  Python executor observable.
- Why this needs code-spec depth: the runtime-quality adapter is executable,
  but production cutover still needs one owner for enablement, Python fallback,
  Rust failure rollback, and route-facing decision metadata. Without this
  owner, routes and compatibility shells would rebuild cutover logic ad hoc.

### 2. Signatures
- Rust owner file:
  `backend-rs/src/services/chapter_candidate_executor_production_adapter_service.rs`.
- Expected owner function:
  `resolve_chapter_candidate_production_adapter_decision(...)`.
- Expected owner function:
  `execute_chapter_candidate_production_adapter(...)`.
- Expected test hook:
  `execute_chapter_candidate_production_adapter_with_executor(...)`.
- Expected decision enum:
  `ChapterCandidateProductionExecutionPath::{RustCandidateExecutor, PythonFallback}`.

### 3. Contracts
- The production adapter must call
  `generate_best_ranked_candidate_with_runtime_quality_adapters(...)` for the
  default Rust path; tests may inject a fake Rust executor through the explicit
  test hook.
- The adapter must expose an explicit config for Rust enablement,
  `fallback_on_rust_error`, disabled reason, and rollback boundary.
- When Rust is disabled, the Python fallback must run without invoking the Rust
  executor.
- When Rust fails and rollback is enabled, the fallback context must include the
  Rust error, fallback reason, and rollback boundary.
- When Rust fails and rollback is disabled, the Rust error must propagate.
- This owner is staged until a Rust route or deployment gateway consumes it.
  Do not report the Python candidate executor active path as retired solely
  because this production adapter exists.

### 4. Validation & Error Matrix
- Rust enabled + Rust executor succeeds -> output path remains
  `RustCandidateExecutor`, fallback is not applied, and runtime state can be
  updated by the Rust executor.
- Rust disabled -> output path is `PythonFallback`, fallback reason comes from
  config, and Rust executor is not called.
- Rust enabled + Rust error + rollback enabled -> output path becomes
  `PythonFallback`, `rust_error` is preserved, and fallback receives rollback
  metadata.
- Rust enabled + Rust error + rollback disabled -> error is returned without
  invoking fallback.
- Wiring readiness -> candidate executor wiring plan includes the
  `production_adapter` stage and target owner.

### 5. Good/Base/Bad Cases
- Good: route-level consumption calls the production adapter and keeps a smoke
  or feature flag that can route back to Python.
- Base: production adapter owner is staged and tested, while active Python
  routes still call the Python executor until route cutover lands.
- Bad: add a production adapter but keep route-level cutover decisions hidden
  in Python closures or local route branches.
- Bad: count Python executor retirement before the route/deployment gateway
  consumes this adapter with rollback evidence.

### 6. Tests Required
- Unit tests in
  `chapter_candidate_executor_production_adapter_service.rs` for Rust enabled,
  Rust disabled, rollback-on-error, and no-rollback error propagation.
- Unit tests in `chapter_candidate_executor_wiring_service.rs` proving the
  production adapter is part of the Rust target/dependency map.
- `cargo check --manifest-path "backend-rs/Cargo.toml"` with a dedicated
  target dir when build artifacts must stay out of the workspace.

## Scenario: Rust chapter candidate route gateway owner

### 1. Scope / Trigger
- Trigger: a migration package adds deployment or route-gateway cutover config
  for the Rust chapter candidate executor production adapter.
- Why this needs code-spec depth: the production adapter owns rollback-aware
  execution decisions, but routes still need one Rust owner that maps app/env
  config into that adapter. Without this gateway, each route or compatibility
  shell can rebuild enablement, disabled-reason, fallback, and rollback-boundary
  decisions locally.

### 2. Signatures
- Rust owner file:
  `backend-rs/src/services/chapter_candidate_route_gateway_service.rs`.
- Expected owner function:
  `build_chapter_candidate_route_gateway_config_from_app_config(...)`.
- Expected owner function:
  `build_chapter_candidate_production_adapter_config_from_route_gateway(...)`.
- Expected owner function:
  `execute_chapter_candidate_route_gateway(...)`.
- Expected test hook:
  `execute_chapter_candidate_route_gateway_with_executor(...)`.
- Expected config env vars:
  `CHAPTER_CANDIDATE_RUST_EXECUTOR_ENABLED`,
  `CHAPTER_CANDIDATE_RUST_EXECUTOR_FALLBACK_ON_ERROR`,
  `CHAPTER_CANDIDATE_RUST_EXECUTOR_DISABLED_REASON`, and
  `CHAPTER_CANDIDATE_RUST_EXECUTOR_ROLLBACK_BOUNDARY`.

### 3. Contracts
- App config must default the Rust candidate executor to disabled and fallback
  on Rust error to enabled.
- Blank disabled reasons must normalize to `None`; blank rollback boundaries
  must normalize to `python_candidate_executor_fallback`.
- The gateway must delegate execution to the production adapter instead of
  reimplementing Rust enabled / Python fallback / rollback-on-error decisions.
- The gateway may expose an injected Rust executor for tests, but the default
  path must still go through
  `execute_chapter_candidate_production_adapter(...)`.
- This owner is staged until an active Rust route, deployment gateway, or smoke
  probe consumes it. Do not report the Python candidate executor active path as
  retired solely because route-gateway config exists.

### 4. Validation & Error Matrix
- Env/app config with Rust enabled and fallback disabled -> gateway config
  preserves both flags.
- Disabled reason and rollback boundary with surrounding whitespace -> gateway
  trims both values before building the production adapter config.
- Blank reason/boundary -> reason becomes `None` and rollback boundary becomes
  `python_candidate_executor_fallback`.
- Rust enabled + fake Rust executor succeeds -> gateway returns the Rust result
  without Python fallback.
- Rust disabled -> gateway calls Python fallback and never invokes the Rust
  executor.
- Wiring readiness -> candidate executor wiring plan includes the
  `route_gateway` stage before the production adapter stage.

### 5. Good/Base/Bad Cases
- Good: an active Rust route or deployment smoke probe consumes the gateway,
  with a documented rollback flag and Python fallback path.
- Base: gateway owner is staged and tested, while active Python routes still
  call the Python executor until route parity and smoke evidence are ready.
- Bad: add env flags to `AppConfig` but leave routes to manually rebuild
  production-adapter config.
- Bad: count Python executor retirement before
  `execute_chapter_candidate_route_gateway(...)` is consumed by a real active
  path or smoke probe.

### 6. Tests Required
- Unit tests in `chapter_candidate_route_gateway_service.rs` for config
  mapping, blank-default normalization, Rust path execution, and disabled
  Python fallback.
- Unit tests in `chapter_candidate_executor_wiring_service.rs` proving the
  route gateway is part of the Rust target/dependency map.
- `cargo test config --manifest-path "backend-rs/Cargo.toml"` after adding
  app/env config fields.
- `cargo check --manifest-path "backend-rs/Cargo.toml"` with a dedicated
  target dir when build artifacts must stay out of the workspace.

## Scenario: Rust chapter candidate route gateway smoke owner

### 1. Scope / Trigger
- Trigger: a migration package needs deployment-smoke evidence that the Rust
  route gateway can be consumed before repointing the active chapter-generation
  route.
- Why this needs code-spec depth: route-gateway config alone is not enough to
  prove cutover readiness. A smoke owner must execute the gateway through both
  Rust and Python-fallback paths and preserve rollback metadata without
  changing the production route.

### 2. Signatures
- Rust owner file:
  `backend-rs/src/services/chapter_candidate_route_gateway_smoke_service.rs`.
- Expected owner function:
  `build_default_chapter_candidate_route_gateway_smoke_probes(...)`.
- Expected owner function:
  `run_chapter_candidate_route_gateway_smoke_suite(...)`.
- Expected owner function:
  `run_chapter_candidate_route_gateway_smoke_probe(...)`.
- Required gateway consumer:
  `execute_chapter_candidate_route_gateway_with_executor(...)`.
- Expected observable route:
  `GET /health/chapter-candidate-route-gateway-smoke`.

### 3. Contracts
- The default smoke suite must include one Rust-owner probe and one forced
  Python-fallback probe.
- The Rust probe must enable the Rust candidate executor and return a result
  that proves the gateway was consumed.
- The Python-fallback probe must disable the Rust candidate executor, preserve
  the disabled reason, and keep the rollback boundary observable.
- Smoke execution must delegate through the route gateway test hook instead of
  calling the production adapter directly.
- Runtime state must be updated with whether the smoke path reached Rust or
  Python fallback.
- The deployment endpoint may expose fake-provider smoke evidence publicly only
  because it does not call the real AI provider or expose user data.
- This owner does not retire the active generation route until that route
  consumes the route gateway with route parity and rollback evidence.

### 4. Validation & Error Matrix
- Default probes -> exactly two probes: `rust` and `python-fallback`.
- Rust probe -> execution path is `rust_candidate_executor`, fallback is not
  applied, result contains `gateway_consumed = true`, and runtime state marks
  `gateway_smoke = rust`.
- Python-fallback probe -> execution path is `python_fallback`, fallback is
  applied, fallback reason is preserved, result contains
  `gateway_consumed = true`, and runtime state marks
  `gateway_smoke = python-fallback`.
- Smoke result metadata -> probe name, owner, route group, rollback boundary,
  Rust error, result payload, and runtime state remain available to a future
  deployment manifest or endpoint.
- Smoke result readiness evidence -> covered Rust owners, Python source map,
  gateway enablement/fallback flags, runtime owner chain, fallback metadata
  fields, and next cutover gate remain observable from the service result.
- Deployment endpoint -> returns status `ok`, owner `rust`, route group
  `chapters`, two probe results, and rollback boundary
  `python_candidate_executor_fallback`.
- Deployment endpoint -> includes each probe's `readiness_evidence` so
  deployment smoke can verify the provider stream, quality adapter, default
  dependency, record owner, route gateway, production adapter, and runtime
  adapter chain without calling the real provider.
- Wiring readiness -> candidate executor wiring plan includes
  `route_gateway_smoke` before `route_gateway`.

### 5. Good/Base/Bad Cases
- Good: deployment smoke or an active Rust route consumes
  `run_chapter_candidate_route_gateway_smoke_suite(...)`, records Rust and
  Python-fallback evidence, and keeps rollback boundary visible.
- Base: staged smoke owner exists and the Rust health route exposes deployment
  evidence, while active Python generation routes still call the Python
  executor.
- Bad: report Python candidate executor active-path retirement because the
  smoke owner exists but no active route or deployment runner invokes it.
- Bad: a smoke implementation calls the production adapter directly and bypasses
  the route gateway configuration owner.

### 6. Tests Required
- Unit tests in `chapter_candidate_route_gateway_smoke_service.rs` for default
  probe construction, Rust-path smoke execution, Python-fallback smoke
  execution, runtime-state updates, readiness evidence, and metadata
  preservation.
- Unit tests in `chapter_candidate_route_gateway_service.rs` must remain green
  because the smoke owner consumes the gateway contract.
- Unit tests in `chapter_candidate_executor_wiring_service.rs` proving the
  `route_gateway_smoke` stage and target owner are part of the Rust package.
- Unit tests in `health.rs` proving the endpoint exposes both smoke paths
  and readiness evidence without requiring a real provider.
- `deploy/strangler-gateway-probes.json` should include a `chapters` probe for
  `GET /health/chapter-candidate-route-gateway-smoke`.
- `cargo check --manifest-path "backend-rs/Cargo.toml"` with a dedicated
  target dir when build artifacts must stay out of the workspace.

## Scenario: Rust chapter candidate executor Send-safe active-route prep owner

### 1. Scope / Trigger
- Trigger: a migration package moves the staged Rust candidate executor toward
  direct Axum / active-route consumption after the route gateway and smoke
  owner already exist.
- Why this needs code-spec depth: a staged Rust owner can pass unit tests while
  still returning non-`Send` futures because callback state is held in
  `Rc<RefCell<_>>`. Active route handlers must not rely on `spawn_blocking`
  quarantine or a current-thread runtime just to await the candidate executor.

### 2. Signatures
- Rust owner file:
  `backend-rs/src/services/chapter_candidate_executor_runtime_adapter_service.rs`.
- Rust owner file:
  `backend-rs/src/services/chapter_candidate_executor_default_dependency_service.rs`.
- Required upstream boundary:
  `backend-rs/src/services/chapter_candidate_executor_production_adapter_service.rs`.
- Required route-gateway boundary:
  `backend-rs/src/services/chapter_candidate_route_gateway_service.rs`.
- Required Send-safe executor hook shape:
  `Pin<Box<dyn Future<Output = Result<Value, String>> + Send + 'request>>`.
- Expected observable route:
  `GET /health/chapter-candidate-route-gateway-smoke`.

### 3. Contracts
- Shared callback state for candidate output, record building, quality
  evaluation, and quality-gate plan building must use `Arc<Mutex<_>>` or an
  equivalent thread-safe owner. `Rc<RefCell<_>>` is not valid at this boundary.
- Locks must not be held across `.await`. Build the provider/collector future
  while locked, drop the guard, then await the future.
- Callback generic bounds that cross runtime, production adapter, or route
  gateway boundaries must include `Send + 'static` where they are stored behind
  the shared Rust owner.
- Executor and fallback hooks that are consumed by the Rust route gateway smoke
  path must return boxed `Send` futures, so the health route can await the
  smoke suite as a normal Axum handler.
- The health smoke endpoint must not use `spawn_blocking`, a nested
  current-thread runtime, or `block_on(...)` to hide non-`Send` gateway futures.
- Python fallback remains a rollback boundary; do not count it or the active
  generation route as retired until a production route consumes the Rust
  gateway with route parity and rollback evidence.

### 4. Validation & Error Matrix
- Runtime adapter tests prove quality adapter callback bridging still works
  after the state owner changes.
- Default dependency tests prove retry, repair, targeted repair, finalize, and
  default formula composition still execute through the Rust owner.
- Production adapter and route gateway tests must remain green, because the
  `Send` bounds are part of their public Rust boundary.
- Route gateway smoke and health tests must remain green, because deployment
  observability must not regress while active-route prep is tightened.
- Searching `health.rs` and the smoke owner for `spawn_blocking`,
  `current_thread`, and `block_on(run_chapter_candidate_route_gateway_smoke_suite`
  should return no matches after the direct async smoke cutover.
- `cargo check --manifest-path "backend-rs/Cargo.toml"` must pass with a
  dedicated target dir.

### 5. Good/Base/Bad Cases
- Good: callback state is thread-safe, locks are dropped before await, and
  runtime / production / gateway boundaries all advertise the same `Send`
  contract.
- Good: `GET /health/chapter-candidate-route-gateway-smoke` directly awaits
  `run_chapter_candidate_route_gateway_smoke_suite()` on the Axum async path.
- Base: Rust candidate executor is active-route-prep ready, while Python
  fallback remains the rollback path and the active generation route is not yet
  retired.
- Bad: leave `Rc<RefCell<_>>` in runtime/default dependency owners and work
  around it in HTTP handlers with `spawn_blocking`.
- Bad: mark Python active path retired just because the Rust gateway smoke
  endpoint still passes.

### 6. Tests Required
- `cargo test chapter_candidate_executor_runtime_adapter_service`.
- `cargo test chapter_candidate_executor_default_dependency_service`.
- `cargo test chapter_candidate_executor_production_adapter_service`.
- `cargo test chapter_candidate_route_gateway_service`.
- `cargo test chapter_candidate_route_gateway_smoke_service`.
- `cargo test health`.
- `cargo check --manifest-path "backend-rs/Cargo.toml"` with an explicit target
  dir outside the workspace when build artifacts must stay off the project
  drive.

## Scenario: Rust chapter candidate quality adapter owner

### 1. Scope / Trigger
- Trigger: a migration package needs to reduce the remaining candidate executor
  quality hook injection surface before active route consumption.
- Why this needs code-spec depth: quality calculation is a large domain, so the
  safe whole-block migration unit is the hook adapter contract first: runtime
  context projection, metrics input construction, and quality gate plan input
  construction.

### 2. Signatures
- Rust owner file:
  `backend-rs/src/services/chapter_candidate_quality_adapter_service.rs`.
- Expected owner type:
  `ChapterCandidateQualityAdapter`.
- Expected owner function:
  `build_chapter_candidate_quality_adapter(...)`.
- Expected owner methods:
  `evaluate_quality(...)` and `build_quality_gate_plan(...)`.

### 3. Contracts
- The adapter owns the Python hook assembly semantics from:
  `backend/app/services/chapter_generation/stream/candidate_service.py` and
  `backend/app/services/batch_generation_candidate_service.py`.
- Quality runtime context construction remains injectable, but the Rust adapter
  must pass story packet, project, chapter, chapter context, target word count,
  and generation intent as one stable input object.
- Story quality metrics calculation remains injectable, but the Rust adapter
  must resolve generated content, `chapter_context.chapter_outline`,
  `project.world_rules`, and the computed quality runtime context.
- Quality gate plan resolution remains injectable, but the Rust adapter must
  preserve retry count, max retries, current story repair payload, and scope.
- The shared adapter must preserve the Python
  `quality_gate_plan_builder(candidate_metrics, attempt_offset)` callback
  contract by carrying `attempt_offset` through
  `CandidateQualityGatePlanInput`. Do not drop this field in the Rust adapter
  even if the current gate-plan logic does not use it for retry budgeting.
- `attempt_offset` is contract/debug/readiness evidence, not retry budget.
  Do not add it to `retry_count` unless the behavior change is deliberately
  reviewed with route parity tests.
- Non-object candidate metrics must be converted to `None` before gate-plan
  resolution, matching Python's `candidate_metrics if isinstance(..., dict)
  else None` behavior.
- This owner is staged until the runtime adapter or production route consumes
  it. Do not report Python quality hook assembly as active-path retired solely
  because the Rust quality adapter exists.

### 4. Validation & Error Matrix
- Chapter scope -> metrics input includes generated content, chapter outline,
  world rules, and quality runtime context.
- Batch scope -> gate-plan input includes retry budget, scope, and current
  story repair payload.
- Non-object metrics -> gate-plan input receives no candidate metrics.
- Wiring readiness -> candidate executor wiring plan includes the quality
  adapter stage and target owner.

### 5. Good/Base/Bad Cases
- Good: Rust adapter owns hook assembly and keeps heavy quality rule callbacks
  as explicit boundaries until the production adapter consumes them.
- Base: staged owner exists, but active Python route still builds hooks until
  route consumption lands.
- Bad: rewrite the entire quality rule domain in the candidate executor package
  without route parity or smoke coverage.
- Bad: leave `scope`, retry budget, or story repair payload hidden inside a
  Python closure after claiming quality adapter migration is complete.

### 6. Tests Required
- Unit tests in `chapter_candidate_quality_adapter_service.rs` for runtime
  context/metrics projection, gate-plan retry/scope payload, and non-object
  metrics handling.
- Unit tests in `chapter_candidate_executor_wiring_service.rs` proving the
  quality adapter is part of the Rust target/dependency map.
- `cargo check --manifest-path "backend-rs/Cargo.toml"` with a dedicated
  target dir when build artifacts must stay out of the workspace.

## Scenario: Rust single-generation candidate quality rule owner

### 1. Scope / Trigger
- Trigger: the active Rust single-generation route consumes the candidate
  gateway, and the old inline quality adapter still returns a fake fixed
  `overall_score`.
- Why this needs code-spec depth: enabling the Rust candidate executor with a
  fake quality gate can silently change candidate selection, targeted repair,
  persistence metadata, and review/fallback behavior.

### 2. Signatures
- Rust owner file:
  `backend-rs/src/services/chapter_single_generation_candidate_quality_service.rs`.
- Runtime consumer:
  `backend-rs/src/services/chapter_generation_runtime_service.rs`.
- Adaptive profile owner:
  `backend-rs/src/services/novel_quality_profile_service.rs`.
- Owner functions:
  `build_single_generation_quality_runtime_context(...)`,
  `compute_single_generation_story_quality_metrics(...)`, and
  `resolve_single_generation_quality_gate_plan(...)`.
- Adaptive profile functions:
  `resolve_runtime_quality_profile(...)`,
  `resolve_quality_weight_profile(...)`,
  `resolve_adaptive_quality_gate_profile(...)`, and
  `resolve_metric_threshold_adjustments(...)`.
- Prompt-block profile functions:
  `build_novel_quality_profile(...)` and
  `build_novel_quality_prompt_blocks(...)`.
- Prompt runtime contract functions in
  `chapter_generation_prompt_service.rs`:
  `build_quality_preference_block(...)`,
  `build_quality_generation_protocol_block(...)`,
  `build_quality_json_protocol_block(...)`,
  `build_quality_contract_block(...)`, and
  `inject_quality_contract(...)`.
- Adapter input types remain owned by:
  `backend-rs/src/services/chapter_candidate_quality_adapter_service.rs`.

### 3. Contracts
- Runtime context output must preserve story packet, project, chapter,
  chapter context, target word count, and generation intent.
- Metrics output must include `overall_score`, `word_count`, the seven
  Python-source-map metric fields, `details`, optional
  `quality_runtime_context`, optional `continuity_preflight`,
  `repair_guidance`, and `quality_gate`.
- The seven metric fields are:
  `conflict_chain_hit_rate`, `rule_grounding_hit_rate`,
  `outline_alignment_rate`, `dialogue_naturalness_rate`,
  `opening_hook_rate`, `payoff_chain_rate`, and `cliffhanger_rate`.
- Gate derivation must reuse
  `normalize_quality_metrics_history_item(..., "chapter")` instead of carrying
  another private quality gate table.
- The shared story-repair quality context owner must surface runtime pressure
  fields for character, relationship, foreshadow, organization, and career
  ledgers, including count fields and the first three normalized item texts.
- Runtime-pressure-driven threshold adjustments must be consumed by repair
  guidance and quality gate derivation so high-pressure continuity ledgers can
  raise the relevant weak metric threshold without duplicating the Python
  quality gate table.
- Adaptive preset/style/genre profile semantics from
  `novel_quality_profile_service.py` and `novel_quality_rules.py` must be owned
  by Rust before the active candidate executor is default-enabled. The Rust
  owner must resolve normalized profile tokens, detected style and genre
  profiles, quality presets, stage-aware focus weights, focus labels, summary
  text, and profile-driven weak-threshold adjustments.
- Story-repair guidance and quality-gate payloads must expose
  `adaptive_quality_profile`. Volume goal completion summaries that consume
  runtime context must expose `quality_weight_profile`, `profile_focuses`,
  `style_profile`, `genre_profiles`, and `quality_preset` from the same Rust
  profile owner instead of deriving placeholder profile fields from unrelated
  story/character focus inputs.
- Prompt-block profile semantics from `NovelQualityProfileService.build_profile`
  must be owned by Rust before Rust generation prompts can be considered
  quality-profile complete. The Rust owner must sanitize summary-only external
  assets, produce ignored-asset reasons, render generation/checker/reviser/
  MCP/external-asset prompt blocks, expose the same `prompt_blocks` key family,
  and keep the policy limits visible.
- `chapter_generation_prompt_service.rs` must fill template placeholders
  `quality_generation_block`, `quality_checker_block`, `quality_reviser_block`,
  `quality_mcp_guard_block`, and `quality_external_assets_block` from the Rust
  profile owner instead of leaving only a raw external-assets block.
- `chapter_generation_prompt_service.rs` must assemble the active chapter
  generation `<quality_contract>` block in Rust. That contract must include the
  Rust-owned generation quality block, creative/story focus blocks when present,
  story repair blocks when present, quality preference preset/notes, the
  unified protocol guard, MCP guard, and summary-only external asset block. It
  must be injected after `</fusion_contract>` for chapter generation prompts and
  must not duplicate an existing custom `<quality_contract>` block.
- Quality preference preset/notes handling in Rust must preserve the Python
  source-map semantics for `balanced`, `plot_drive`, `immersive`,
  `emotion_drama`, and `clean_prose`, including Chinese aliases, max four note
  items, de-duplication, and chapter-scene bullet text.
- Gate plan output must keep `action`, `quality_gate`, `quality_metrics`,
  retry budget, scope, and current story-repair payload.
- When runtime continuity ledgers are present, the Rust owner must run the
  `build_story_continuity_preflight(...)` source-map function group and merge
  `continuity_warning_count`, `continuity_preflight`, continuity focus areas,
  and repair targets into the quality gate payload.
- This owner narrows the active Rust route quality gap, but it does not retire
  the Python FastAPI route, Python candidate executor fallback shell, or the
  full Python quality domain.

### 4. Validation & Error Matrix
- Empty / unanchored content -> lower score and non-`allow_save` quality gate.
- Outline and world rules missing -> corresponding metric detail is marked
  `applicable: false` instead of forcing a hard zero into the weighted score.
- Runtime context object present -> copied into metrics so history and gate
  semantics can derive stage/pressure.
- Organization/career/relationship/character continuity ledgers present ->
  `repair_guidance.quality_runtime_pressure` exposes counts and normalized
  item samples.
- Organization/career pressure present -> quality gate failed metrics use the
  adjusted threshold for rule-grounding, conflict, outline, or payoff metrics.
- Preset/style/genre runtime context present -> `adaptive_quality_profile`
  carries resolved stage, quality preset, style profile, genre profiles, focus
  areas, and the nested weight profile.
- Style/genre/preset/intent focus inputs present -> weak-threshold adjustments
  are cumulative with stage and runtime-pressure adjustments.
- Genre/style text with no explicit profile token -> Rust detection resolves
  the same profile keys used by the Python source map.
- External assets containing summaries -> Rust prompt blocks include the
  summary-only asset lines and keep raw provider payloads available separately.
- Duplicate, raw-only, no-summary, or over-limit external assets -> Rust
  profile output includes `ignored_external_assets` with stable ignore reasons.
- Rust chapter generation prompt params -> include non-empty quality profile
  blocks for generation, checker, reviser, MCP guard, and external assets.
- Rust chapter generation prompt params -> include `quality_preference_block`,
  `quality_generation_protocol_block`, `quality_json_protocol_block`, and a
  non-empty `quality_contract_block` when quality inputs exist.
- Rendered Rust chapter generation prompt -> injects `<quality_contract>` after
  `</fusion_contract>` and keeps summary-only external assets inside that
  contract.
- Runtime context ledger item missing from content -> `continuity_preflight`
  has `status: "warning"`, `warning_count`, warning records, focus areas, and
  repair targets.
- Runtime context ledger anchors present in content -> `continuity_preflight`
  has `status: "ok"` and zero warnings.
- `auto_repair` gate with retry budget -> gate plan action is `retry`.
- Retry budget exhausted -> gate plan action remains `continue` while the
  non-pass `quality_gate` stays visible.
- Non-object metrics still follow the generic adapter rule and are dropped
  before this owner receives gate-plan input.

### 5. Good/Base/Bad Cases
- Good: `chapter_generation_runtime_service.rs` wires adapter callbacks to the
  single-generation quality owner functions.
- Base: active route uses Rust metrics/gate, while Python quality domain remains
  a source map for future deeper parity.
- Bad: reintroduce an inline `overall_score: 80.0` or unconditional
  `allow_save` gate in the runtime owner.
- Bad: duplicate the quality gate threshold table instead of reusing the Rust
  story-repair quality context owner.

### 6. Tests Required
- `cargo test chapter_single_generation_candidate_quality_service`.
- `cargo test novel_quality_profile_service`.
- `cargo test chapter_generation_prompt_service`.
- `cargo test chapter_story_repair_quality_context_service`.
- `cargo test chapter_candidate_quality_adapter_service`.
- `cargo test chapter_generation_runtime_service`.
- `cargo test chapter_candidate_executor_runtime_adapter_service`.
- `cargo test chapter_single_generation_active_gateway_smoke_service`.
- `cargo check --manifest-path "backend-rs/Cargo.toml"` with a dedicated
  target dir outside the workspace.

### 7. Wrong vs Correct
#### Wrong
```rust
|input| json!({
    "overall_score": 80.0,
    "word_count": input.content.chars().count(),
})
```

#### Correct
```rust
build_chapter_candidate_quality_adapter(
    context,
    build_single_generation_quality_runtime_context,
    compute_single_generation_story_quality_metrics,
    resolve_single_generation_quality_gate_plan,
)
```

## Scenario: Rust single-generation stream success finalize owner

### 1. Scope / Trigger
- Trigger: a migration package ports or tightens single-chapter stream finalize
  behavior from `backend/app/services/chapter_generation/stream/finalize_service.py`
  into `backend-rs/src/services/chapter_single_generation_stream_workflow_service.rs`.
- Why this needs code-spec depth: stream finalize controls the externally
  observed SSE success sequence, quality gate events, final result payload,
  analysis-started event, and latest quality-history sync. Splitting this back
  into ad hoc workflow logic makes Rust active-route behavior drift from the
  Python source map even when generation content still persists correctly.

### 2. Signatures
- Rust owner file:
  `backend-rs/src/services/chapter_single_generation_stream_workflow_service.rs`.
- Expected owner type:
  `SingleGenerationStreamSuccessArtifacts`.
- Expected follow-up owner:
  `SingleGenerationStreamAnalysisFollowupPlan`.
- Expected emission-step owner:
  `SingleGenerationStreamEmissionStep`.
- Expected owner methods:
  `SingleGenerationStreamSuccessArtifacts::from_generated_result(...)`.
- Expected owner methods:
  `SingleGenerationStreamSuccessArtifacts::from_quality_metrics(...)`.
- Expected owner methods:
  `SingleGenerationStreamSuccessArtifacts::build_success_emission_plan(...)`.
- Expected owner methods:
  `SingleGenerationStreamSuccessArtifacts::emit_success(...)`.

### 3. Contracts
- The stream success owner must build the story-runtime contract before
  follow-up analysis and attach it to quality metrics when missing.
- Latest generated chapter history quality metrics must be updated from the
  same success owner after follow-up analysis returns metrics.
- Quality gate decisions must normalize to the Python stream contract:
  `passed` / `continue` -> `continue`,
  `auto_repair` / `repair` / `retry` -> `retry`,
  `manual_review` -> `continue` because manual review is telemetry-only.
- Completion message must be derived from the normalized action:
  `retry` -> `章节生成完成，已转入质量修复`,
  otherwise -> `章节生成完成`.
- Analysis-started message must be derived from the same normalized action:
  `retry` -> `质量修复分析任务已启动`,
  otherwise -> `章节分析任务已启动`.
- Ordered success payloads must stay:
  `quality_metrics -> quality_gate -> result -> analysis_started`, omitting
  optional events when absent.
- Ordered SSE emission plan must stay:
  `complete -> ordered success payloads -> done`.
- This plan may live inside the same stream workflow owner, but it must not be
  a neighboring file or wrapper that only replays a single call.

### 4. Validation & Error Matrix
- `quality_gate.decision = passed` -> action `continue`, no quality-gate SSE
  event, completion message `章节生成完成`.
- `quality_gate.decision = auto_repair` -> action `retry`, quality-gate SSE
  event type `quality_gate_retry`, progress `88`, and analysis-started message
  `质量修复分析任务已启动`.
- `quality_gate.decision = manual_review` -> action `continue`, no
  quality-gate SSE event, and completion message `章节生成完成`.
- Missing analysis task id -> no analysis-started event.
- Missing quality metrics -> no quality-metrics event, but result and done
  must still be emitted.
- Existing `story_runtime_contract` in metrics -> preserve it; missing
  contract in metrics -> attach the owner-built contract.

### 5. Good/Base/Bad Cases
- Good: `SingleGenerationStreamSuccessArtifacts` owns follow-up plan,
  response payload, ordered payloads, emission plan, and actual SSE emission
  from one active Rust stream owner with focused tests.
- Base: Python `finalize_service.py` remains source map / compatibility code
  while the Rust active stream lane owns the same success finalize contract.
- Bad: rebuild completion messages and quality-gate event payloads in the
  workflow `run(...)` body while the success owner already has the same data.
- Bad: add a separate `*_success_emission_plan_service.rs` file that only
  forwards `success artifacts -> emit`.
- Bad: change SSE event order or result payload fields while migrating the
  finalize owner.

### 6. Tests Required
- Unit tests in `chapter_single_generation_stream_workflow_service.rs` for
  quality gate action normalization.
- Unit tests for terminal success response payload shape including
  `analysis_task_id`, `quality_metrics`, `quality_gate_action`,
  `hard_gate_blocked`, `content_applied`, and `story_runtime_contract`.
- Unit tests for retry/manual-review quality events and analysis-started
  messages.
- Unit tests for ordered success event payloads.
- Unit tests for `build_success_emission_plan(...)` proving
  `complete -> quality_metrics -> quality_gate -> result -> analysis_started
  -> done`.
- `cargo test chapter_single_generation_stream_workflow_service
  --manifest-path "backend-rs/Cargo.toml" --target-dir "<external-target-dir>"
  -- --nocapture`.
- `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir
  "<external-target-dir>"`.

### 7. Wrong vs Correct
#### Wrong
```rust
let _ = tx.send(Ok(tracker.complete(Some("章节生成完成")))).await;
if let Some(metrics) = analysis.quality_metrics_event(result) {
    let _ = tx.send(Ok(sse_json(&metrics))).await;
}
let _ = tx.send(Ok(sse_result(&analysis.response_payload(result)))).await;
let _ = tx.send(Ok(sse_done())).await;
```

#### Correct
```rust
for step in success_artifacts.build_success_emission_plan(result) {
    match step {
        SingleGenerationStreamEmissionStep::Complete(message) => {
            let _ = tx.send(Ok(tracker.complete(Some(&message)))).await;
        }
        SingleGenerationStreamEmissionStep::Payload(payload) => {
            let _ = tx.send(Ok(payload.into_event())).await;
        }
        SingleGenerationStreamEmissionStep::Done => {
            let _ = tx.send(Ok(sse_done())).await;
        }
    }
}
```

## Scenario: Rust single-generation active route candidate gateway owner

### 1. Scope / Trigger
- Trigger: a migration package moves Rust single-chapter stream/background
  generation from direct `AIService.generate_text(...)` execution toward
  consuming the Rust chapter candidate route gateway.
- Why this needs code-spec depth: the active Rust generation route is the first
  route-adjacent consumer that can prove the candidate gateway is no longer
  smoke-only. A careless cutover can change HTTP/SSE payloads, persistence
  history, task lifecycle checkpoints, or rollback behavior.

### 2. Signatures
- Rust route file:
  `backend-rs/src/api/chapter_generation_routes.rs`.
- Runtime owner:
  `backend-rs/src/services/chapter_generation_runtime_service.rs`.
- Stream/background owners:
  `backend-rs/src/services/chapter_single_generation_stream_workflow_service.rs`,
  `backend-rs/src/services/chapter_single_generation_write_workflow_service.rs`,
  and `backend-rs/src/services/chapter_single_generation_runtime_state_service.rs`.
- Expected gateway-aware runtime function:
  `generate_and_persist_chapter_content_with_candidate_route_gateway(...)`.
- Active-route smoke owner:
  `backend-rs/src/services/chapter_single_generation_active_gateway_smoke_service.rs`.
- Active-route smoke endpoint:
  `GET /health/chapter-single-generation-active-gateway-smoke`.

### 3. Contracts
- Route handlers must stay transport-thin: extract `AppConfig`, build the
  candidate route-gateway config through the existing route gateway owner, and
  pass it to stream/background service owners.
- Route-local wrappers around the route-gateway config builder are acceptable
  only when they stay transport-thin and make `AppConfig` -> gateway config
  parity testable inside `chapter_generation_routes.rs`.
- Default behavior must remain direct single-generation fallback while
  `CHAPTER_CANDIDATE_RUST_EXECUTOR_ENABLED` is false.
- When the Rust candidate executor is enabled, active Rust stream/background
  generation must consume `execute_chapter_candidate_route_gateway(...)` before
  persistence.
- Rust executor failure must honor the gateway fallback policy and return to
  the direct single-generation fallback when rollback is enabled.
- Candidate gateway output must be normalized back into the existing
  `GeneratedChapterResult` persistence path so chapter content, history
  payloads, and SSE success projection keep their current shapes.
- `GeneratedChapterResult` must carry candidate gateway observability metadata
  as `candidate_gateway_metadata`. The persisted history payload and stream
  result payload expose it as `candidate_gateway` with:
  `execution_path`, `fallback_applied`, `fallback_reason`,
  `rollback_boundary`, and `rust_error`.
- Later quality-analysis rewrites through
  `update_latest_generated_chapter_history_quality_metrics(...)` must preserve
  the existing `candidate_gateway` object instead of replacing it with a
  quality-only payload.
- Active-route parity smoke must reuse the single-generation candidate request
  builder and candidate/fallback content extractor. It must use fake Rust
  executor and fake direct-generation fallback payloads so the smoke proves the
  route-group boundary without calling a real provider.
- Active-route smoke readiness evidence must expose the route/workflow/runtime
  owner chain, including `chapter_generation_routes`,
  `chapter_single_generation_stream_workflow_service`,
  `chapter_single_generation_write_workflow_service`,
  `chapter_single_generation_runtime_state_service`,
  `chapter_generation_runtime_service`, and the shared candidate executor
  provider/quality/record owners. The single-generation candidate request,
  quality adapter, direct fallback candidate, metadata, and content extraction
  helpers belong to `chapter_generation_runtime_service` when that is their
  only production runtime owner; do not keep or recreate a standalone
  `chapter_single_generation_candidate_gateway_service` helper file just to
  forward runtime-owned gateway details. This prevents a passing smoke from
  being mistaken for route cutover readiness when it only exercised a
  projection helper.
- Active-route smoke readiness evidence must also expose
  `active_route_gateway_config` from `AppConfig -> chapter_generation_routes ->
  stream/write workflow -> runtime lifecycle`, preserving
  `rust_executor_enabled`, `fallback_on_rust_error`, `disabled_reason`,
  and `rollback_boundary`. It must not depend on the retired active-route
  direct-fallback smoke probe for readiness.
- The active-route smoke rollback boundary is
  `legacy_single_generation_direct_ai`, but readiness should prove the Rust
  owner path plus fallback-freeze candidate. The deploy/AppConfig gateway knob,
  not a health probe that executes Python direct fallback, is the rollback
  boundary while Python fallback shells remain frozen source maps.
- Do not report Python FastAPI route retirement solely because the Rust route
  now consumes the gateway. Python route retirement still requires route parity
  and fallback-shell deletion or repoint evidence.

### 4. Validation & Error Matrix
- Gateway disabled -> direct single-generation fallback is selected and the
  generated content still flows through the Rust narrative cleaner.
- Gateway enabled -> candidate executor request carries prompt, temperature,
  max tokens, target word count, source, generation label, max candidates, and
  empty runtime state.
- Gateway payload with `full_content` or `content` -> accepted.
- Gateway payload with blank content -> runtime error
  `candidate route gateway returned empty generated content`.
- Active-route smoke with `rust_executor_enabled=true` -> execution path
  `rust_candidate_executor`, no fallback, content extracted from
  `full_content`, and runtime state keeps `generation_label =
  single_generation_candidate`.
- Active-route fallback-freeze smoke with `rust_executor_enabled=true` and
  `fallback_on_rust_error=false` -> execution path `rust_candidate_executor`,
  no fallback, and readiness reports `python_fallback_removal_ready = true`.
- Active-route smoke readiness evidence -> `owner_scope` is
  `active_route_gateway_stream_background_runtime_terminal`, covered Rust
  owners include route/workflow/runtime/candidate/provider/quality/record plus
  stream/background/terminal projection owners, and `runtime_owner_chain`
  names `create_owned_single_generation_stream`,
  `SingleGenerationBackgroundWriteWorkflowEntry::start_from_route_payload`,
  `SingleGenerationStreamLifecyclePlan::from_runtime_launch_with_gateway_config`,
  `SingleGenerationRuntimeLifecyclePlan::from_runtime_launch_with_gateway_config`,
  and `SingleGenerationRuntimeLaunchInput::execute_generation_with_gateway_config`.
- Active-route smoke health endpoint -> each probe exposes the same
  `readiness_evidence`, so deployment checks can inspect route config
  preservation and rollback metadata without calling the real provider.
- Runtime candidate gateway output -> history payload contains
  `candidate_gateway.execution_path` and `candidate_gateway.rollback_boundary`.
- Follow-up analysis quality-metrics rewrite -> existing
  `candidate_gateway` metadata remains present in generated history.
- Stream success result payload -> exposes the same `candidate_gateway` object
  without changing existing event types or success event order.
- Background runtime completion checkpoints -> expose the same
  `candidate_gateway` object on finalizing/completed snapshots when the
  generated result carries gateway metadata. Quality-gate terminal checkpoints
  must also preserve that object when a generated result exists, while pure
  provider/runtime failures without a generated result must not invent gateway
  metadata.
- Background task read/status projections -> expose an object-shaped snapshot
  `candidate_gateway` at the route-facing task payload top level while keeping
  the same object under `checkpoint.candidate_gateway`. Invalid or non-object
  runtime metadata must remain visible only in the raw checkpoint and must not
  be promoted as route-facing gateway metadata.
- Route/status endpoint evidence -> `GET /chapters/batch-generate/{batch_id}/status`,
  active project batch reads, and active batch task-list reads must all consume
  the same Rust read-context projection for object-shaped `candidate_gateway`.
  The route-facing top-level object must equal `checkpoint.candidate_gateway`,
  and route constants/tests must guard the status endpoint path while the
  Python compatibility shell remains only a rollback/source-map boundary.
- Shared candidate quality owner -> `chapter_candidate_quality_adapter_service.rs`
  owns both quality adapter construction and runtime callback materialization.
  Do not reintroduce a separate bridge module that only forwards
  `build_runtime_quality_adapter_callbacks(...)` or poisoned-lock callback
  handling; runtime/default dependency consumers should reuse this shared owner
  so single, batch, and route-gateway candidate flows cannot drift.
- Shared default record owner -> `chapter_candidate_executor_default_dependency_service.rs`
  owns `ChapterCandidateDefaultRecordBuildInput` and
  `build_default_generation_candidate_record(...)`. Do not reintroduce a
  forwarding-only `chapter_candidate_runtime_record_bridge_service.rs`; runtime
  adapters should call the default dependency owner and let the record owner
  return errors for sanitized-empty or workflow/meta content.
- Active-route readiness evidence must cover both the enabled Rust owner path
  and the fallback-freeze candidate path. It must show stream
  `candidate_gateway`, background create-response contract, terminal
  quality-gate projection, rollback boundary, and must explicitly note that
  background create responses do not attach final `candidate_gateway` metadata
  before runtime completion.
- Stream/background lifecycle tests must remain green because they consume the
  same runtime launch owner.
- Stream/background/runtime lifecycle owners preserve the route-supplied
  `ChapterCandidateRouteGatewayConfig`; they must not silently fall back to
  `default_single_generation_candidate_gateway_config()` after route config is
  already built from `AppConfig`.

### 5. Good/Base/Bad Cases
- Good: both stream and background Rust active routes pass gateway config to
  the runtime owner, focused tests cover request shape and payload extraction,
  the active-route smoke endpoint proves enabled/fallback-freeze behavior
  without a provider call, and `cargo check` passes with only known existing
  warnings.
- Base: active Rust route consumes the gateway behind a rollback knob, while
  Python FastAPI route remains frozen as fallback/source map.
- Bad: add another health or smoke-only gateway consumer and report active-path
  progress without tying it to the single-generation active-route request and
  content-normalization owners.
- Bad: enable Rust candidate executor by default without route parity and
  rollback evidence.
- Bad: persist candidate output through a separate history path that bypasses
  the existing generated-result owner.

### 6. Tests Required
- Unit tests in `chapter_generation_runtime_service.rs` for candidate request
  shape, candidate/fallback content extraction, blank payload rejection,
  candidate gateway history metadata, and metadata preservation during quality
  history rewrites.
- Unit tests in `chapter_single_generation_active_gateway_smoke_service.rs`
  for enabled Rust executor smoke, fallback-freeze smoke, metadata, runtime
  state, content normalization, and route-facing readiness evidence across
  stream/background/terminal owners.
- Focused tests for `chapter_generation_routes`,
  `chapter_single_generation_stream_workflow_service`,
  `chapter_single_generation_write_workflow_service`, and
  `chapter_single_generation_runtime_state_service`; stream workflow tests
  must assert that result payloads expose `candidate_gateway` when the runtime
  result carries it, runtime-state tests must assert checkpoint
  `candidate_gateway` preservation, and stream/runtime lifecycle tests must
  assert explicit gateway config retention.
- Route tests in `chapter_generation_routes` must assert that
  `AppConfig` cutover fields are preserved when building the
  single-generation route gateway config.
- Existing route-gateway, health, and auth public-path smoke tests must remain
  green.
- `python backend/tools/run_strangler_gateway_smoke.py --validate-manifest-only`
  must include the active-route smoke probe when a deployment-visible endpoint
  is added.
- `cargo check --manifest-path "backend-rs/Cargo.toml"` with an explicit
  target dir when build artifacts must stay outside the workspace.

## Scenario: Rust chapter prompt story-runtime block owner

## Scenario: Rust chapter draft/history GenerationHistory owner

### 1. Scope / Trigger
- Trigger: a migration package ports or consolidates
  `backend/app/services/chapter_generation/history_service.py`,
  `chapter_draft_query_service.py`, `chapter_draft_workflow_service.py`, or
  `chapter_analysis_response_service.py` semantics into Rust.
- Why this needs code-spec depth: draft detail, auto-revision apply, chapter
  analysis payloads, and generation history replay all depend on the same
  `GenerationHistory` parsing rules. If checker/reviser parsing or latest-item
  lookup drifts across Rust files, payloads stay shape-compatible while hidden
  behavior diverges.

### 2. Signatures
- Rust owner file:
  `backend-rs/src/services/chapter_draft_history_service.rs`.
- Expected owner functions:
  `parse_reviser_result_from_history(generated_content) -> Option<Value>`.
- Expected owner functions:
  `parse_checker_result_from_history(generated_content) -> Option<Value>`.
- Expected owner functions:
  `load_latest_reviser_history(db, chapter_id, history_id)
  -> Result<Option<(generation_history::Model, Value)>, sea_orm::DbErr>`.
- Expected owner functions:
  `load_recent_generation_histories(db, chapter_id, limit)
  -> Result<Vec<generation_history::Model>, sea_orm::DbErr>`.
- Expected owner fragments:
  `ChapterAnalysisCheckerFragments::from_histories(histories)`.
- Expected production consumers:
  `chapter_analysis_read_context_service.rs`.
- Expected production consumers:
  `chapter_analysis_draft_service.rs`,
  `chapter_analysis_view_query_service.rs`,
  `chapter_draft_apply_service.rs`,
  `chapter_draft_view_payload_service.rs`.

### 3. Contracts
- Reviser parsing must only accept
  `log_type = "chapter_text_reviser_v1"` and must only return object-shaped
  `reviser_result`.
- Checker parsing must only accept
  `log_type = "chapter_text_checker_v1"` and must only return object-shaped
  `checker_result`.
- Explicit `history_id` lookup must preserve chapter ownership filtering and
  must reject blank selector text as if no explicit selector was provided.
- Latest reviser lookup must preserve the current descending `created_at` scan
  with the same 60-item cap until a broader read-context owner intentionally
  changes that bound.
- Checker fragment selection must use the first matching history and must keep
  `checker_created_at = None` when the matched history has no timestamp.
- Once the shared draft/history owner exposes recent-history loading,
  `chapter_analysis_read_context_service.rs` should consume that owner instead
  of reopening a local `generation_history::Entity::find(...)` seam.
- Moving these semantics into Rust is migration progress only when active
  draft/query/apply consumers are rewired to the new owner; a standalone parser
  port with no consumer cutover is not enough.

### 4. Validation & Error Matrix
- Invalid JSON -> parser returns `None`, not an error.
- Wrong `log_type` -> parser returns `None`.
- Matching `log_type` but missing `checker_result` / `reviser_result` object ->
  parser returns `None`.
- Explicit `history_id` found but payload is not a valid reviser history ->
  `load_latest_reviser_history(...)` returns `Ok(None)`.
- No matching reviser history in the latest scan window ->
  `load_latest_reviser_history(...)` returns `Ok(None)`.
- Matching checker history with missing `created_at` ->
  `checker_created_at` stays `None`.

### 5. Good/Base/Bad Cases
- Good: one Rust history owner is shared by analysis view, draft detail, and
  draft apply consumers, with focused tests proving checker/reviser parity.
- Base: the owner is centralized in Rust, while Python draft/history routes
  remain source maps for the next route package.
- Bad: `chapter_analysis_view_query_service.rs` reintroduces ad hoc checker
  JSON parsing after the shared owner exists.
- Bad: `chapter_draft_source_service.rs` and a new owner both keep separate
  copies of latest reviser lookup semantics.
- Bad: route handlers or response adapters parse `GenerationHistory` payloads
  inline.

### 6. Tests Required
- Unit tests in `chapter_draft_history_service.rs` for:
  - valid reviser parse
  - valid checker parse
  - invalid / wrong-log histories
  - first-match checker fragment selection
  - missing-timestamp checker fragment behavior
- Focused tests in `chapter_analysis_view_query_service.rs` proving payload
  shapes remain unchanged after consumer rewiring.
- Focused tests in `chapter_analysis_draft_service.rs` proving route-facing
  request/error ownership remains unchanged after latest-history rewiring.
- `cargo check --manifest-path "backend-rs/Cargo.toml"` with an explicit target
  dir when build artifacts must stay out of the workspace.

### 7. Wrong vs Correct
#### Wrong
```rust
fn parse_checker_result(history: &generation_history::Model) -> Option<Value> {
    serde_json::from_str::<Value>(history.generated_content.as_deref()?).ok()?
        .get("checker_result")
        .cloned()
}
```

#### Correct
```rust
fn parse_checker_result(history: &generation_history::Model) -> Option<Value> {
    parse_checker_result_from_history(history.generated_content.as_deref())
}
```

### 1. Scope / Trigger
- Trigger: a migration package ports creative mode, story focus, plot stage,
  narrative blueprint, or chapter story card prompt behavior from
  `backend/app/services/prompt_service.py` into the active Rust chapter
  generation prompt path.
- Why this needs code-spec depth: these blocks shape the final model prompt.
  Treating them as raw pass-through fields makes Rust appear migrated while
  Python still owns the real story-runtime policy.

### 2. Signatures
- Rust owner file:
  `backend-rs/src/services/chapter_generation_prompt_service.rs`.
- Expected owner helpers:
  `normalize_creative_mode(value) -> Option<&'static str>`.
- Expected owner helpers:
  `normalize_story_focus(value) -> Option<&'static str>`.
- Expected owner helpers:
  `normalize_plot_stage(value) -> Option<&'static str>`.
- Expected owner helpers:
  `build_creative_mode_block(mode) -> String`.
- Expected owner helpers:
  `build_story_focus_block(value) -> String`.
- Expected owner helpers:
  `build_narrative_blueprint_block(creative_mode, story_focus, plot_stage)
  -> String`.
- Expected owner helpers:
  `build_story_objective_card_block(creative_mode, story_focus, plot_stage)
  -> String`.
- Expected owner helpers:
  `build_story_result_card_block(creative_mode, story_focus, plot_stage)
  -> String`.
- Expected owner helpers:
  `build_story_payoff_chain_card_block(creative_mode, story_focus, plot_stage)
  -> String`.
- Expected owner helpers:
  `build_story_rule_grounding_card_block(creative_mode, story_focus,
  plot_stage) -> String`.
- Expected owner helpers:
  `build_story_information_release_card_block(creative_mode, story_focus,
  plot_stage) -> String`.
- Expected owner helpers:
  `build_story_emotion_landing_card_block(creative_mode, story_focus,
  plot_stage) -> String`.
- Expected owner helpers:
  `build_story_action_rendering_card_block(creative_mode, story_focus,
  plot_stage) -> String`.
- Expected owner helpers:
  `build_story_summary_tone_control_card_block(creative_mode, story_focus,
  plot_stage) -> String`.
- Expected owner helpers:
  `build_story_repetition_control_card_block(creative_mode, story_focus,
  plot_stage) -> String`.
- Expected owner helpers:
  `build_story_viewpoint_discipline_card_block(creative_mode, story_focus,
  plot_stage) -> String`.
- Expected owner helpers:
  `build_story_dialogue_advancement_card_block(creative_mode, story_focus,
  plot_stage) -> String`.
- Expected owner helpers:
  `build_story_opening_hook_card_block(creative_mode, story_focus,
  plot_stage) -> String`.
- Expected owner helpers:
  `build_story_execution_checklist_block(creative_mode, story_focus,
  plot_stage) -> String`.
- Expected owner helpers:
  `build_story_scene_anchor_card_block(creative_mode, story_focus,
  plot_stage) -> String`.
- Expected owner helpers:
  `build_story_scene_density_card_block(creative_mode, story_focus,
  plot_stage) -> String`.
- Expected owner helpers:
  `build_story_repetition_risk_block(creative_mode, story_focus,
  plot_stage) -> String`.
- Expected owner helpers:
  `build_story_acceptance_card_block(creative_mode, story_focus,
  plot_stage) -> String`.
- Expected owner helpers:
  `build_story_cliffhanger_card_block(creative_mode, story_focus,
  plot_stage) -> String`.
- Expected owner helpers:
  `build_story_character_arc_card_block(creative_mode, story_focus,
  plot_stage) -> String`.
- Expected production consumer:
  `build_prompt_params_with_provider_payload(...)` must set
  `creative_mode_block`, `story_focus_block`, `narrative_blueprint_block`,
  `story_objective_card_block`, `story_result_card_block`,
  `story_payoff_chain_card_block`, `story_rule_grounding_card_block`,
  `story_information_release_card_block`, `story_emotion_landing_card_block`,
  `story_action_rendering_card_block`, and
  `story_summary_tone_control_card_block`,
  `story_repetition_control_card_block`,
  `story_viewpoint_discipline_card_block`,
  `story_dialogue_advancement_card_block`,
  `story_opening_hook_card_block`,
  `story_execution_checklist_block`,
  `story_scene_anchor_card_block`,
  `story_scene_density_card_block`,
  `story_repetition_risk_block`,
  `story_acceptance_card_block`,
  `story_cliffhanger_card_block`, and
  `story_character_arc_card_block` before `quality_contract_block` is
  assembled.

### 3. Contracts
- Creative mode aliases must include English keys and Chinese labels such as
  `hook`, `钩子`, `钩子优先`, `suspense`, `悬念`, and `悬念拉满`.
- Story focus aliases must include English keys and Chinese labels such as
  `escalate_conflict`, `冲突`, `冲突升级`, `relationship_shift`, `关系`,
  and `关系转折`.
- Plot stage aliases must include `development` / `发展`,
  `climax` / `高潮`, and `ending` / `结局`.
- Unknown or blank values must produce empty blocks, not raw user text wrapped
  in a misleading heading.
- `creative_mode_block` and `story_focus_block` must use Rust-owned labels and
  chapter-scene bullet tables, not `build_optional_instruction_block(...)`.
- `narrative_blueprint_block` must combine creative mode, story focus, and
  plot stage labels into a chapter beat plan with priority beats and a first
  avoid-risk line.
- `story_objective_card_block` must describe chapter target, obstacle, turn,
  and hook, using the same override order as Python: creative mode first,
  story focus second, plot stage last.
- `story_result_card_block` must describe progress, reveal, relationship, and
  fallout, with plot stage allowed to override earlier story-focus text.
- `story_payoff_chain_card_block` must describe seed, payoff, feedback, reader
  reward, optional stage reminder, and avoid line.
- `story_rule_grounding_card_block` must describe rule landing, trigger
  condition, cost/limit, scene manifestation, a chapter hard indicator, optional
  stage reminder, and avoid line.
- `story_information_release_card_block` must describe new information,
  carrier, explanation limit, reader handle, optional stage reminder, and avoid
  line.
- `story_emotion_landing_card_block` must describe trigger point, visible
  reaction, relationship wave, layered shift, optional stage reminder, and
  avoid line.
- `story_action_rendering_card_block` must describe action start, collision
  feedback, visible scene change, lens priority, optional stage reminder, and
  avoid line.
- `story_summary_tone_control_card_block` must describe conclusion restraint,
  replacement path, blank space, sentence control, optional stage reminder, and
  avoid line.
- `story_repetition_control_card_block` must describe repeat target, first hit,
  later handling, merge/delete rule, optional stage reminder, and avoid line.
- `story_viewpoint_discipline_card_block` must describe main camera, visible
  boundary, inner-access rule, switching rule, optional stage reminder, and
  avoid line.
- `story_dialogue_advancement_card_block` must describe dialogue task,
  information gap, voice separation, action support, optional stage reminder,
  and avoid line.
- `story_opening_hook_card_block` must describe first strike, trouble seed,
  unresolved question, chapter hard indicators, optional stage reminder, and
  avoid line.
- `story_execution_checklist_block` must describe scene entrance, core
  collision, reader benefit, ending suspension, optional stage reminder, and
  avoid line.
- `story_scene_anchor_card_block` must describe core scene, viewpoint position,
  environmental pressure, scene exit state, optional stage reminder, and avoid
  line.
- `story_scene_density_card_block` must describe live-scene ratio, action and
  dialogue pressure, compression target, optional stage reminder, and avoid
  line.
- `story_repetition_risk_block` must describe repeated content risk, merge
  strategy, replacement target, optional stage reminder, and avoid line.
- `story_acceptance_card_block` must describe chapter acceptance criteria,
  hard failure signals, reader-visible gains, optional stage reminder, and
  avoid line.
- `story_cliffhanger_card_block` must describe ending suspension, unresolved
  question, next-chapter pull, optional stage reminder, and avoid line.
- `story_character_arc_card_block` must describe character decision pressure,
  visible change, cost, relationship impact, optional stage reminder, and
  avoid line.
- `QUALITY_CONTRACT_BLOCK_ORDER` must include `narrative_blueprint_block`
  after `story_focus_block`, and the story card group after
  `quality_preference_block`. It must also keep the tail scene / acceptance /
  cliffhanger / character card group after `story_repair_diagnostic_block`, so
  final prompt injection carries the migrated story-runtime structure in the
  same order as the Python generation contract.

### 4. Validation & Error Matrix
- `creative_mode = "钩子"` -> block contains
  `【创作模式】当前采用“钩子优先”` and chapter hook bullets.
- `story_focus = "冲突"` -> block contains
  `【结构侧重点】当前优先“冲突升级”` and conflict escalation bullets.
- `plot_stage = "高潮"` with creative/story focus -> blueprint combo includes
  `高潮阶段` and the climax pressure beat.
- Blank/unknown creative/story/stage values -> all three migrated blocks stay
  empty unless another recognized value is present.
- Final `quality_contract_block` -> creative block appears before story focus,
  and story focus appears before narrative blueprint.
- Recognized story runtime inputs -> objective, result, payoff-chain, and
  rule-grounding cards are non-empty and appear in contract order after
  `quality_preference_block`.
- Recognized story runtime inputs -> information-release, emotion-landing,
  action-rendering, and summary-tone-control cards are non-empty and appear in
  contract order immediately after rule-grounding.
- Recognized story runtime inputs -> repetition-control,
  viewpoint-discipline, dialogue-advancement, and opening-hook cards are
  non-empty and appear in contract order immediately after summary-tone
  control.
- `story_focus = "冲突"` and `plot_stage = "高潮"` -> stage-specific climax
  lines override conflicting focus-specific objective/result lines, matching
  Python's final assignment order.
- `plot_stage = "高潮"` -> the four follow-up cards use climax-specific stage
  or avoid lines such as suppressing long setting explanation, keeping emotion
  in the collision, making action visible, and avoiding author-summary prose.
- `plot_stage = "高潮"` -> the control/voice/opening cards use climax-specific
  stage or avoid lines such as reducing repeated recap, avoiding camera
  hopping, keeping dialogue short/sharp, and preserving opening pressure.
- Recognized story runtime inputs -> execution-checklist, scene-anchor,
  scene-density, repetition-risk, acceptance, cliffhanger, and character-arc
  cards are non-empty and appear in contract order immediately after repair
  diagnostics.
- `plot_stage = "高潮"` -> the tail scene / acceptance / cliffhanger /
  character cards use climax-specific stage or avoid lines such as moving the
  chapter quickly into the main collision, increasing live-scene ratio,
  avoiding fake collision, preserving impact aftershock, and forcing the
  character's real bottom line.

### 5. Good/Base/Bad Cases
- Good: Rust prompt params normalize aliases, build rich blocks, inject them
  into `<quality_contract>`, and focused tests cover Chinese aliases plus
  final contract order.
- Base: Python `prompt_service.py` remains as source map / fallback while
  active Rust chapter generation owns the runtime prompt materialization.
- Bad: wrap raw `creative_mode` or `story_focus` with a heading and report
  that as migration progress.
- Bad: port helper constants but forget to include
  `narrative_blueprint_block` in the quality contract order.
- Bad: port objective/result/payoff/rule helpers but leave them outside
  `QUALITY_CONTRACT_BLOCK_ORDER`, causing active prompts to omit them.
- Bad: port information/emotion/action/summary-tone helpers but leave them as
  unconsumed Rust functions outside the active prompt params and quality
  contract.
- Bad: port repetition/viewpoint/dialogue/opening helpers but leave them
  outside `QUALITY_CONTRACT_BLOCK_ORDER`, causing active prompts to keep using
  Python-owned policy.
- Bad: port execution/scene-density/acceptance/cliffhanger/character helpers
  but leave them outside `QUALITY_CONTRACT_BLOCK_ORDER`, causing active prompts
  to omit the migrated tail cards even though Rust code compiles.
- Bad: retire Python prompt fallback solely because the active Rust prompt path
  owns this block group; route parity and fallback deletion are separate
  cutover work.

### 6. Tests Required
- Unit tests in `chapter_generation_prompt_service.rs` proving Chinese aliases
  for creative/story/stage produce the expected rich blocks.
- Unit tests proving project defaults and request overrides both feed the Rust
  story-runtime block owner.
- Unit tests proving `quality_contract_block` orders creative, story focus,
  and narrative blueprint blocks correctly.
- Unit tests proving objective, result, payoff-chain, and rule-grounding cards
  are generated from the same aliases and injected in contract order.
- Unit tests proving information-release, emotion-landing, action-rendering,
  and summary-tone-control cards are generated from the same aliases and
  injected after rule-grounding in contract order.
- Unit tests proving repetition-control, viewpoint-discipline,
  dialogue-advancement, and opening-hook cards are generated from the same
  aliases and injected after summary-tone-control in contract order.
- Unit tests proving execution-checklist, scene-anchor, scene-density,
  repetition-risk, acceptance, cliffhanger, and character-arc cards are
  generated from the same aliases and injected after repair diagnostics in
  contract order.
- Unit tests proving blank/unknown inputs keep story card blocks empty.
- `cargo test chapter_generation_prompt_service --manifest-path
  "backend-rs/Cargo.toml" --target-dir "<external-target-dir>" -- --nocapture`.
- `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir
  "<external-target-dir>"`.
- Manifest validation when route/gateway owner evidence is part of the same
  checkpoint.

### 7. Wrong vs Correct
#### Wrong
```rust
params.insert(
    "creative_mode_block".to_string(),
    build_optional_instruction_block("创作模式", &creative_mode),
);
```

#### Correct
```rust
params.insert(
    "creative_mode_block".to_string(),
    build_creative_mode_block(&creative_mode),
);
params.insert(
    "narrative_blueprint_block".to_string(),
    build_narrative_blueprint_block(&creative_mode, &story_focus, &plot_stage),
);
params.insert(
    "story_objective_card_block".to_string(),
    build_story_objective_card_block(&creative_mode, &story_focus, &plot_stage),
);
params.insert(
    "story_information_release_card_block".to_string(),
    build_story_information_release_card_block(&creative_mode, &story_focus, &plot_stage),
);
params.insert(
    "story_repetition_control_card_block".to_string(),
    build_story_repetition_control_card_block(&creative_mode, &story_focus, &plot_stage),
);
params.insert(
    "story_execution_checklist_block".to_string(),
    build_story_execution_checklist_block(&creative_mode, &story_focus, &plot_stage),
);
```

---

## Change Checklist

- Rust migration owner consolidation rule:
  if a runtime adapter only bridges quality callbacks, default dependency
  wiring, provider stream collection, or record construction for one
  production adapter, collapse it into that production adapter instead of
  preserving a forwarding-only Rust owner. Readiness evidence should name the
  production adapter plus real supporting owners such as provider stream,
  quality adapter, default dependency, record, wiring, and gateway smoke files.
  Do not count a deleted runtime adapter shell as migration progress.
- Candidate executor owner rule:
  the default dependency service must only assemble default Rust callbacks,
  provider/record adapters, and rerank formulas. It must not duplicate the
  candidate executor stage orchestration. Production wiring should call the
  boxed executor workflow owner in `chapter_candidate_executor_service.rs`, so
  generation, word-budget repair, targeted repair, finalize ordering, runtime
  state handoff, and post-finalize targeted repair selection stay in one
  executor owner.
- Candidate generation owner rule:
  default generation dependency construction belongs to
  `chapter_candidate_generation_service.rs`, not to executor default wiring.
  The default dependency service may adapt provider output and record builders
  into generation callbacks, but retry prompt suffix, retry strategy suffix,
  retry temperature, additional-candidate policy, best-candidate selection, and
  generation runtime-state handoff should remain owned by the generation
  service.
- Candidate finalize owner rule:
  default finalize dependency construction belongs to
  `chapter_candidate_finalize_service.rs`, not to executor default wiring. The
  default dependency service may call `build_default_finalize_dependencies(...)`
  while assembling production wiring, but final candidate selection metadata,
  quality-gate normalization, candidate pool summary, and word-budget repair
  promotion preference should remain owned by the finalize service.
- Candidate targeted repair owner rule:
  default targeted final repair dependency construction belongs to
  `chapter_candidate_targeted_final_repair_service.rs`, not to executor default
  wiring. The default dependency service may adapt provider output and record
  builders into targeted callbacks, but suffix construction,
  temperature/max-token/char-limit resolution, keep/adopt/prefer/followup
  policy, runtime-state handoff, and repair seed metadata semantics should
  remain owned by the targeted repair service.
- Candidate word-budget repair owner rule:
  default word-budget repair dependency construction belongs to
  `chapter_candidate_word_budget_repair_service.rs`, not to executor default
  wiring. The default dependency service may adapt provider output and record
  builders into word-budget callbacks, but suffix construction, apply/relax
  policy, temperature/max-token/char-limit resolution, keep/select/prefer
  policy, runtime-state handoff, and repair seed metadata semantics should
  remain owned by the word-budget repair service.
- Route gateway smoke/readiness consolidation rule:
  if a smoke/readiness service file only proves one route gateway owner and has
  no independent transport branch, fallback branch, or deploy contract, move
  the smoke probes, result projection, and readiness evidence into the route
  gateway owner. Keep the public health endpoint path stable and update wiring
  evidence to name the route gateway owner, not the deleted smoke-only file.
- Batch route error mapper consolidation rule:
  if a `chapter_batch_generation` API error mapper is consumed only by
  `chapter_batch_generation.rs`, keep the mapper as a private module inside
  that route owner instead of a standalone file. Do not restore
  `chapter_batch_generation_error_mapper.rs` unless multiple real API owners
  need to share the same mapping boundary; otherwise create/status/stream,
  active task list, cancel, and resume route parity evidence is split across
  fake owners.
- Batch status/cancel task-not-found fallback retirement rule:
  once `chapter_batch_generation.rs` maps status and cancel
  `LoadOwnedBatchGenerationTaskError::TaskNotFound` to
  `404 {"detail": "Batch generation task not found"}` and focused
  `chapter_batch_generation` tests pass, track both task-not-found probes as
  Rust `requires_login` asymmetric probes. Do not restore
  `chapters-batch-status-task-not-found-python-fallback` or
  `chapters-batch-cancel-task-not-found-python-fallback` unless the same
  change rolls back the Rust route owner, its error mapper tests, or the
  authenticated 404 manifest probes.
- Draft route error mapper consolidation rule:
  if a `chapter_draft` API error mapper is consumed only by
  `chapter_draft_routes.rs`, keep the mapper as a private module inside that
  route owner instead of a standalone file. Do not restore
  `chapter_draft_error_mapper.rs` unless multiple real API owners need to
  share the same mapping boundary; otherwise auto-revision, candidate draft
  load/apply, history, readiness, and selection-mode-sensitive not-found
  parity evidence is split across fake owners.
- Chapter analysis query error mapper consolidation rule:
  if a `chapter_analysis` query API error mapper is consumed only by
  `chapter_analysis_routes.rs`, keep the mapper as a private module inside that
  route owner instead of a standalone file. Do not restore
  `chapter_analysis_query_error_mapper.rs` unless multiple real API owners need
  to share the same mapping boundary; otherwise owned analysis view, quality
  metrics, and analysis task status parity evidence is split across fake
  owners.
- Chapter CRUD error mapper consolidation rule:
  if a chapter CRUD API error mapper is consumed only by
  `chapter_crud_routes.rs`, keep the mapper as a private module inside that
  route owner instead of a standalone file. Do not restore
  `chapter_crud_error_mapper.rs` unless multiple real API owners need to share
  the same mapping boundary; otherwise create/list/get/update/delete and
  project-path list parity evidence is split across fake owners.
- Chapter project-list fallback retirement rule:
  once `chapters-project-list-auth-guard-rust` covers the same path as
  `GET /api/chapters/project/test-project-id` and focused `chapter_crud` tests
  pass, do not reintroduce
  `chapters-project-list-auth-guard-python-fallback` unless the same change
  rolls back the Rust project-path route owner or its focused tests. The query
  form `chapters-list-auth-guard-rust` and path form
  `chapters-project-list-auth-guard-rust` are separate route evidence; do not
  use one as rollback proof for the other.
- Memories route error mapper consolidation rule:
  if a memories API error mapper is consumed only by `memories.rs`, keep the
  mapper as a private module inside that route owner instead of a standalone
  file. Do not restore `memories_error_mapper.rs` unless multiple real API
  owners need to share the same mapping boundary; otherwise project memory
  query/write, chapter-analysis payload, and analyze-chapter workflow parity
  evidence is split across fake owners.
- Single-generation candidate gateway file-collapse rule:
  if the candidate gateway helpers only support the shared generation runtime
  owner and active route smoke evidence, collapse request construction,
  active quality-adapter construction, direct fallback candidate payload,
  gateway metadata, and candidate/fallback content extraction into
  `chapter_generation_runtime_service.rs`. Readiness evidence should name
  `chapter_generation_runtime_service` and shared candidate executor owners,
  not the deleted `chapter_single_generation_candidate_gateway_service.rs`.
  Do not count the deleted candidate-gateway helper shell as a Rust target
  file or recreate it as a forwarding-only owner.
- Batch status task-view payload owner rule:
  status task-view payload construction belongs in
  `chapter_batch_generation_task_payload_base_service.rs` with the other batch
  task payload contracts. Read/query services should load tasks and snapshots,
  then call the payload-base owner; runtime-state persistence owners should
  also call payload-base directly instead of importing a read-context helper.
  Do not reintroduce a read-context-only status payload builder when the
  payload shape, terminal fields, quality context, and candidate gateway
  projection are shared by status, runtime cancellation, and route-facing read
  projections.
- Chapter draft route readiness owner rule:
  draft route readiness probes are evidence for
  `chapter_draft_route_service.rs`, not a separate production owner. Keep
  auto-revision/candidate load/apply readiness paths, fallback shell, and
  rollback boundary beside the route-facing draft payload owner. Do not restore
  `chapter_draft_route_readiness_service.rs` unless readiness becomes a real
  shared health/deploy endpoint consumed outside the draft route package.
- Candidate executor wiring readiness owner rule:
  candidate executor wiring readiness is evidence for
  `chapter_candidate_executor_default_dependency_service.rs`, because the
  default dependency owner composes provider, record, quality, rerank,
  generation, repair, finalize, and executor callbacks for the Rust candidate
  package. Do not restore `chapter_candidate_executor_wiring_service.rs` as a
  standalone Rust target unless production code consumes a separate wiring
  service again; keep dependency-graph tests in the default dependency owner.
- Candidate event/progress projection owner rule:
  batch and candidate generation event payload projection belongs to
  `chapter_candidate_event_service.rs`. Status/read-context, batch candidate,
  and single-generation owners should call that Rust owner for progress,
  selected-candidate, chunk, and single-generation progress kwargs payloads
  instead of duplicating Python-era JSON construction locally. This owner ports
  Python `chapter_candidate_event_service.py` and the event-facing subset of
  `chapter_candidate_view_service.py`; count it as migration progress only
  when at least one real Rust route/service consumer uses the projection, and
  count selected-candidate/chunk builders as active-path migration only after a
  production Rust batch candidate emission path consumes them.
- Candidate selected-event batch rule:
  the Python `emit_batch_generation_selected_candidate_events(...)` decision
  boundary maps to
  `build_batch_generation_selected_candidate_event_batch(...)` in
  `chapter_candidate_event_service.rs`. Keep stream-task gating, selected
  progress emission, `stream_chunks`, and quality-gate action based chunk
  suppression together in that Rust owner. Do not re-split this logic across a
  publisher, read-context owner, or batch workflow once the production Rust
  publisher consumes it; the publisher should only fan out the returned event
  batch.
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
- Keeping a compat owner alive only because it contains a small real helper.
  If the real service owner is clear, move that helper back to the real owner
  and delete the compat file with focused tests. When the helper crosses an
  existing reverse dependency, prefer a local lazy import over preserving a
  compatibility file just to avoid the import cycle.
- Retiring a compat owner but leaving route default wiring or test monkeypatch
  surfaces behind. Move those dependencies to the real service owner in the
  same file-level cleanup, then update API tests to patch the real owner so the
  patch still hits the production call path.
- Retiring a route compat owner but keeping the remaining default wiring split
  across another helper file for no reason. If the route module is already the
  only production consumer, collapse those default wiring helpers back into the
  route owner and move stream/access monkeypatch surfaces to that route module
  in the same change.
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
---

## Scenario: Background Task Project Scope Admission

### 1. Scope / Trigger

- Trigger: adding or changing a Rust `background_tasks` task type whose request may
  run without a project.
- Owner: `backend-rs/src/api/background_tasks.rs`.

### 2. Signatures

- `task_type_allows_empty_project(task_type: &str) -> bool` owns the explicit
  global-task allowlist.
- `create_task(...)` must call that helper before accepting an empty
  `project_id`.

### 3. Contracts

- Project-scoped tasks reject an empty `project_id` with the existing bad-request
  response.
- Only task types explicitly returned by
  `task_type_allows_empty_project(...)` may run without a project.
- A new global task type must update the helper and its positive contract test in
  the same change; a new project-scoped task must remain covered by the negative
  contract test.
- Do not infer project scope from payload fields, route origin, or frontend
  behavior.

### 4. Validation & Error Matrix

- Known global task + empty `project_id` -> admitted.
- Known project-scoped task + empty `project_id` -> rejected.
- Any task + non-empty `project_id` -> continue to normal task-type and access
  validation.
- Unknown task type -> project-scope admission does not make it executable; the
  normal unsupported-task validation still applies.

### 5. Good/Base/Bad Cases

- Good: `inspiration_quick_generate` is listed in the helper and covered by the
  global-task test.
- Base: `chapter_regenerate` keeps requiring a project and is covered by the
  project-scoped test.
- Bad: duplicating a string allowlist inside `create_task()` or accepting all
  empty-project tasks because one payload happens to carry enough context.

### 6. Tests Required

- `global_background_task_types_allow_empty_project` asserts every supported
  global task type.
- `project_scoped_background_task_types_require_project` asserts representative
  project-bound generation task types.
- Run `cargo test --manifest-path backend-rs/Cargo.toml
  api::background_tasks::tests` and the full Rust suite after changing the
  allowlist or dispatch table.
- Owner-contract tests must compare the complete semantic entry set; do not use
  array indexes that silently drift when an entry is inserted or reordered.

### 7. Wrong vs Correct

#### Wrong

```rust
if project_id.is_empty() && task_type != "one_global_task" {
    return Err(bad_request());
}
```

#### Correct

```rust
if project_id.is_empty() && !task_type_allows_empty_project(task_type) {
    return Err(bad_request());
}
```

---

## Scenario: Rust Production CI and Real-Backend E2E Ownership

### 1. Scope / Trigger

- Trigger: any change to `backend-rs/**`, backend migration support, frontend
  real-backend smoke coverage, or the owning GitHub Actions workflows.
- The production backend owner is `backend-rs`; CI and E2E must exercise that
  runtime directly instead of using the retired Python FastAPI runtime as a
  substitute.
- Python pytest remains a migration/support regression suite and must be named
  accordingly.

### 2. Signatures

The production Rust CI gate is:

```text
cargo fmt --manifest-path backend-rs/Cargo.toml -- --check
cargo check --locked --manifest-path backend-rs/Cargo.toml
cargo test --locked --manifest-path backend-rs/Cargo.toml
cargo clippy --locked --manifest-path backend-rs/Cargo.toml --all-targets -- -D clippy::correctness -D clippy::suspicious
```

The real-backend E2E startup signatures, executed from `backend-rs/`, are:

```text
cargo run --locked -- migration-executor
cargo build --locked
nohup ./target/debug/mumu-novel-backend
GET http://127.0.0.1:8003/readyz
npm run e2e -- e2e/auth.spec.ts e2e/background-task-pages.spec.ts
```

The migration and successful-run evidence signatures, stored under
`e2e-diagnostics/`, are:

```text
migration-executor.json
migration-executor-stderr.log
migration-executor-exit-code.txt
rust-backend.pid
rust-backend-lifecycle.json
runner-success.json
```

### 3. Contracts

- Rust toolchain: `1.88`, matching `backend-rs/Dockerfile`.
- Database service: PostgreSQL `18-alpine`, matching production Compose.
- Required E2E environment: `DATABASE_URL`, `JWT_SECRET`, `APP_HOST`,
  `APP_PORT`, `ENABLE_STARTUP_SCHEMA_SYNC=false`, and local-auth credentials.
- `STATIC_DIR=../backend/static` assumes the Rust process working directory is
  `backend-rs`; changing the working directory requires changing this path in
  the same patch.
- Migration order is strict: PostgreSQL healthy -> Rust `migration-executor` ->
  Rust `/readyz` confirms DB connectivity and matching migration head -> Playwright.
- `migration-executor` stdout must contain one machine-readable JSON report only.
  Tracing, configuration, and connection diagnostics must be written to stderr.
- The E2E workflow must capture migration stdout, stderr, and the original process
  exit code into separate evidence files, echo both streams to the runner terminal,
  and propagate the original non-zero exit code without starting preflight or server.
- The workflow must build and launch `./target/debug/mumu-novel-backend`
  directly. The PID persisted to `/tmp/rust-backend.pid` and
  `e2e-diagnostics/rust-backend.pid` must therefore identify the Rust server,
  not a `cargo run` wrapper whose child could survive cleanup.
- Cleanup must run after Playwright and before success evidence or artifact upload.
  It records `rust-backend-lifecycle.json`, sends `TERM`, waits up to ten seconds,
  and fails the job after a `KILL` fallback instead of silently accepting a leaked
  or non-graceful server process.
- `runner-success.json` may be created only after migration, release preflight,
  runtime `/readyz`, release `/releasez`, both Playwright smoke specs, and backend
  lifecycle cleanup succeed. It must identify the Rust runtime owner, PostgreSQL
  database, each passed gate including `backend_lifecycle`, `GITHUB_SHA`,
  `GITHUB_RUN_ID`, and `GITHUB_RUN_ATTEMPT`.
- The existing `rust-readiness-diagnostics` artifact name is compatibility-stable.
  Its upload step must use `always()` so successful runner evidence and failure
  diagnostics are both durable; a failed run must never create `runner-success.json`.
- The current incremental Clippy gate blocks `correctness` and `suspicious`
  diagnostics. The remaining historical warnings are tracked debt; do not hide
  them with crate-wide `allow` attributes. Full `-D warnings` is a separate
  cleanup target.

### 4. Validation & Error Matrix

- PostgreSQL health check fails -> the E2E job fails before migration.
- Rust migration executor exits non-zero -> preserve the JSON, stderr, and exit-code
  evidence under `e2e-diagnostics/`, upload it through the existing diagnostics
  artifact path, propagate the original exit code, and do not start preflight or runtime.
- Rust `/readyz` does not return success within the workflow timeout -> print
  `e2e-diagnostics/rust-backend.log` and fail. Database ping success with a missing, empty, or
  mismatched `alembic_version` head must remain not-ready.
- Playwright smoke fails -> do not create `runner-success.json`; upload the
  available Rust diagnostics and Playwright report, print Rust logs, and fail the job.
- Backend PID file is absent -> write lifecycle status `not_started`; preserve the
  earlier failure state without inventing a cleanup failure.
- Backend PID is no longer alive -> write lifecycle status `already_exited`, keep
  the available Rust log, fail cleanup, and do not create `runner-success.json`.
- Backend exits after `TERM` -> write lifecycle status `terminated` with signal `TERM`.
- Backend remains alive for ten seconds after `TERM` -> send `KILL`, write lifecycle
  status `forced_kill`, fail cleanup, and do not create `runner-success.json`.
- All Rust release gates, Playwright smoke, and graceful lifecycle cleanup pass ->
  create a parseable `runner-success.json` bound to the current GitHub SHA/run/attempt,
  then upload the complete `e2e-diagnostics/` directory including lifecycle evidence.
- Rust fmt/check/test or high-confidence Clippy diagnostics fail -> fail the
  production backend CI job.
- Python pytest fails -> fail only the migration/support job; never relabel that
  job as the production runtime owner.

### 5. Good/Base/Bad Cases

- Good: a `backend-rs/**` pull request runs Rust fmt/check/test/Clippy and the
  PostgreSQL-backed Rust E2E smoke; migration evidence remains a parseable JSON report,
  a separate stderr log, and the original exit-code file; the direct Rust server PID and
  lifecycle JSON are also durable, while a successful run publishes `runner-success.json`
  only after graceful cleanup through the compatibility-stable diagnostics artifact.
- Base: a `backend/**` migration-support change still runs Python pytest and the
  shared E2E contract without restoring Python runtime ownership.
- Bad: E2E uses SQLite, `alembic-sqlite.ini`, or
  `python -m uvicorn app.main:app` and passes while the Rust runtime is broken.
- Bad: migration output exists only in the runner terminal, tracing is mixed into the
  JSON report, or `|| true` hides the executor failure and permits later startup steps.
- Bad: diagnostics upload uses `failure()` only, so a green runner has no durable
  evidence, a success manifest is written before Playwright or cleanup finishes, or
  cleanup kills only a `cargo run` wrapper while the Rust server survives.

### 6. Tests Required

- Parse both workflow YAML files and assert they contain no UTF-8 BOM.
- Assert `backend-rs/**` is present in both workflow path filters.
- Assert the E2E workflow contains PostgreSQL, `migration-executor`, Rust server
  startup, `/readyz`, and both Playwright smoke specs; reject a liveness-only
  `/health` wait gate.
- Assert the E2E workflow contains neither `uvicorn` nor
  `alembic-sqlite.ini`.
- Run
  `production_ci_contract_tests::e2e_smoke_preserves_migration_executor_evidence_and_structured_stdout`
  and
  `production_ci_contract_tests::e2e_smoke_persists_successful_runner_evidence_and_always_uploads_diagnostics`.
- Parse the workflow YAML, extract the success-evidence and lifecycle shell blocks,
  validate them with Git Bash `bash -n`, execute success evidence with deterministic
  GitHub metadata, and parse `runner-success.json` with exact field assertions.
- Execute the lifecycle block against a TERM-responsive process, a process that exits
  before cleanup, and a process that ignores TERM; assert `terminated/TERM` returns
  zero, both `already_exited` and `forced_kill/KILL` return non-zero, and all lifecycle
  JSON documents parse exactly.
- Parse the workflow YAML, validate the migration shell block with Git Bash
  `bash -n`, and run an isolated process probe that parses stdout as JSON while
  asserting the process exit code equals the report `exit_code`.
- Run the four Rust CI commands, the frontend production build, and
  `git diff --check`.
- A GitHub runner execution is required before declaring the PostgreSQL/Rust
  E2E rollout fully verified; local static validation is not equivalent.

### 7. Wrong vs Correct

#### Wrong

```yaml
- name: Migrate PostgreSQL with Rust
  working-directory: backend-rs
  run: cargo run --locked -- migration-executor || true

- name: Start backend
  run: python -m uvicorn app.main:app --port 8003
```

#### Correct

```yaml
- name: Migrate PostgreSQL with Rust
  working-directory: backend-rs
  run: |
    mkdir -p ../e2e-diagnostics
    set +e
    cargo run --locked -- migration-executor \
      > ../e2e-diagnostics/migration-executor.json \
      2> ../e2e-diagnostics/migration-executor-stderr.log
    migration_exit_code=$?
    set -e
    printf '%s\n' "$migration_exit_code" \
      > ../e2e-diagnostics/migration-executor-exit-code.txt
    cat ../e2e-diagnostics/migration-executor.json
    cat ../e2e-diagnostics/migration-executor-stderr.log >&2
    if [ "$migration_exit_code" -ne 0 ]; then
      exit "$migration_exit_code"
    fi

- name: Start Rust backend
  working-directory: backend-rs
  run: |
    cargo build --locked
    nohup ./target/debug/mumu-novel-backend \
      > ../e2e-diagnostics/rust-backend.log 2>&1 &
    backend_pid=$!
    printf '%s\n' "$backend_pid" > /tmp/rust-backend.pid
    printf '%s\n' "$backend_pid" > ../e2e-diagnostics/rust-backend.pid

- name: Run auth + background-task smoke against Rust
  run: npm run e2e -- e2e/auth.spec.ts e2e/background-task-pages.spec.ts

- name: Stop Rust backend and record lifecycle
  if: always()
  run: terminate-direct-rust-server-record-lifecycle-and-fail-on-forced-kill

- name: Record successful Rust E2E evidence
  if: success()
  run: write-runner-success-json-after-rust-playwright-and-lifecycle-gates

- name: Upload Rust readiness diagnostics
  if: always()
  uses: actions/upload-artifact@v4
  with:
    name: rust-readiness-diagnostics
    path: e2e-diagnostics/
```

---

## Scenario: Local Gateway System Proxy Isolation

### 1. Scope / Trigger

- Trigger: any Rust AI client or deterministic HTTP mock that targets a loopback or
  developer-local gateway while `reqwest` is built with its default `system-proxy`
  feature.
- This contract prevents Windows or host-level proxy configuration from intercepting
  local OpenAI-compatible, Gemini, or Anthropic probes without changing remote provider
  proxy behavior.

### 2. Signatures

The shared decision owner is:

```rust
pub(super) fn should_bypass_system_proxy(base_url: &str) -> bool
```

AI clients must apply the result while constructing their private `reqwest::Client`:

```rust
let mut client_builder = Client::builder().timeout(timeout);
if super::should_bypass_system_proxy(&normalized_base_url) {
    client_builder = client_builder.no_proxy();
}
let client = client_builder.build()?;
```

### 3. Contracts

- The only bypass hosts are `127.0.0.1`, `localhost`, IPv6 loopback (`::1` or
  `[::1]`), and `host.docker.internal`.
- Host matching is URL-host based; path text, user info, or a remote hostname that merely
  contains one of those strings must not activate the bypass.
- Remote provider URLs retain configured/system proxy behavior.
- OpenAI, Gemini, and Anthropic clients reuse the same helper; do not copy host lists into
  provider-specific modules.
- Readiness probes and the subsequent AI request must agree on local transport isolation.

### 4. Validation & Error Matrix

- Local gateway URL + system proxy configured -> build the client with `.no_proxy()` and
  connect directly to the local listener.
- Remote provider URL + system proxy configured -> preserve normal `reqwest` proxy
  discovery.
- Malformed URL -> return `false`; normal client URL validation remains the error owner.
- Local mock returns 404/500/502 -> diagnostics must report that mock endpoint, status,
  and response rather than a proxy-generated or another test's response.
- Concurrent Settings HTTP mock failure -> treat it as transport isolation evidence;
  do not hide it with global serialization.

### 5. Good/Base/Bad Cases

- Good: Settings HTTP mock tests run with `--test-threads=32` while a Windows system proxy
  is configured and consistently observe their own endpoint/status/body.
- Base: a remote provider continues to use the operator's configured proxy.
- Bad: disable proxies globally for every provider, add a process-wide mutex, or force
  `--test-threads=1` to make local HTTP tests appear deterministic.

### 6. Tests Required

- Unit-test every allowed local host and representative remote/lookalike hosts.
- Run `api::settings::tests::` concurrently with `--test-threads=32`.
- For a transport-flake fix, preserve before/diagnostic/after stress artifacts and record
  the failing iteration, endpoint, status, and response marker.
- Run Rust fmt, the focused proxy-bypass test, the full Settings test module, the full Rust
  suite, and `git diff --check` before accepting the change.

### 7. Wrong vs Correct

#### Wrong

```rust
let client = Client::builder().timeout(timeout).build()?;
// A host system proxy may intercept a loopback test or local gateway request.
```

#### Correct

```rust
let mut client_builder = Client::builder().timeout(timeout);
if should_bypass_system_proxy(&normalized_base_url) {
    client_builder = client_builder.no_proxy();
}
let client = client_builder.build()?;
```

---

## Scenario: Background Task Snapshot Atomic Persistence

### 1. Scope / Trigger

This contract applies whenever `backend-rs/src/tasks/persistence.rs` reads or writes the
process-owned background-task registry snapshot. It covers crash-safe file persistence only;
`task_type` recovery classification and business checkpoint semantics belong to R2 and later
work.

### 2. Signatures

The production entry points remain backward compatible:

```rust
pub async fn load_from_disk(registry: &TaskRegistry)
pub async fn save_to_disk(registry: &TaskRegistry)
pub fn start_periodic_save(registry: TaskRegistry)
```

Testable directory-injected owners remain private to the module:

```rust
async fn load_from_dir(registry: &TaskRegistry, dir: &Path) -> LoadOutcome
async fn save_to_dir(
    registry: &TaskRegistry,
    dir: &Path,
) -> Result<(), SnapshotPersistenceError>
```

### 3. Contracts

- Snapshot schema version remains `1`; existing version-1 primary snapshots must load unchanged.
- All candidates live in the same directory and use fixed roles:
  - `background_tasks.json`: current primary snapshot.
  - `background_tasks.json.bak`: previous validated primary snapshot.
  - `background_tasks.json.tmp`: synced but not yet committed candidate.
  - `<candidate>.corrupt-<timestamp>-<uuid>`: quarantined invalid evidence.
- Every save is serialized by the same process-local Tokio mutex, including periodic and explicit
  saves.
- The write sequence is `serialize -> write_all -> flush -> sync_all(temp) -> validate -> rotate
  primary to backup -> rename temp to primary`.
- Windows compatibility requires two renames. Never depend on renaming a temporary file over an
  existing primary file.
- Loading order is strictly `primary -> backup -> temporary`; the first valid version-1 snapshot
  replaces the registry contents.
- Parent-directory sync is best effort on Unix after commit. Lack of directory-sync support must
  not cause a fallback to direct primary overwrite.
- Logs may include candidate role, path, item count, and error details, but must not include task
  payloads or user content.

### 4. Validation & Error Matrix

| Condition | Required behavior |
|---|---|
| Candidate is missing (`NotFound`) | Continue to the next candidate without treating it as corruption. |
| Candidate read fails for another I/O reason | Log the failure and continue fallback loading. |
| JSON is malformed | Rename the candidate to a unique `.corrupt-*` path, then continue fallback. |
| Snapshot version is not `1` | Treat it as invalid, quarantine it, then continue fallback. |
| Temporary open/write/flush/sync fails | Return the save error and preserve the existing primary and backup. |
| Existing primary is valid | Remove only the stale backup, then rename primary to backup before commit. |
| Existing primary is invalid | Quarantine it and preserve any existing valid backup. |
| Temporary-to-primary commit fails after rotation | If primary is absent and backup exists, attempt backup-to-primary rollback and retain temp as a recovery candidate. |
| No valid candidate exists | Start with an empty registry and emit a diagnostic log. |

### 5. Good / Base / Bad Cases

**Good case:** a second successful save leaves the new snapshot in the primary file and the
previous valid snapshot in the backup file; both parse as version `1`.

**Base case:** the first save creates a synced temporary file and commits it to the unchanged
production filename without requiring a backup.

**Bad cases:** malformed JSON, unsupported versions, interrupted commits, and a directory placed
at the temporary-file path must not destroy the last valid primary/backup candidate.

### 6. Tests Required

The persistence module must test these assertion points with an injected temporary directory:

1. First save creates a parseable primary snapshot.
2. Second save preserves the previous primary as backup.
3. Corrupt primary is quarantined and backup is loaded.
4. Missing primary falls back to backup.
5. Valid temporary snapshot is the last-resort fallback.
6. Unsupported version is quarantined before backup fallback.
7. Temporary-file open failure leaves the existing primary unchanged.
8. Concurrent saves leave primary and backup parseable and no temporary candidate behind.
9. The production primary filename remains `background_tasks.json`.

Required gates are targeted persistence tests, `cargo fmt --check`, locked `cargo check`, the full
locked Rust test suite, and Clippy with `correctness` plus `suspicious` denied.

### 7. Wrong vs Correct

Wrong — truncates the only valid snapshot before a durable replacement exists:

```rust
tokio::fs::write(primary_path, serialized).await?;
```

Correct — preserve a durable candidate and commit without overwrite-style rename:

```text
write_all(temp) -> flush(temp) -> sync_all(temp)
-> rename(valid primary, backup)
-> rename(temp, primary)
-> best-effort sync(parent directory)
```


---

## Scenario: Background Task Recovery Policy Registry

### 1. Scope / Trigger

This contract applies whenever a production Rust background task type is added, renamed, or changes
its restart/checkpoint semantics, and whenever startup orphan recovery changes. The only generic
startup recovery owner is `backend-rs/src/tasks/recovery.rs`. Business resume commands for chapter
batch/single generation remain owned by their existing database runtime-state services.

### 2. Signatures

The recovery registry and startup owner are:

```rust
pub enum TaskRecoveryPolicy {
    Restartable,
    CheckpointResumable,
    ManualConfirmation,
    NonResumable,
}

pub const TASK_RECOVERY_POLICIES: &[TaskRecoveryPolicyEntry];
pub fn has_explicit_recovery_policy(task_type: &str) -> bool;
pub fn recovery_policy_for(task_type: &str) -> TaskRecoveryPolicy;
pub async fn recover_orphan_tasks(registry: &TaskRegistry) -> usize;
```

`TaskRecord::new()` keeps its existing signature. Recovery metadata is optional and backward
compatible:

```rust
pub terminal_reason: Option<String>;
pub terminal_label: Option<String>;
pub review_required: Option<bool>;
pub can_resume: Option<bool>;
```

### 3. Contracts

- The registry contains exactly 24 known production task types: 5 restartable, 2
  checkpoint-resumable, 16 manual-confirmation, and 1 explicit non-resumable entry
  (`novel_autopilot`).
- `has_explicit_recovery_policy()` distinguishes an intentional registry decision from the safe
  fallback. Unknown or unregistered task types always return `false` there and still resolve to
  `NonResumable` through `recovery_policy_for()`.
- Startup recovery only mutates `pending` and `running` records. Existing completed, failed, and
  cancelled records remain byte-for-byte semantically unchanged.
- Every recovered record remains `failed` for API compatibility and sets all four recovery metadata
  fields explicitly.
- `CheckpointResumable` sets `can_resume=true` only when checkpoint is a non-empty JSON object.
- Recovery preserves result, progress, `started_at` exactly (including `None`), and custom fields
  from an object checkpoint. Only the normal pending-to-running lifecycle owner may initialize a
  missing `started_at`; startup recovery must not fabricate an execution start time. A non-object
  checkpoint is replaced by a structured diagnostic object.
- Recovery explicitly refreshes `updated_at`; `TaskRegistry::update()` does not do this implicitly.
  One recovery projection must sample a single timestamp and reuse it for checkpoint `updated_at`,
  record `completed_at`, and record `updated_at`; independently sampled times for the same projection
  are forbidden because they weaken audit ordering.
- Generic startup recovery never replays a payload and never calls a business resume command.
- `recover_orphan_tasks()` returns the number of active records projected during that invocation; an
  empty registry or a registry containing only terminal records returns `0`.
- Startup ordering is `load snapshot -> recover orphans -> conditionally save when count > 0 -> start
  periodic save/cleanup -> build router`. The recovered projection must reach the existing atomic
  snapshot owner before the process begins serving requests.
- The immediate save keeps the existing best-effort `save_to_disk()` error contract: persistence
  failures are logged and startup continues. Making recovery persistence fail-closed requires a
  separate availability and operations design.
- Logs may contain task id, task type, policy, and projected status only; do not log payload, result,
  checkpoint bodies, prompts, or other user content.

### 4. Validation & Error Matrix

| Input | Required projection |
|---|---|
| Known restartable active task | `failed`, `restart_required`, no resume, no review |
| Chapter checkpoint task with non-empty object | `failed`, `resume_available`, resume allowed |
| Chapter checkpoint task with missing/null/scalar/array/empty object | `failed`, `checkpoint_missing`, no resume |
| Known manual-confirmation active task | `failed`, `manual_review`, review required |
| Unknown active task | `failed`, `non_resumable`, no resume, no review |
| Existing terminal task | No mutation; return count excludes it. |
| Active task with `started_at=None` | Preserve `None`; recovery sets only terminal/update timestamps. |
| Recovered active task | Checkpoint `updated_at`, record `completed_at`, and record `updated_at` represent the exact same instant. |
| No active records recovered | Return `0`; do not perform an immediate startup save. |
| One or more active records recovered | Return the exact count and call `save_to_disk()` before periodic workers and router construction. |
| Immediate snapshot save fails | Log through the existing persistence owner and continue startup; do not silently switch to direct overwrite. |

Adding a new task type without a registry decision is a review failure even though runtime fallback
remains safe. The source-contract test must derive generic executable task types from the real
`execute_task()` match arms and assert that every literal has an explicit registry entry; a second
hand-maintained executable-type list is not sufficient drift protection. Never classify a mutating or
partially persisted operation as restartable without an idempotency proof.

### 5. Good/Base/Bad Cases

- Good: a chapter generation orphan with a usable checkpoint is marked failed with
  `resume_available`; the existing chapter resume UI may offer its owner-specific command.
- Base: a stateless analysis orphan is marked `restart_required`; the user recreates it from the
  original business page because no payload is persisted.
- Bad: startup deserializes a generic task and automatically reruns it, an unknown task receives
  `can_resume=true`, or recovered records remain memory-only until the first periodic save tick.

### 6. Tests Required

1. Assert registry length is 23, entries are unique, and policy counts are 5/2/16.
2. Assert unknown and future-looking task types use `NonResumable`.
3. Cover all four policy projections and all checkpoint JSON shapes.
4. Cover pending/running recovery and terminal-record immutability.
5. Assert result, progress, object-checkpoint custom fields, and `started_at` are preserved exactly:
   existing values remain unchanged and missing values remain `None`.
6. Parse checkpoint `updated_at` and assert it equals both record `updated_at` and `completed_at` for the
   same recovery projection.
7. Assert version-1 JSON without recovery fields deserializes with all four values as `None`.
8. Assert `/background-tasks` compatible payloads expose non-empty recovery fields at both top level
   and under `data` without changing the success wrapper.
9. Assert recovery returns `0` for no active records and the exact number of pending/running records.
10. Keep a startup source-contract test proving the ordered sequence
   `load -> recover -> conditional save -> periodic workers -> router` in
   `production_ci_contract_tests.rs`.
11. Parse the real generic `execute_task()` top-level string match arms, assert the task types are
    unique, and require `has_explicit_recovery_policy()` for every executable literal.
12. Run Rust fmt/check/test, Clippy correctness+suspicious, the frontend production build, UTF-8 BOM
    checks, and `git diff --check`.

### 7. Wrong vs Correct

Wrong — missing payload makes generic automatic replay incomplete and unsafe:

```rust
if record.status.is_active() {
    rerun_task(record.task_type).await;
}
```

Correct — classify and project an actionable terminal state while leaving execution to the existing
business owner, then persist any startup recovery before periodic workers and router construction:

```rust
let recovered_count = recover_orphan_tasks(&task_registry).await;
if recovered_count > 0 {
    persistence::save_to_disk(&task_registry).await;
}
persistence::start_periodic_save(task_registry.clone());

// recover_orphan_tasks() owns policy projection only; it never replays business payloads.
```


---

## Scenario: Versioned Business Checkpoints in Batch Runtime State

### 1. Scope / Trigger

This contract applies when a database-backed batch workflow records a durable business boundary or
uses that boundary to resume execution. The first supported owner is batch chapter generation at
`chapter_draft_saved`. This checkpoint is separate from the process-owned background-task recovery
checkpoint and must reuse the existing batch task, generation contract, and runtime snapshot owners.

### 2. Signatures

The typed owner remains internal to the Rust service layer:

```rust
pub const BUSINESS_CHECKPOINT_SCHEMA_VERSION: &str = "business-checkpoint/v1";

pub(crate) enum BusinessCheckpointBoundary {
    ChapterDraftSaved,
}

pub(crate) enum BusinessCheckpointOutputReferenceV1 {
    Chapter { id: String },
}

pub(crate) struct BusinessCheckpointV1 {
    pub(crate) schema_version: String,
    pub(crate) boundary: BusinessCheckpointBoundary,
    pub(crate) revision: u64,
    pub(crate) idempotency_key: String,
    pub(crate) input_digest: String,
    pub(crate) output_reference: BusinessCheckpointOutputReferenceV1,
    pub(crate) recorded_at: String,
}

pub(crate) fn build_business_checkpoint(...) -> Result<BusinessCheckpointV1, BusinessCheckpointError>;
pub(crate) fn read_business_checkpoint_runtime_state(&Value) -> BusinessCheckpointRead;
pub(crate) fn merge_business_checkpoint_runtime_state(
    &mut Value,
    &BusinessCheckpointV1,
) -> Result<(), BusinessCheckpointError>;
pub(crate) fn validate_business_checkpoint_idempotency_key(
    batch_task_id: &str,
    checkpoint: &BusinessCheckpointV1,
) -> Result<(), BusinessCheckpointError>;
```

The persisted location is additive and fixed:

```text
batch_generation_snapshots.workflow_runtime_state.business_checkpoint
```

### 3. Contracts

- The only current schema is `business-checkpoint/v1`; the only current boundary is
  `chapter_draft_saved`, and its output reference is exactly `{ "kind": "chapter", "id": "..." }`.
- `revision` is positive and monotonic within one batch task. Chapter success uses the greater of the
  persisted successful-chapter count and the previous valid business-checkpoint revision.
- `idempotency_key` is a `sha256:` digest of canonical allowlisted fields: schema version, batch task
  id, boundary, revision, R4 `input_digest`, and typed output reference. Resume must recompute it from
  persisted fields; validating only its prefix is insufficient.
- `input_digest` reuses the validated R4 generation-contract snapshot. Do not derive a second input
  identity or accept a checkpoint whose digest differs from the current persisted contract.
- Persist only the typed allowlist. Prompts, chapter bodies, API keys, authorization headers, provider
  payloads, and complete URLs must never enter the business-checkpoint subtree or safe domain errors.
- Merge under `workflow_runtime_state.business_checkpoint`. Do not replace the existing runtime
  `checkpoint`, create another task store, create another project-state fact, or add a migration for
  this JSON-only extension.
- A missing business checkpoint is legacy-compatible. An unsupported, invalid, tampered, mismatched,
  dangling, cross-project, or empty-output checkpoint fails before task reset and runtime dispatch.
- Resume validation must prove that the chapter id belongs to the batch selection, exists in the
  current project, and has non-empty trimmed content. Valid recovery continues from the next
  incomplete chapter while preserving unrelated runtime-state fields.
- If a legacy success snapshot has no valid R4 generation contract, preserve the old success
  persistence behavior and skip business-checkpoint creation rather than fabricating an input digest.

### 4. Validation & Error Matrix

| Persisted state | Required behavior |
|---|---|
| `business_checkpoint` missing | Continue through the legacy resume path. |
| Unknown `schema_version` | Return a typed safe resume-domain error; do not reset or dispatch. |
| Invalid fields, zero revision, malformed digest, or malformed output | Return a typed safe error; preserve task and snapshot. |
| Canonical idempotency key does not match persisted fields | Reject as tampered; do not expose expected/actual digests to the API caller. |
| R4 `input_digest` differs | Reject as stale input before runtime launch. |
| Chapter id is outside the task selection | Reject before task reset and runtime dispatch. |
| Chapter is missing or belongs to another project | Reject the dangling/cross-project reference. |
| Chapter content is empty after trimming | Reject because the business boundary is not durable. |
| Valid chapter output and matching input contract | Preserve the checkpoint and resume from the next incomplete chapter. |
| Chapter success without a valid generation contract | Persist the legacy success snapshot without adding a business checkpoint. |

### 5. Good / Base / Bad Cases

- Good: chapter 1 content is saved, the production success persistence owner writes revision `1`, a
  later chapter fails, and resume validates the stored chapter before continuing from chapter 2.
- Base: an old snapshot has no `business_checkpoint`; existing resume behavior remains unchanged.
- Bad: a caller supplies an arbitrary `sha256:` idempotency key, resume trusts a chapter id without
  project/content validation, or checkpoint serialization copies the full runtime/provider payload.

### 6. Tests Required

1. Round-trip the exact typed allowlist and reject unknown schema, invalid revision, malformed digest,
   invalid output reference, and invalid timestamp values.
2. Prove canonical idempotency stability and identity sensitivity, then reject a persisted tampered key.
3. Prove runtime-state merge preserves unrelated fields and the legacy `checkpoint` projection.
4. Prove chapter-success persistence uses the R4 digest and never decreases revision across retry/resume.
5. Run a real SQLite/SeaORM success -> checkpoint persistence -> failure -> resume test through the
   production persistence and resume owners; do not create the valid checkpoint directly in the test.
6. Cover missing legacy state, unsupported/invalid checkpoint, digest mismatch, chapter outside task,
   missing chapter, cross-project chapter, and empty chapter content.
7. Assert validation failures leave the task/snapshot unchanged and do not start the runtime.
8. Assert the serialized checkpoint excludes prompt, body, credentials, authorization, and complete URL
   markers; public resume errors must remain fixed safe text.
9. Run the `business_checkpoint`, resume-command, runtime-state, and full Rust test suites plus fmt/check.

### 7. Wrong vs Correct

Wrong — a syntactically shaped digest and output id are trusted without recomputing identity or
validating the durable business result:

```rust
if checkpoint.idempotency_key.starts_with("sha256:") {
    reset_task_and_resume(checkpoint.output_reference.id()).await?;
}
```

Correct — validate the typed checkpoint, canonical identity, R4 input identity, and database-backed
output before any task mutation or runtime dispatch:

```rust
validate_business_checkpoint_idempotency_key(&batch_task_id, checkpoint)?;
ensure_input_digest_matches_generation_contract(checkpoint, persisted_contract)?;
validate_saved_chapter_output(db, project_id, task_chapter_ids, checkpoint).await?;
reset_task_and_resume_from_next_incomplete_chapter().await?;
```


---

## Scenario: Background Task Runtime Terminal Monotonicity

### 1. Scope / Trigger

This contract applies whenever generic Rust background-task lifecycle owners, executor spawn logic,
channel/stream adapters, cancellation, or `TaskRegistry` conditional mutation changes. It prevents
terminal-state rollback, stale executor overwrite, and check-then-update races.

### 2. Signatures

```rust
pub async fn update_if<P, F>(
    &self,
    task_id: &str,
    predicate: P,
    updater: F,
) -> Option<TaskRecord>
where
    P: FnOnce(&TaskRecord) -> bool,
    F: FnOnce(&mut TaskRecord);
```

Lifecycle owners keep these internal contracts:

```text
mark_task_running(task_id): Pending -> Running, returns execution admission
complete_task(task_id): active -> Completed
fail_task(task_id): active -> Failed
cancel_active_task(task_id, user_id): owned active -> Cancelled
```

Task-stream owners keep these async contracts:

```text
TaskStreamHub::subscribe(task_id): atomically reuse or create one broadcast channel
TaskStreamHub::fanout(task_id, event): wait for sender-map access and deliver when subscribed
TaskStreamHub::fanout_terminal(task_id, event): remove the sender atomically, then deliver the final event
subscribe_task_with_latest_snapshot(...): subscribe first, then refresh the connected snapshot
next_task_stream_data(state): on lag, resubscribe at the channel tail and emit the latest connected snapshot
```

### 3. Contracts

- `TaskRegistry::update_if()` is the atomic owner for state transitions requiring a predicate. The
  predicate and updater execute under the same registry write lock. Startup orphan recovery is also a
  lifecycle projection owner and must use `update_if()` with an active-record predicate.
- Do not implement lifecycle transitions as `get()` validation followed by unconditional `update()`, or
  as ordinary `update()` with an early return inside the updater closure. The former races across locks;
  the latter bypasses the shared predicate-owner contract and obscures stale-candidate outcomes.
- `Pending -> Running` must return an admission result. A delayed spawn that loses admission exits
  before executing business logic.
- `Completed`, `Failed`, and `Cancelled` are irreversible terminal states. Completion, failure,
  cancellation, and channel progress may mutate only active records.
- Lifecycle stream fanout must be derived from the record returned by a successful atomic transition.
  When `update_if()` rejects a stale or terminal lifecycle event, the owner must not emit a progress,
  result, done, error, or cancellation event for that rejected transition.
- Concurrent first subscribers for the same task must share one broadcast channel. Channel lookup and
  first sender creation must execute under one sender-map write lock; do not split them into read/drop/write
  phases that can overwrite a channel and disconnect an earlier receiver.
- `TaskStreamHub::fanout()` must await sender-map read access. Lock contention is not permission to drop an
  event with `try_read()`; clone the sender under the read guard, release the guard, then serialize and send.
- The SSE route must establish its subscription before refreshing the connected snapshot. A lifecycle
  transition between authorization and subscription must be represented either by the refreshed connected
  event or by the queued broadcast stream, never by neither.
- Final `done`, `error`, and `cancelled` events must use `fanout_terminal()`. Removing the sender and
  obtaining the sender used for delivery are one write-lock operation: existing receivers retain the
  channel long enough to consume the buffered terminal event, while a later subscriber creates a fresh
  channel and obtains the terminal truth from the connected snapshot. Do not retain terminal senders for
  the process lifetime.
- An SSE broadcast lag must never be ignored and must not be hidden by only increasing channel capacity.
  Call `Receiver::resubscribe()` on the existing channel before reading `TaskRegistry`, then emit the latest
  state through the existing `connected` event. This drops every retained pre-snapshot event and preserves
  subscribe-then-snapshot ordering without creating a new sender-map entry.
- After lag recovery, an active snapshot continues from events sent after `resubscribe()`. A terminal snapshot
  is emitted once and then closes the stream so stale buffered progress cannot regress the client and a
  terminal task cannot recreate a resident sender. If TTL cleanup removed the record, do not fabricate a new
  protocol event; continue until the existing channel closes or receives a later event.
- `complete_task()` is the only generic executor owner of the final `Completed` projection. A
  channel/stream adapter may copy active progress, message, or result, but channel `success` must not
  set `TaskStatus::Completed` before the executor owner records result and terminal timestamps.
- A cancellation projection samples one timestamp and reuses it for checkpoint `updated_at`, record
  `completed_at`, and record `updated_at`.
- Recovered terminal metadata (`recovery_policy`, `recovery_action`, `can_resume`, `review_required`)
  remains immutable under late executor or channel updates.
- Production task creation uses a new UUID and `TaskRecord::new()`; dedup may return an existing active
  task only. Do not invent terminal-record reactivation without a separately approved lifecycle design.

### 4. Validation & Error Matrix

| Current state / event | Required result |
|---|---|
| `Pending` + executor admission | Atomically become `Running`; return admitted. |
| `Pending` + concurrent executor admission/cancel | Serialize both transitions under `update_if()`; cancellation owns the final terminal state, and the task cannot be reactivated. |
| `Cancelled` + delayed executor admission | No mutation; return not admitted; do not execute business logic. |
| Active + executor success/failure | Atomically become `Completed`/`Failed` with one fact timestamp. |
| Any terminal + late success/failure | No mutation or stream fanout; preserve status, result, timestamps, and recovery metadata. |
| Owned active + cancel | Atomically checkpoint and become `Cancelled`. |
| Completed between cancel validation and mutation | Cancellation predicate fails; never overwrite `Completed`. |
| Active + channel progress/message/result | Update active projection only. |
| Channel `success` | Preserve active lifecycle status; final `Completed` belongs to `complete_task()`. |
| Terminal + late channel update | No mutation; the bridge stops. |
| Two concurrent first stream subscriptions | Reuse one sender; both receivers remain connected and receive later fanout. |
| Fanout while sender-map write lock is held | Await the lock, then deliver; never silently discard because of contention. |
| Terminal transition after authorization but before subscription | Connected snapshot refreshes to the latest terminal record, or the subscribed receiver queues the transition. |
| Final `done`/`error`/`cancelled` fanout | Existing receivers receive the final event; sender-map entry is removed; later subscription creates a fresh channel. |

A failed predicate is an expected stale-event outcome, not permission to retry with an unconditional
write. Endpoint-level not-found/ownership responses remain owned by the existing API contract.

### 5. Good/Base/Bad Cases

- Good: executor admission and cancellation race from `Pending`; `update_if()` serializes both owners.
  Cancellation succeeds in either legal lock order, the final record is `Cancelled`, and later admission
  fails. If admission won first, `started_at` may remain as execution history.
- Good: a pending task is cancelled before its delayed spawn runs; admission fails and no business
  executor starts.
- Base: an active channel reports success and supplies a result; the record remains active until
  `complete_task()` commits the final result and terminal timestamps.
- Good: two callers subscribe to an unseen task concurrently; both receivers share the same sender and
  receive the next event.
- Good: fanout waits behind a temporary sender-map write lock and delivers after the guard is released.
- Base: a task completes after authorization but before stream subscription; the post-subscription refresh
  emits a `completed` connected snapshot with progress 100.
- Bad: cancellation performs `get()` and later `update()`, a channel adapter directly writes
  `Completed`, a late executor changes `Cancelled`/recovered `Failed` into another terminal state,
  sender creation uses a read/drop/write sequence, fanout uses `try_read()`, or the route snapshots before
  it subscribes.

### 6. Tests Required

1. Test the `update_if()` primitive directly: missing task executes neither callback; rejected predicate
   skips the updater and preserves the record; accepted predicate executes each callback once and returns
   the latest record; two concurrent `Pending` admissions produce exactly one successful transition.
2. Run `mark_task_running()` and `cancel_active_task()` concurrently from `Pending`; assert cancel
   succeeds, the final record is `Cancelled`, and later admission fails. If admission won first,
   `started_at` may be present, but the terminal state must remain `Cancelled`.
3. Cancel a pending record, subscribe to its task stream, then call running/completion/failure
   owners; assert status remains `Cancelled`, ordinary cancellation recovery fields remain `None`, and
   the receiver observes no event from the rejected lifecycle transitions.
4. Seed a recovered `Failed` record, deliver late running/completion/failure events, and assert all
   recovery fields, result, checkpoint, and timestamps remain unchanged.
5. Feed channel `success`; assert it may update active data but not status, then assert
   `complete_task()` owns final result and `completed_at`.
6. Deliver late channel progress/message to `Cancelled`; assert no mutation and bridge termination.
7. Assert cancellation checkpoint `updated_at` equals record `completed_at` and `updated_at`.
8. Exercise startup orphan recovery across active and terminal candidates; assert terminal candidates are
   unchanged and repeated recovery is idempotent under the same `update_if()` owner contract. Keep a
   production source contract that scopes to `recover_orphan_task()` and rejects ordinary `update()` or
   closure-external recovery metadata while requiring `update_if()` plus the active predicate.
9. Hold the sender-map write lock while fanout starts; assert fanout waits and the receiver obtains the
   event after lock release. Start two first subscriptions concurrently; assert both receive one later event.
10. Complete a task between the authorization snapshot and subscription helper call; assert the refreshed
    connected event reports `completed` with progress 100 while the newly created receiver remains empty.
11. Send a terminal event through `fanout_terminal()`; assert the existing receiver consumes the event and
    then observes channel closure, the sender-map entry is gone, and a later subscriber receives events on
    a fresh channel.
12. Overflow a small broadcast channel, then assert lag recovery emits the latest active registry snapshot,
    discards retained stale progress, receives a later fresh event, emits the latest terminal snapshot after
    a second lag, and closes without replaying stale buffered events.
13. Run `api::background_tasks::tests`, `tasks::stream::tests`, `tasks::registry::tests`,
    `tasks::recovery::tests`, production CI contract tests, full locked Rust tests, fmt/check, Clippy
    correctness+suspicious, UTF-8/BOM checks, and `git diff --check`.

### 7. Wrong vs Correct

Wrong — validation and mutation use different locks, allowing terminal overwrite:

```rust
if registry.get(task_id).await.is_some_and(|task| task.status.is_active()) {
    registry.update(task_id, |task| task.status = TaskStatus::Cancelled).await;
}
```

Correct — predicate and transition share one write lock, and stale events become no-ops:

```rust
registry
    .update_if(
        task_id,
        |task| task.status.is_active(),
        |task| {
            task.status = TaskStatus::Cancelled;
            task.completed_at = Some(now);
            task.updated_at = now;
        },
    )
    .await;
```


## PostgreSQL Password Verifier Storage Readiness Contract

Rust `/readyz` must treat local-auth verifier storage capacity as a production readiness contract, not
as an implicit consequence of the migration revision head.

- The password hashing service is the owner of the canonical Argon2 PHC shape and required storage length.
  Readiness code must reuse that owner contract instead of duplicating a magic number.
- PostgreSQL checks must be read-only and query `information_schema.columns` for
  `user_passwords.password_hash`; readiness must not mutate or auto-repair Schema.
- `TEXT`, unbounded `VARCHAR`, and bounded character storage whose capacity is at least the canonical
  verifier length may allow readiness. Only the separately approved target type may mark the final R0.1
  storage contract complete.
- Missing columns, unsupported types, insufficient bounded capacity, metadata query failures, and an
  unavailable database must fail closed with `503 not_ready` and structured diagnostics.
- Non-PostgreSQL test databases must report an explicit not-applicable state. They must not be presented
  as evidence that the PostgreSQL Auth Schema is compatible.
- A unit-tested metadata classifier is necessary but not sufficient. Before R0.2 is accepted, run the
  real PostgreSQL query/decoding path and prove that legacy `VARCHAR(64)` is rejected before auth writes.
- This readiness check is not permission to change Schema and does not complete R0.1, R0.2, R0.3, or G0.
- CI readiness polling must retain the last response body and HTTP status. Redirecting `/readyz` to
  `/dev/null` is forbidden because it removes the structured reason for migration or Auth Schema failure.
- On runner failure, upload the retained `/readyz` payload together with the Rust backend log as a dedicated
  diagnostic artifact. A Playwright report alone is insufficient when failure happens before Playwright starts.
- Runtime readiness and release-gate completion are distinct contracts. `/readyz` owns runtime readiness;
  `/releasez` owns the production release contract and must require `matches_target_storage_contract=true`.
  HTTP 200 from `/readyz` alone must not allow a merely compatible bounded column to satisfy R0.1/G0.
- Production E2E and local R0.2 tooling must call the Rust-owned `/releasez` endpoint instead of duplicating
  readiness JSON field rules in Workflow, Node, PowerShell, or shell code. This keeps the final target decision
  in the Rust service that owns the metadata contract.
- `/releasez` must fail closed for unavailable databases, migration-head mismatch, incompatible storage,
  compatible-but-non-target storage, and non-PostgreSQL evidence. SQLite runtime readiness is not valid release
  evidence for the PostgreSQL production contract.
- CI diagnostics must retain both `/readyz` and `/releasez` response bodies and HTTP statuses when the release
  gate fails before Playwright.

- Local R0.2 tooling may expose `release-readiness-preflight`, but the command must call the same
  `production_readiness_service` used by `/readyz` and `/releasez`; duplicating target-storage rules in
  CLI code is forbidden.
- `release-readiness-preflight` stdout is a single `/releasez`-compatible JSON document. Tracing and
  connection/configuration errors must go to stderr so automation can parse stdout without log stripping.
- The preflight exits `0` only when `release_ready=true`; configuration failure, connection failure,
  migration-head mismatch, compatible-but-non-target storage, incompatible storage, and non-PostgreSQL
  evidence must exit non-zero with structured fail-closed JSON.
- The preflight is read-only: it must not call the migration executor, create temporary schemas, execute
  DDL, auto-repair metadata, or imply authorization for the R0.1 Schema change.
- Rust real-E2E workflows must run `release-readiness-preflight` after PostgreSQL migration succeeds and
  before the Rust server starts. The preflight is an early release-contract gate, not a replacement for runtime
  `/readyz` and `/releasez` evidence.
- Runner automation must preserve preflight stdout, stderr, and the original exit code in separate files named
  `release-preflight.json`, `release-preflight-stderr.log`, and `release-preflight-exit-code.txt`.
- Workflow logic must temporarily disable immediate shell exit only long enough to capture the preflight status,
  restore fail-fast behavior, and propagate the original non-zero exit code. `|| true`, swallowed failures, and
  terminal-only diagnostics are forbidden.
- Failure artifact upload paths must include all three preflight evidence files. HTTP `/readyz`, `/releasez`, their
  status files, and the backend log remain required because preflight evidence alone does not prove runtime health.

### R0.3 Linux runner binary identity and cleanup contract (2026-07-13)

- A production real-E2E workflow must build the Rust server binary first and start that binary directly. A
  background `cargo run` wrapper is not an acceptable server process owner because its PID does not prove the
  final server lifecycle.
- Before startup, record the resolved binary path and SHA-256 in diagnostics. After startup, verify both values
  against Linux `/proc/<pid>/exe` and persist a structured identity record.
- Cleanup must treat the PID file as an untrusted reference. Validate the PID shape, require the expected path
  and hash evidence, and re-read `/proc/<pid>/exe` immediately before sending `TERM` or `KILL`.
- If the PID is invalid, expected identity is missing, `/proc/<pid>/exe` is unavailable, or path/hash comparison
  fails, cleanup must write `cleanup_status=signal_refused`, return non-zero, and must not signal that PID.
- The success manifest is written only after verified cleanup succeeds. It must bind the binary path/hash and
  GitHub SHA/run ID/attempt. Failure must write a separate structured manifest, and the diagnostics directory
  must be uploaded with `always()`.
- Playwright output and its original exit code belong in the same diagnostics artifact as migration, preflight,
  readiness, server log, identity, and lifecycle evidence. A terminal-only Playwright result is insufficient.
- Static workflow tests and Linux-container probes are required local gates, but they do not complete R0.3.
  R0.3 passes only when an actual GitHub-hosted runner produces a green, downloadable artifact for the exact
  commit containing the contract.

## Scenario: R0.3 Hosted Runner Browser Origin and Commit Identity Evidence

### 1. Scope / Trigger

- Trigger: `.github/workflows/e2e-smoke.yml` runs the non-development Rust backend against the Playwright browser smoke on a GitHub-hosted runner.
- Scope: workflow environment wiring plus `runner-success.json` and `runner-failure.json` commit identity fields.
- Owner: `.github/workflows/e2e-smoke.yml`, with drift protection in `backend-rs/src/production_ci_contract_tests.rs`.

### 2. Signatures

```text
CORS_ORIGINS=http://127.0.0.1:5175
GITHUB_HEAD_SHA=${{ github.event.pull_request.head.sha || github.sha }}
runner manifest github_sha=${GITHUB_SHA}
runner manifest github_head_sha=${GITHUB_HEAD_SHA}
```

### 3. Contracts

- The non-development Rust E2E job must set an explicit, non-wildcard `CORS_ORIGINS` equal to the Playwright browser origin. It must not rely on the Rust development default `*`.
- `github_sha` is the Actions execution SHA. For a `pull_request` workflow it may be the synthetic merge commit and must not be relabeled as the candidate commit.
- `github_head_sha` is the candidate source SHA. It uses `github.event.pull_request.head.sha` for pull requests and falls back to `github.sha` for push events.
- Both success and failure manifests must record `github_sha`, `github_head_sha`, run ID, and run attempt so an artifact can be joined to both the executed merge ref and the reviewed candidate commit.
- R0.3 evidence is valid only when the run API `head_sha`, manifest `github_head_sha`, and the intended candidate commit are identical.

### 4. Validation & Error Matrix

| Condition | Required result |
| --- | --- |
| Non-development E2E uses `CORS_ORIGINS=*` or omits the explicit origin | Router build fails closed; R0.3 remains failed |
| Pull request event | `github_sha` may be merge SHA; `github_head_sha` must equal `pull_request.head.sha` |
| Push event | `github_head_sha` falls back to `github.sha` and equals `github_sha` |
| Success or failure manifest omits either SHA field | Contract test fails; artifact is insufficient for R0.3 |
| Artifact head SHA differs from the intended candidate | Reject the evidence and keep G0 No-Go |

### 5. Good / Base / Bad Cases

- Good: Playwright uses `http://127.0.0.1:5175`, the Rust job explicitly allows that origin, and both manifest SHA fields map to the GitHub run and PR metadata.
- Base: a push run has identical execution and head SHA values through the documented fallback.
- Bad: a PR failure manifest contains only `${GITHUB_SHA}` and incorrectly treats the synthetic merge SHA as the candidate commit.

### 6. Tests Required

- Assert the workflow contains the explicit Playwright origin and does not contain wildcard `CORS_ORIGINS`.
- Assert `GITHUB_HEAD_SHA` uses `pull_request.head.sha || github.sha`.
- Assert both success and failure manifest templates contain `${GITHUB_SHA}` and `${GITHUB_HEAD_SHA}`.
- Parse the workflow as YAML and parse downloaded manifest JSON before accepting runner evidence.

### 7. Wrong vs Correct

#### Wrong

```yaml
env:
  DEBUG: "false"
# CORS_ORIGINS falls back to '*' and non-development startup fails.
```

```json
{ "github_sha": "${GITHUB_SHA}" }
```

#### Correct

```yaml
env:
  DEBUG: "false"
  CORS_ORIGINS: http://127.0.0.1:5175
  GITHUB_HEAD_SHA: ${{ github.event.pull_request.head.sha || github.sha }}
```

```json
{
  "github_sha": "${GITHUB_SHA}",
  "github_head_sha": "${GITHUB_HEAD_SHA}"
}
```

### R0.1 approved password verifier storage contract (2026-07-13)

- The approved PostgreSQL target for `user_passwords.password_hash` is `TEXT NOT NULL`; bounded storage is
  not the final release contract even when it can hold the current canonical verifier.
- The Rust/Python frozen revision graph head is `20260712_password_hash_phc_text`, with
  `20260517_project_core_defaults` as its parent. New databases and upgraded databases must converge on the
  same column type, nullability, comment, and revision head.
- The production Rust migration executor remains upgrade-only. A guarded downgrade may exist in metadata and
  the frozen source-map for isolated verification, but production code must not execute `downgrade_steps` or
  expose a downgrade CLI.
- Upgrade verification must prove legacy 64-character SHA256 values are byte-for-byte unchanged. Auth
  verification must prove a successful legacy login can persist the canonical Argon2 PHC without truncation.
- Guarded downgrade verification must fail before narrowing the column whenever any verifier exceeds 64
  characters, leaving both the `TEXT` column and stored verifiers unchanged.
- R0.1 source and isolated-database completion does not authorize production migration and does not complete
  R0.2, R0.3, or G0. The next mandatory gate is local PostgreSQL + Rust + Playwright real E2E.

## Scenario: Python Migration/Support CI Dependency Boundary

### 1. Scope / Trigger

- Trigger: `.github/workflows/backend-ci.yml` runs the residual Python migration/support regression job.
- Scope: migration metadata, migrator models, deployment-support tools, and `backend/tests/test_tools` only.
- The Python job is not the production backend runtime gate; Rust owns production fmt/check/test/Clippy.

### 2. Signatures

```text
python -m pip install -r requirements-migrator.txt -r requirements-test.txt
DATABASE_URL=sqlite+aiosqlite:///./data/ci.db pytest tests/test_tools
```

Workflow identity:

```text
job: python-migration-support
python-version: 3.11
timeout-minutes: 20
```

### 3. Contracts

- `cache-dependency-path` must include `backend/requirements-migrator.txt` and
  `backend/requirements-test.txt`.
- The install step must use only those two files; it must not install `backend/requirements.txt`.
- The pytest boundary must remain `tests/test_tools` unless the PRD explicitly expands Python ownership.
- Tests that validate deployment probes require tracked `deploy/strangler-gateway-probes.json` in the checkout.

### 4. Validation & Error Matrix

| Condition | Required result |
| --- | --- |
| Full AI runtime requirements are installed | Contract test fails before merge |
| Python 3.11 cannot collect a support tool | Job fails; fix syntax rather than widening the image/version |
| Deploy probe manifest is missing | Test fails; restore the tracked support file to the checkout |
| Any migration/support test fails | Job fails |
| All scoped tests pass within 20 minutes | Job passes without claiming production-runtime coverage |

### 5. Good / Base / Bad Cases

- Good: cold-cache Python 3.11 installs the two narrow requirement files and passes all `tests/test_tools`.
- Base: pip cache is warm, but the same dependency files and test boundary remain visible in workflow text.
- Bad: install `requirements.txt`, downloading Chroma/Transformers/Torch/CUDA while pytest never starts before timeout.

### 6. Tests Required

- Rust workflow contract test asserts the job name, both dependency files, and `pytest tests/test_tools`.
- The same contract test rejects `-r requirements.txt`.
- A clean-checkout Python 3.11 probe must install from a cold cache and assert all support tests pass.
- YAML parsing and `python -m py_compile backend/tools/check_text_encoding_health.py` must pass locally.

### 7. Wrong vs Correct

#### Wrong

```yaml
run: pip install -r requirements.txt -r requirements-test.txt
# Downloads the retired Python application AI runtime for a migration/support job.
```

#### Correct

```yaml
run: >-
  python -m pip install
  -r requirements-migrator.txt
  -r requirements-test.txt

run: pytest tests/test_tools
```


## Scenario: Cooperative Cancellation and Terminal Persistence Ownership

### 1. Scope / Trigger

- Trigger: a generic background task or batch-generation runtime can continue producing progress or terminal
  persistence after a user cancellation request races the active execution.
- Scope: in-process token registration and propagation, generic/background and batch lifecycle owners, progress
  bridges, task/snapshot transactions, terminal conditional updates, and their Rust tests.
- This contract does not make cancellation durable across process restarts and does not promise resume from an
  arbitrary token position.

### 2. Signatures

```rust
pub(crate) enum CooperativeCancellationScope {
    BackgroundTask,
    BatchGeneration,
}

impl CooperativeCancellationRegistry {
    pub(crate) fn register(
        &self,
        scope: CooperativeCancellationScope,
        task_id: impl Into<String>,
    ) -> CooperativeCancellationRegistration;

    pub(crate) fn cancel(&self, scope: CooperativeCancellationScope, task_id: &str) -> bool;
    fn remove_if_current(&self, key: &CooperativeCancellationKey, registration_id: u64) -> bool;
}

impl CooperativeCancellationToken {
    pub(crate) fn cancel(&self) -> bool;
    pub(crate) async fn cancelled(&self);
}

impl CooperativeCancellationRegistration {
    pub(crate) fn token(&self) -> CooperativeCancellationToken;
    pub(crate) fn cleanup(&self) -> bool;
}
```

Database ownership predicates:

```text
runtime persistence: status NOT IN ('completed', 'failed', 'cancelled')
cancel persistence:  status IN ('pending', 'running')
rows_affected == 0:  rejected terminal-ownership attempt
```

### 3. Contracts

- A registration identity is `(scope, task ID, unique registration ID)`. Registering a replacement cancels the
  previous token, but cleanup from the old execution must not remove the replacement.
- The cancellation registry is an in-process control plane, not durable task state. The database remains the
  terminal-state source of truth.
- Generic and batch lifecycle owners must race their execution future against the token with biased
  `tokio::select!`; the cancellation branch must exit without calling a failed-task projection.
- Every child progress bridge must observe the same token as its lifecycle owner and stop forwarding after the
  signal.
- Durable cancellation must update the task and cancelled snapshot in one transaction. The token may be
  signalled only after that transaction commits successfully.
- Runtime task changes and their snapshot changes must share one transaction. Terminal rows reject late
  preparing, progress, success, cancellation, or failure patches without overwriting the snapshot.
- Runtime conditional updates reject `completed`, `failed`, and `cancelled`; cancellation conditional updates
  accept only `pending` and `running`. A zero affected-row count is an ownership rejection, never success.
- Tests for asynchronously dispatched work may assert the initial state on the synchronous command response,
  but must not assume a later database read still remains at `pending/queued` after the runtime has spawned.

### 4. Validation & Error Matrix

| Condition | Required result |
| --- | --- |
| New registration replaces an active registration | Previous token is cancelled; old cleanup cannot remove the new registration |
| Cancellation persistence commits | Task and snapshot are both cancelled, then the current token is signalled |
| Cancellation persistence rolls back or is rejected | Token remains unsignalled and durable state remains unchanged |
| Runtime patch reaches a terminal task | Conditional update affects zero rows; snapshot is not overwritten |
| Cancellation races completion | Exactly one conditional update commits; task and snapshot expose the same terminal owner |
| Cancellation wins the lifecycle `select!` | Execution exits without projecting `Failed`; child bridges stop |
| Completion wins before cancellation | Completion remains durable and a rejected cancellation must not signal the token |
| Local MSVC test link fails with PDB `LNK1318` | Re-run local verification with `rust-lld`, `debuginfo=0`, and `/DEBUG:NONE`; do not change product logic |

Expected ownership-rejection errors remain stable internal messages:

```text
Batch generation runtime persistence rejected by terminal task status
Batch generation cancel persistence rejected by inactive task status
```

### 5. Good / Base / Bad Cases

- Good: cancellation and final completion start from the same barrier; one transaction wins, the losing owner
  receives a zero-row rejection, task/snapshot agree, and only a committed cancellation signals the token.
- Base: a running task receives cancellation without a competing terminal write; cancelled task and snapshot
  commit together, bridges exit, registration cleanup is idempotent, and the public API/SSE contract is unchanged.
- Bad: signal the token before database commit, update task and snapshot in separate transactions, treat zero
  affected rows as success, call `fail_task` from the cancellation branch, or let old cleanup delete a resumed
  execution's replacement registration.

### 6. Tests Required

- Unit-test registration uniqueness, replacement cancellation, idempotent cleanup, and old-cleanup safety.
- Test generic lifecycle and every progress bridge with the same token; assert cancellation exits without a
  failed projection or leaked forwarding task.
- DB-test successful cancellation, then attempt late preparing/progress/success/failure writes and assert both
  task and snapshot remain cancelled.
- Inject a database failure into cancellation persistence; assert rollback, unchanged running state, and an
  unsignalled token.
- Use a barrier-driven cancel-vs-completion race; assert exactly one terminal owner succeeds and task/snapshot
  terminal states match. Repeat the race to detect timing-sensitive regressions.
- Keep resume fixtures in a valid lifecycle order (`running -> checkpoint -> failed -> resume`).
- Run `cargo fmt --check`, `cargo check`, `cargo check --tests`, focused Rust tests, and the full Rust suite.

### 7. Wrong vs Correct

#### Wrong

```rust
// Signal first: execution may stop even when durable cancellation later rolls back.
registry.cancel(scope, task_id);
task_model.update(db).await?;
snapshot.persist(db).await?;
```

```rust
// Unconditional late runtime write can move a terminal task backwards.
task_model.update(db).await?;
snapshot.persist(db).await?;
```

#### Correct

```rust
let transaction = db.begin().await?;
let result = batch_generation_task::Entity::update_many()
    .set(cancelled_active_model)
    .filter(batch_generation_task::Column::Id.eq(task_id))
    .filter(batch_generation_task::Column::Status.is_in(["pending", "running"]))
    .exec(&transaction)
    .await?;
if result.rows_affected == 0 {
    return Err(inactive_task_error());
}
upsert_cancelled_snapshot(&transaction, task_id).await?;
transaction.commit().await?;
registry.cancel(CooperativeCancellationScope::BatchGeneration, task_id);
```

```rust
let transaction = db.begin().await?;
let result = batch_generation_task::Entity::update_many()
    .set(runtime_active_model)
    .filter(batch_generation_task::Column::Id.eq(task_id))
    .filter(batch_generation_task::Column::Status.is_not_in([
        "completed",
        "failed",
        "cancelled",
    ]))
    .exec(&transaction)
    .await?;
if result.rows_affected == 0 {
    return Err(terminal_ownership_rejected());
}
upsert_runtime_snapshot(&transaction, task_id).await?;
transaction.commit().await?;
```


## R7 Controlled Autopilot Task and Minimal Coordinator Contract (2026-07-16)

### 1. Scope / Trigger

- Trigger: a confirmed `novel_autopilot` background task executes the first controlled workflow Tool.
- Scope: `autopilot_coordinator_service`, the internal Tool Contract, generic background-task dispatch,
  recovery registry, and frontend task-type presentation.
- This is not a Provider-agent loop, a public Autopilot control API, a durable Tool audit store, or a
  resumable invocation protocol.

### 2. Signatures

```rust
pub async fn execute_novel_autopilot_task(
    db: &DatabaseConnection,
    record: &TaskRecord,
    payload: serde_json::Value,
) -> Result<serde_json::Value, String>;

pub struct AutopilotToolExecutionContext<'a> {
    pub actor_user_id: &'a str,
    pub confirmation: AutopilotToolConfirmation,
    pub project_scope: Option<&'a str>,
}
```

`execute_task` owns the `"novel_autopilot"` match arm. It must project success with the existing
`complete_task` owner and must not introduce a second registry, terminal-state owner, task table, or SSE kind.

### 3. Contracts

- `NovelAutopilotTaskPayload` is strict (`deny_unknown_fields`) and contains only `tool_name`, raw JSON
  `arguments`, and `confirmed_by_user`.
- The task actor is always `TaskRecord.user_id`; the canonical project scope is always
  `TaskRecord.project_id`. Neither value is accepted from the payload or Tool arguments.
- `transition_project_workflow` is the only current allowlisted Tool. Its strict project ID must equal the
  optional task `project_scope` before calling `novel_workflow_service::transition`.
- A true `confirmed_by_user` maps to `ConfirmedByUser`; false maps to `Missing`. The coordinator only
  forwards the raw argument string to the Tool Contract and never becomes a JSON/permission boundary.
- The result is the versioned `autopilot-tool-contract/v1` receipt serialized into the existing task result.
- Because `TaskRecord` does not persist the payload, confirmation, or raw arguments, `novel_autopilot` is
  explicitly `NonResumable`; orphan recovery fails safely and never replays the write.

### 4. Validation & Error Matrix

| Input or lifecycle state | Required outcome |
|---|---|
| Empty task project ID, malformed payload, unknown payload field, unknown Tool, malformed arguments | Fail before workflow mutation with a stable safe message |
| Tool project differs from `TaskRecord.project_id` | Fail closed before the workflow service call |
| `confirmed_by_user=false` | Return confirmation-required error; project workflow is unchanged |
| Authenticated task actor lacks project ownership | Preserve canonical workflow not-found/access-denied behavior |
| Valid confirmed CAS transition | Delegate to workflow owner and persist only the versioned receipt |
| Orphaned running task after restart | Mark failed as non-resumable; require a new user-initiated task |

Do not log raw arguments, prompts, tokens, URLs, API keys, or internal database errors. Map a project-scope
mismatch to the safe invalid-task-payload message; map internal failures to a stable execution-failed message.

### 5. Good / Base / Bad Cases

- Good: a task for `project-a` created by `user-a` carries a confirmed `transition_project_workflow` payload
  for `project-a`; the Tool Contract delegates the CAS transition to the canonical workflow service and the
  generic task result stores the receipt.
- Base: a direct internal Tool Contract caller has `project_scope: None`; existing first-slice compatibility
  remains valid while canonical workflow authorization continues to apply.
- Bad: read a user ID or project ID from the payload, let `arguments.project_id` cross task scope, deserialize
  model output in the coordinator, call a Provider/MCP, perform direct SQL, or mark the task restartable even
  though its invocation inputs were not persisted.

### 6. Tests Required

- Tool Contract tests: static allowlist/schema, missing confirmation, strict argument rejection, matching and
  mismatching project scope, canonical authorization, and stale CAS propagation.
- Coordinator SQLite tests: confirmed success, cross-project failure without mutation, strict payload rejection,
  and error redaction.
- Generic task tests: `novel_autopilot` result projection, project requirement, cancellation/terminal ownership
  reuse, recovery registry, and frontend/executor/recovery lockstep contract.
- Run formatting, `cargo check`, focused tests, the full Rust suite, frontend lint/build, and UTF-8/LF/trailing
  whitespace checks for every changed text file.

### 7. Wrong vs Correct

```rust
// Wrong: the payload becomes an authority boundary and bypasses the task scope.
let actor = payload.user_id;
let project_id = payload.project_id;
workflow_service::transition(db, project_id, actor, /* ... */).await?;
```

```rust
// Correct: TaskRecord supplies authority; Tool Contract validates the argument scope before its canonical call.
let context = AutopilotToolExecutionContext {
    actor_user_id: &record.user_id,
    confirmation,
    project_scope: Some(&record.project_id),
};
let receipt = dispatch_autopilot_tool_call(db, context, &payload.tool_name, &payload.arguments).await?;
```

## Health Migration Catalog Regression Rule (2026-07-16)

### 1. Scope / Trigger

- Trigger: a health/readiness test asserts metadata derived from the Rust PostgreSQL migration catalog.
- Scope: test-only readiness assertions in `backend-rs/src/api/health.rs`; the canonical catalog remains
  `schema_migration_metadata_service::postgres_revision_catalog()`.

### 2. Signatures

```rust
pub fn postgres_revision_catalog() -> &'static [PostgresRevisionMetadata];
```

### 3. Contracts

- The readiness payload exposes its migration revision count from the canonical Rust-owned catalog.
- Tests asserting that count must derive the expected value from `postgres_revision_catalog().len()` rather
  than duplicating a numeric literal.

### 4. Validation & Error Matrix

| Change | Required result |
|---|---|
| A revision is added to the canonical catalog | The readiness count test remains aligned without a second count update |
| Catalog construction fails or changes contract | The canonical catalog/service tests fail; do not mask the failure in health tests |
| A test hardcodes the old catalog length | Treat it as a regression-prone duplicate of migration metadata |

### 5. Good / Base / Bad Cases

- Good: health tests compare the JSON revision count with `json!(postgres_revision_catalog().len())`.
- Base: tests for a specific revision head may assert the named head when that exact contract is intended.
- Bad: change an expected count from one literal to another after every migration, or reconstruct the catalog
  in the health test.

### 6. Tests Required

- Run the targeted readiness metadata test and the canonical catalog single-chain test after changing
  migration metadata.
- Run the full Rust suite when the health readiness contract or catalog is modified.

### 7. Wrong vs Correct

```rust
// Wrong: a duplicate count drifts whenever a migration is added.
assert_eq!(body["checks"]["schema_migration"]["postgres_revision_catalog"]["revision_count"], json!(20));
```

```rust
// Correct: the test follows the Rust-owned catalog.
assert_eq!(
    body["checks"]["schema_migration"]["postgres_revision_catalog"]["revision_count"],
    json!(postgres_revision_catalog().len()),
);
```
## Deterministic AI Smoke Prompt Classification Rule (2026-07-19)

Deterministic OpenAI-compatible smoke providers must classify a request from the prompt's
**current task instruction**, not from arbitrary numbers appearing elsewhere in the template.

- Prefer explicit smoke output markers and task-specific sentences such as `撰写第 X 章`,
  `全面分析第 X 章`, or `章节：第 X 章`.
- Do not collect every chapter-like number in the whole prompt and choose `min` or `max`.
- Do not prioritize parameter guidance, JSON examples, output schemas, or explanatory text such as
  `chapter_number为2或3`; those numbers describe the template rather than the current task.
- Keep regression examples for chapter generation, analysis, repair, and polish prompt shapes. When
  production system templates change, update the classifier tests with the real task sentence before
  changing the classifier fallback order.
- Smoke failure summaries and persisted artifacts must not include complete prompts, generated prose,
  provider reasoning/thinking, credentials, or raw provider errors. Store only allowlisted status,
  counters, identifiers, hashes, and explicit non-sensitive markers.

The regression class is an implicit-assumption and coverage-gap failure: a generic structured field may
appear in instructions or examples and is not necessarily the active request value. The prevention
mechanism is task-sentence-first parsing plus real-template regression fixtures.
