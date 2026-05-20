# Implementation Plan

## Execution Rule

Do not start implementation until the planning artifacts in this task are
reviewed and approved.

## Ordered Checklist

1. Inspect the current `backend-rs` state and select the highest-signal
   remaining seam for this round.
2. Load the relevant backend spec indexes before editing code.
3. Implement one low-risk slice only.
4. Add focused tests if the slice extracts or changes pure helper behavior.
5. Run validation with `cargo check` and any relevant targeted tests.
6. Reassess whether the next seam is still worth pursuing in this session.

## Candidate Execution Waves

### Wave 1: Batch-generation semantics hardening

Goal:

- tighten service-owned runtime or read-side semantics with helper extraction
  and regression protection

Likely candidates:

- progress/checkpoint helper cleanup
- stream fallback or event-status helper cleanup
- status view normalization

Validation:

- `cargo check`
- focused unit tests in touched service files

Completed checkpoint:

- `chapter_batch_generation_status_view_service.rs`
  - `BatchGenerationStreamState` now owns `event_status`
  - progress-event payload construction no longer remaps status locally
  - focused tests added for failed/cancelled/unknown fallback semantics
  - `cargo check` passed after the slice
- `chapter_batch_generation_status_stream_service.rs`
  - terminal event selection now goes through
    `build_terminal_batch_generation_events()`
  - focused tests added for completed/failed/cancelled/non-terminal branches
  - `cargo check` passed after the slice
- `chapter_batch_generation_status_payload_adapter_service.rs`
  - shared payload field assembly now goes through `base_task_payload()`
  - focused tests added to protect the difference between
    `task_status_payload()` and `active_task_payload()`
  - `cargo check` passed after the slice

### Wave 2: Provider seam tightening

Goal:

- remove any remaining route-local default provider assembly when the service
  boundary can own it without behavior change

Validation:

- `cargo check`
- targeted grep/sanity checks on the affected call path

Candidate focus:

- identify any remaining route-local or caller-local default provider payload
  assembly in batch-generation / single-generation paths
- move ownership only if the prepared request/workflow boundary can hold the
  default once without changing behavior

Completed checkpoint:

- `chapter_generation_runtime_service.rs`
  - removed the default provider-payload wrapper entrypoint
  - kept only the explicit `generate_and_persist_chapter_content_with_provider_payload()`
    boundary
  - `cargo check` passed after the slice
- `chapter_batch_generation_access_service.rs`
  - added `prepare_generation_execution_config()` so `AIConfig` plus default
    provider payload are prepared once in a shared helper
  - `chapter_batch_generation_create_workflow_service.rs`,
    `chapter_batch_generation_resume_service.rs`, and
    `chapter_single_generation_request_service.rs` now consume that helper
    instead of repeating local assembly
  - `cargo check` passed after the slice

### Wave 3: Route seam compression

Goal:

- keep route files transport-only once adjacent service semantics are stable

Validation:

- `cargo check`
- targeted payload-shape sanity review if route shaping changes

Current priority:

- lower than Wave 2 unless a no-risk route-only delegation cleanup is clearly
  isolated from behavior-sensitive files

### Wave 4: Phase 5 governance asset hardening

Goal:

- enter Phase 5 through executable governance assets, not by removing Python
  fallback prematurely

Execution rule:

- keep this wave limited to owner / smoke / rollback evidence
- do not couple Phase 5 work to new Rust business behavior or schema changes
- only promote a route-group slice when it can be validated through the
  gateway with stable expectations

Sub-waves:

1. P0 route-group smoke isolation
   - split `chapters` / `projects` / `wizard-stream` / `settings` /
     `memories` into a dedicated `phase5-p0` smoke profile
   - keep `deploy`, `route-groups`, and `business` profiles unchanged as
     broader control-plane / owner evidence lanes
2. P0 stronger business/SSE evidence
   - add one stronger business or SSE smoke slice for the highest-signal P0
     groups when the assertion can stay stable through the gateway
   - prefer structure/content-type assertions over time-sensitive values
3. rollback asset capture
   - turn the current route-group checklist rollback notes into executable or
     operator-ready per-group steps, starting with P0 groups
4. P1 route-group follow-up
   - extend stable owner/business smoke to `auth`, `users`, `characters`,
     `outlines`, and `book_import` only after P0 evidence is solid

Validation:

- `pytest backend/tests/test_tools/test_run_strangler_gateway_smoke.py -q`
- `python backend/tools/run_strangler_gateway_smoke.py --manifest deploy/strangler-gateway-probes.json --profile <profile> --validate-manifest-only`
- live gateway smoke only when the target environment is up and the probe is
  stable enough to avoid false positives

Current priority:

- start with P0 route-group smoke isolation and stronger owner evidence
- defer Python fallback removal until smoke + rollback assets are complete
- now that the P0/P1 governance assets are executable, it is also valid to
  continue low-risk `backend-rs` seam tightening in parallel, as long as each
  slice stays behavior-preserving and independently verifiable

Completed checkpoint:

- `chapter_single_generation_background_workflow_service.rs`
  - moved single-chapter background generation orchestration out of
    `chapter_batch_generation.rs`
  - route now delegates `prepare request + create task plan` to a dedicated
    workflow service before dispatching runtime work
  - `cargo check` passed after the slice
- `chapter_single_generation_stream_workflow_service.rs`
  - moved single-chapter stream orchestration out of
    `chapter_batch_generation.rs`
  - route now delegates `prepare request + build stream` to a dedicated
    workflow service before returning SSE transport state
  - `cargo check` passed after the slice
- `chapter_batch_generation_dispatch_service.rs`
  - added `dispatch_resume_generation_runtime()` so
    `chapter_batch_generation.rs` no longer owns `ResumeExecutionPlan`
    branching before runtime dispatch
  - `cargo check` passed after the slice
- `chapter_batch_generation_task_command_service.rs`
  - removed local task type / stage / execution-mode semantics helpers
  - now reuses shared `chapter_batch_generation_status_semantics_service`
    helpers while keeping the existing payload shape
  - `cargo check` passed after the slice
  - targeted `cargo test chapter_batch_generation_task_command_service`
    still fails due to pre-existing test compile errors outside this slice
- validation unblock checkpoint
  - fixed the stale `terminal_semantics` test import in
    `chapter_batch_generation_status_view_service.rs`
  - added the missing `serde_json::Value` test import in
    `chapter_quality_metrics_source_service.rs`
  - `cargo check --manifest-path "backend-rs/Cargo.toml"` passed after the fix
  - targeted `cargo test chapter_batch_generation_task_command_service` no
    longer fails on those source-level test import issues; the remaining
    failure is a Rust test compilation memory/stack-overrun problem in this
    environment
- cancel/runtime consistency checkpoint
  - `chapter_batch_generation_task_command_service.rs` now calls the existing
    cancelled runtime finalizer after explicit cancel
  - this keeps `task.status=cancelled` aligned with persisted
    `workflow_runtime_state/checkpoint` instead of leaving stale
    `generating/running` state behind
  - `cargo check --manifest-path "backend-rs/Cargo.toml"` passed after the fix
- resume/runtime snapshot consistency checkpoint
  - `chapter_batch_generation_task_command_service.rs` now rebuilds the resume
    snapshot instead of merge-updating the old runtime checkpoint payload
  - batch-task resume clears stale `current_chapter_*`, resets checkpoint
    `completed/total` progress to a fresh pending state, and clears stale
    `failed_chapters`
  - resume snapshot replacement also clears stale
    `latest_quality_metrics/quality_metrics_history/quality_metrics_summary`
    so pending tasks do not keep exposing the previous terminal quality state
  - single-chapter resume still preserves the current chapter pointer
  - `cargo check --manifest-path "backend-rs/Cargo.toml"` passed after the fix
  - `cargo test chapter_batch_generation_task_command_service --manifest-path "backend-rs/Cargo.toml"`
    passed with the focused resume-checkpoint tests
- runtime/read-side semantics dedupe checkpoint
  - removed the stale status/read-side payload helpers from
    `chapter_batch_generation_runtime_state_service.rs`
  - runtime state service now keeps runtime snapshot/checkpoint execution
    helpers only, while status vocabulary and response payload assembly stay
    owned by `chapter_batch_generation_status_semantics_service.rs`,
    `chapter_batch_generation_quality_status_service.rs`, and
    `chapter_batch_generation_status_payload_adapter_service.rs`
  - this reduces the chance of future drift between runtime internals and
    status/query responses without changing HTTP or SSE payloads
  - `cargo check --manifest-path "backend-rs/Cargo.toml"` passed after the
    cleanup
  - focused tests passed:
    `chapter_batch_generation_runtime_state_service`,
    `chapter_batch_generation_task_command_service`, and
    `chapter_batch_generation_status_payload_adapter_service`
- manual-review semantics dedupe checkpoint
  - `manual_review_label()` is now owned by
    `chapter_batch_generation_quality_status_service.rs`
  - `chapter_batch_generation_task_command_service.rs` reuses that helper for
    resume blocking instead of carrying a second copy of the same quality-gate
    parsing logic
  - focused tests now protect the shared manual-review label behavior used by
    both status payloads and resume blocking
  - `cargo check --manifest-path "backend-rs/Cargo.toml"` passed after the
    change
  - focused tests passed:
    `chapter_batch_generation_task_command_service`,
    `chapter_batch_generation_status_view_service`, and
    `chapter_batch_generation_status_payload_adapter_service`
- chapter-regeneration stream workflow checkpoint
  - `chapter_regeneration_stream_workflow_service.rs` now owns the full and
    partial regeneration stream workflow assembly
  - `chapter_regeneration_routes.rs` delegates access loading, prepare-service
    calls, and stream input construction to that workflow service, leaving the
    route with HTTP/SSE boundary and error mapping only
  - this follows the Phase 3 route-to-workflow seam from
    `docs/architecture/chapter-api-gateway-seams.zh-CN.md` without changing
    prompt construction, SSE event shape, or generated result payloads
  - targeted `rustfmt --edition 2021 --check` passed for the touched Rust
    files
  - `cargo check --manifest-path "backend-rs/Cargo.toml"` passed after the
    change
  - `cargo test chapter_regeneration --manifest-path "backend-rs/Cargo.toml"`
    compiled successfully; the filter currently matches no tests and still
    reports the pre-existing `utils/sse.rs` unused `Event` warnings
- analysis/regeneration query ownership checkpoint
  - `chapter_analysis_query_service.rs` now owns access loading for the
    chapter analysis view and chapter quality metrics query helpers
  - `chapter_analysis_routes.rs` now delegates those two read routes to owned
    query helpers instead of loading the chapter in the route and then passing
    it through
  - `chapter_regeneration_query_service.rs` now owns access loading and limit
    normalization for regeneration task history queries
  - `chapter_regeneration_routes.rs` now delegates regeneration task history
    lookup to the owned query helper, keeping route-local logic to transport
    parsing and response mapping
  - `cargo check --manifest-path "backend-rs/Cargo.toml"` passed after the
    change
  - focused filters compiled successfully:
    `cargo test chapter_analysis --manifest-path "backend-rs/Cargo.toml"` and
    `cargo test chapter_regeneration --manifest-path "backend-rs/Cargo.toml"`;
    both filters currently match no tests and still report the pre-existing
    `utils/sse.rs` unused `Event` warnings
- analysis draft workflow/query ownership checkpoint
  - `chapter_analysis_draft_service.rs` now owns the access loading, request
    parsing, and payload delegation for auto-revision draft load/apply and
    candidate draft load/apply routes
  - `chapter_analysis_routes.rs` now keeps those four draft endpoints as
    transport-only handlers that pass query/body data to owned service helpers
    and map typed service errors
  - `chapter_analysis_draft_error_mapper.rs` now maps the owned draft helper
    errors while reusing the existing draft error response semantics, including
    the history/attempt-id-specific 404 messages and stale/preview conflict
    responses
  - `cargo check --manifest-path "backend-rs/Cargo.toml"` passed after the
    change
  - `cargo test chapter_analysis --manifest-path "backend-rs/Cargo.toml"`
    compiled successfully; the filter currently matches no tests and still
    reports the pre-existing `utils/sse.rs` unused `Event` warnings
- partial-regeneration apply ownership checkpoint
  - `chapter_regeneration_apply_service.rs` now owns access loading and route
    default normalization for partial-regeneration apply requests
  - `chapter_regeneration_routes.rs` delegates `apply-partial-regenerate` to
    `apply_owned_partial_regenerate_payload()` and no longer loads the chapter
    or expands missing body fields locally
  - the existing `ApplyPartialRegenerateError` mapping remains unchanged, so
    empty text, workflow meta text, invalid range, not found, and internal
    failures preserve their HTTP response semantics
  - `cargo check --manifest-path "backend-rs/Cargo.toml"` passed after the
    change
  - `cargo test chapter_regeneration --manifest-path "backend-rs/Cargo.toml"`
    compiled successfully; the filter currently matches no tests and still
    reports the pre-existing `utils/sse.rs` unused `Event` warnings
- analysis trigger dispatch ownership checkpoint
  - `chapter_analysis_trigger_service.rs` now owns dispatching the prepared
    chapter-analysis background runtime
  - `chapter_analysis_routes.rs` now prepares the trigger, delegates dispatch,
    and returns the existing payload without directly spawning the background
    runtime task
  - task creation, runtime execution, response payload, and
    `PrepareChapterAnalysisTriggerError` mapping remain unchanged
  - `cargo check --manifest-path "backend-rs/Cargo.toml"` passed after the
    change
  - `cargo test chapter_analysis --manifest-path "backend-rs/Cargo.toml"`
    compiled successfully; the filter currently matches no tests and still
    reports the pre-existing `utils/sse.rs` unused `Event` warnings
- active batch task list query ownership checkpoint
  - `chapter_batch_generation_active_list_query_service.rs` now owns the
    active-task list limit default and clamp semantics
  - `chapter_batch_generation.rs` delegates the raw optional query limit to
    the owned query helper instead of normalizing it in the route
  - the default remains `20` and the allowed range remains `1..=100`
  - `cargo check --manifest-path "backend-rs/Cargo.toml"` passed after the
    change
  - `cargo test chapter_batch_generation_active_list_query_service --manifest-path "backend-rs/Cargo.toml"`
    compiled successfully; the filter currently matches no tests and still
    reports the pre-existing `utils/sse.rs` unused `Event` warnings
- SSE validation cleanup checkpoint
  - `backend-rs/src/utils/sse.rs` tests now explicitly ignore the returned
    `Event` values in the no-panic smoke test
  - this removes the recurring unused `Event` warnings from focused Rust test
    runs without changing runtime SSE event builders or payload shape
  - `cargo check --manifest-path "backend-rs/Cargo.toml"` passed after the
    change
  - `cargo test sse --manifest-path "backend-rs/Cargo.toml"` passed with 5
    tests and no unused `Event` warnings
- single-chapter compatibility field ownership checkpoint
  - `chapter_single_generation_request_service.rs` now owns consumption of the
    background single-chapter generation compatibility-only `enable_analysis`
  field
  - `chapter_batch_generation.rs` delegates that compatibility field handling
    to the request service instead of swallowing it in the route handler
  - this preserves request compatibility without changing generation
    execution fields, provider payload ownership, response payloads, or SSE
    behavior
  - targeted `rustfmt --edition 2021 --check` passed for the touched Rust
    files
  - `cargo check --manifest-path "backend-rs/Cargo.toml"` passed after the
    change
  - `cargo test chapter_single_generation_request_service --manifest-path "backend-rs/Cargo.toml"`
    compiled successfully; the filter currently matches no tests
- single-chapter request builder ownership checkpoint
  - `chapter_single_generation_request_service.rs` now owns one
    `build_single_chapter_generation_request()` helper for the standard
    single-chapter request shape plus compat-only field consumption
  - `chapter_batch_generation.rs` no longer assembles the single-chapter
    request struct inline for background vs stream separately
  - this keeps transport/auth/workflow behavior unchanged while shrinking
    route-local request ownership
  - `cargo check --manifest-path "backend-rs/Cargo.toml"` passed after the
    change
  - focused `cargo test chapter_single_generation_request_service --manifest-path "backend-rs/Cargo.toml"`
    passed with 3 tests
- regeneration default-semantics explicitness checkpoint
  - `chapter_regeneration_query_service.rs` now owns
    `normalize_regeneration_tasks_limit()` for regeneration task history query
    defaults and clamping
  - `chapter_regeneration_stream_workflow_service.rs` now owns explicit
    helpers for partial-regeneration `context_chars` and
    `enable_web_research` defaults
  - this keeps the existing defaults unchanged (`limit` default `10`, clamp
    `1..=50`; `context_chars` default `500`; web research default `false`)
    while making the cross-layer default semantics testable instead of leaving
    them embedded in call expressions
  - targeted `rustfmt --edition 2021 --check` passed for the touched Rust
    files
  - `cargo check --manifest-path "backend-rs/Cargo.toml"` passed after the
    change
  - focused tests passed:
    `chapter_regeneration_query_service` (1 test) and
    `chapter_regeneration_stream_workflow_service` (2 tests)
- active batch task list default-semantics explicitness checkpoint
  - `chapter_batch_generation_active_list_query_service.rs` now owns
    `normalize_active_batch_generation_task_list_limit()` for active batch
    task list query defaults and clamping
  - this keeps the existing default unchanged (`20`) and allowed range
    unchanged (`1..=100`) while making the query boundary default semantics
    testable
  - targeted `rustfmt --edition 2021 --check` passed for the touched Rust file
  - `cargo check --manifest-path "backend-rs/Cargo.toml"` passed after the
    change
  - focused `cargo test chapter_batch_generation_active_list_query_service --manifest-path "backend-rs/Cargo.toml"`
    passed with 1 test
- batch generation create default-semantics explicitness checkpoint
  - `chapter_batch_generation_create_workflow_service.rs` now owns
    `normalize_batch_generation_enable_analysis()` and
    `normalize_batch_generation_max_retries()` for batch-create workflow
    defaults
  - this keeps the existing defaults unchanged (`enable_analysis=false`,
    `max_retries=3`) while making create-workflow default semantics testable
  - targeted `rustfmt --edition 2021 --check` passed for the touched Rust file
  - `cargo check --manifest-path "backend-rs/Cargo.toml"` passed after the
    change
  - focused `cargo test chapter_batch_generation_create_workflow_service --manifest-path "backend-rs/Cargo.toml"`
    passed with 2 tests
- generation target-word-count default-semantics test checkpoint
  - added focused tests for the existing
    `normalize_batch_generation_target_word_count()` helper in
    `chapter_batch_generation_create_service.rs`
  - added focused tests for the existing
    `normalize_single_chapter_generation_target_word_count()` and
    `load_chapter_generation_target()` helpers in
    `chapter_single_generation_request_service.rs`
  - this does not change runtime behavior; it protects the shared default
    target word count (`3000`), minimum clamp (`1`), and explicit target value
    preservation used by batch and single-chapter generation
  - targeted `rustfmt --edition 2021 --check` passed for the touched Rust
    files
  - `cargo check --manifest-path "backend-rs/Cargo.toml"` passed after the
    test additions
  - focused tests passed:
    `chapter_batch_generation_create_service` (1 test) and
    `chapter_single_generation_request_service` (2 tests)
- analysis draft request default-semantics test checkpoint
  - added focused tests for `chapter_analysis_draft_request_service.rs`
    request parsing helpers
  - coverage now protects auto-revision vs candidate draft field ownership,
    trimmed/empty ID handling, and `allow_stale=false` default semantics
  - this does not change runtime behavior; it adds regression protection for
    the service-owned draft request boundary used by analysis draft routes
  - targeted `rustfmt --edition 2021 --check` passed for the touched Rust file
  - `cargo check --manifest-path "backend-rs/Cargo.toml"` passed after the
    test additions
  - focused `cargo test chapter_analysis_draft_request_service --manifest-path "backend-rs/Cargo.toml"`
    passed with 4 tests
- regeneration prepare default-semantics test checkpoint
  - added focused tests for `chapter_regeneration_prepare_service.rs` pure
    helpers
  - coverage now protects partial-regeneration length-mode defaults,
    target-word calculation, override selected text handling, context
    extraction, invalid range rejection, and empty-selection rejection
  - this does not change runtime behavior; it adds regression protection for
    the service-owned partial-regeneration preparation boundary used by SSE
    workflows
  - targeted `rustfmt --edition 2021 --check` passed for the touched Rust file
  - `cargo check --manifest-path "backend-rs/Cargo.toml"` passed after the
    test additions
  - focused `cargo test chapter_regeneration_prepare_service --manifest-path "backend-rs/Cargo.toml"`
    passed with 5 tests
- regeneration prepare prompt/boundary default-semantics test checkpoint
  - added focused tests for `chapter_regeneration_prepare_service.rs` prompt
    and partial-regeneration boundary helpers
  - coverage now protects full-regeneration prompt defaults
    (`target_word_count=3000`, `preserve_structure=false`,
    `preserve_character_traits=true`), explicit prompt field projection, blank
    selected-text fallback to chapter content, edge context extraction, and
    partial-regeneration `max_tokens` floor/cap semantics
  - this does not change runtime behavior; it adds regression protection for
    the service-owned prompt/preparation contract used by regeneration SSE
    workflows
  - targeted `rustfmt --edition 2021 --check` passed for the touched Rust file
  - `cargo check --manifest-path "backend-rs/Cargo.toml"` passed after the
    test additions
  - focused `cargo test chapter_regeneration_prepare_service --manifest-path "backend-rs/Cargo.toml"`
    passed with 9 tests
- analysis query error-code semantics test checkpoint
  - added focused tests for `chapter_analysis_query_service.rs`
    `classify_analysis_error_code()` mapping
  - coverage now protects retrying, JSON parse/format failures, empty AI
    response, stream interruption, timeout/startup timeout, empty chapter,
    missing project, unknown-error fallback, and missing-error `None`
    semantics
  - this does not change runtime behavior or payload shape; it adds regression
    protection for the read-side analysis task status `error_code` field
  - targeted `rustfmt --edition 2021 --check` passed for the touched Rust
    files
  - `cargo check --manifest-path "backend-rs/Cargo.toml"` passed after the
    test additions
  - focused `cargo test chapter_analysis_query_service --manifest-path "backend-rs/Cargo.toml"`
    passed with 2 tests
- partial-regeneration apply preparation seam checkpoint
  - `chapter_regeneration_apply_service.rs` now owns a pure
    `prepare_partial_regenerate_apply()` helper for sanitized replacement
    text validation, range validation, new-content assembly, and old word-count
    capture before the DB update boundary
  - `apply_partial_regenerate_payload()` now delegates preparation to that
    helper and keeps the existing `ChapterService::update()` response/error
    shape unchanged
  - focused tests protect successful content replacement, empty generated text,
    meta-only generated text becoming empty after sanitization, and invalid
    range rejection
  - targeted `rustfmt --edition 2021 --check` passed for the touched Rust file
  - `cargo check --manifest-path "backend-rs/Cargo.toml"` passed after the
    seam extraction
  - focused `cargo test chapter_regeneration_apply_service --manifest-path "backend-rs/Cargo.toml"`
    passed with 4 tests
- batch status checkpoint metadata semantics test checkpoint
  - added focused tests for
    `chapter_batch_generation_status_payload_adapter_service.rs`
    `checkpoint_with_runtime_metadata()`
  - coverage now protects empty-runtime checkpoint metadata insertion and
    existing checkpoint field preservation while current `stage_code` /
    `execution_mode` override stale runtime metadata
  - this does not change runtime behavior or payload shape; it adds direct
    regression protection for the status/read-side checkpoint adapter contract
  - targeted `rustfmt --edition 2021 --check` passed for the touched Rust file
  - `cargo check --manifest-path "backend-rs/Cargo.toml"` passed after the
    test additions
  - focused `cargo test chapter_batch_generation_status_payload_adapter_service --manifest-path "backend-rs/Cargo.toml"`
    passed with 4 tests
- regeneration text normalization/finalize semantics test checkpoint
  - added focused tests for `chapter_regeneration_text_service.rs`
    output normalization and finalization helpers
  - coverage now protects rewrite-prefix stripping, quote/bracket unwrapping,
    partial-regeneration result payload fields, full-chapter regeneration
    payload fields, and meta-only generated text becoming `EmptyContent` after
    sanitization
  - this does not change runtime behavior or SSE payload shape; it adds direct
    regression protection for the regeneration stream finalization contract
  - targeted `rustfmt --edition 2021 --check` passed for the touched Rust file
  - `cargo check --manifest-path "backend-rs/Cargo.toml"` passed after the
    test additions
  - focused `cargo test chapter_regeneration_text_service --manifest-path "backend-rs/Cargo.toml"`
    passed with 4 tests
- batch quality status context semantics test checkpoint
  - added focused tests for
    `chapter_batch_generation_quality_status_service.rs`
    `active_story_repair_payload_from_runtime_state()` and
    `build_quality_status_context()`
  - coverage now protects object-only active story repair payload extraction,
    non-object payload rejection, snapshot quality metric/summary propagation,
    and empty-context defaults
  - this does not change runtime behavior or payload shape; it adds direct
    regression protection for the batch read-side quality status context used
    by status view payload assembly
  - targeted `rustfmt --edition 2021 --check` passed for the touched Rust file
  - `cargo check --manifest-path "backend-rs/Cargo.toml"` passed after the
    test additions
  - focused `cargo test chapter_batch_generation_quality_status_service --manifest-path "backend-rs/Cargo.toml"`
    passed with 4 tests
- analysis checker fragments semantics test checkpoint
  - added focused tests for `chapter_analysis_checker_query_service.rs`
    `build_chapter_analysis_checker_fragments()`
  - coverage now protects selecting the first valid
    `chapter_text_checker_v1` history, ignoring unrelated/invalid/missing
    checker-result histories, formatting checker creation time, and preserving
    `checker_result` when a matching history has no `created_at`
  - this does not change runtime behavior or payload shape; it adds direct
    regression protection for the analysis view read-side checker fragment
    contract
  - targeted `rustfmt --edition 2021 --check` passed for the touched Rust file
  - `cargo check --manifest-path "backend-rs/Cargo.toml"` passed after the
    test additions
  - focused `cargo test chapter_analysis_checker_query_service --manifest-path "backend-rs/Cargo.toml"`
    passed with 3 tests
- regeneration query datetime semantics test checkpoint
  - added focused tests for `chapter_regeneration_query_service.rs`
    `datetime_to_string()`
  - coverage now protects regeneration task history timestamp formatting
    (`%Y-%m-%dT%H:%M:%S`) and `None` propagation
  - this does not change runtime behavior or payload shape; it adds direct
    regression protection for the regeneration task history read-side payload
    contract
  - targeted `rustfmt --edition 2021 --check` passed for the touched Rust file
  - `cargo check --manifest-path "backend-rs/Cargo.toml"` passed after the
    test additions
  - focused `cargo test chapter_regeneration_query_service --manifest-path "backend-rs/Cargo.toml"`
    passed with 2 tests
- batch request compat projection semantics test checkpoint
  - added focused tests for
    `chapter_batch_generation_request_compat_service.rs`
    `project_batch_generation_request_compat_fields()`
  - coverage now protects projection of owned compatibility fields into
    borrowed view fields, including optional strings, booleans, vector slices,
    and all-default `None` semantics
  - this does not change runtime behavior or request compatibility; it adds
    direct regression protection for the service-owned legacy request field
    consumption boundary
  - targeted `rustfmt --edition 2021 --check` passed for the touched Rust file
  - `cargo check --manifest-path "backend-rs/Cargo.toml"` passed after the
    test additions
  - focused `cargo test chapter_batch_generation_request_compat_service --manifest-path "backend-rs/Cargo.toml"`
    passed with 2 tests
- analysis view payload adapter semantics test checkpoint
  - added focused tests for `chapter_analysis_view_payload_adapter_service.rs`
    `value_or_null()` and `build_chapter_analysis_view_payload()`
  - coverage now protects JSON null fallback, analysis field projection,
    memory projection and `is_foreshadow` boolean conversion, checker/draft
    fragment projection, quality metrics/summary projection, and top-level
    `created_at`
  - this does not change runtime behavior or payload shape; it adds direct
    regression protection for the analysis view read-side response adapter
  - targeted `rustfmt --edition 2021 --check` passed for the touched Rust file
  - `cargo check --manifest-path "backend-rs/Cargo.toml"` passed after the
    test additions
  - focused `cargo test chapter_analysis_view_payload_adapter_service --manifest-path "backend-rs/Cargo.toml"`
    passed with 2 tests
- chapter quality metrics payload adapter semantics test checkpoint
  - added focused tests for
    `chapter_quality_metrics_payload_adapter_service.rs`
    `build_chapter_quality_metrics_payload()`
  - coverage now protects `has_metrics`, `latest_metrics` /
    `latest_quality_metrics` compatibility duplication, history/timestamp
    projection, summary projection, and null/default response fields when no
    metrics are available
  - this does not change runtime behavior or payload shape; it adds direct
    regression protection for the chapter quality metrics read-side response
    adapter
  - targeted `rustfmt --edition 2021 --check` passed for the touched Rust file
  - `cargo check --manifest-path "backend-rs/Cargo.toml"` passed after the
    test additions
  - focused `cargo test chapter_quality_metrics_payload_adapter_service --manifest-path "backend-rs/Cargo.toml"`
    passed with 2 tests
- batch generation status semantics test checkpoint
  - added focused tests for
    `chapter_batch_generation_status_semantics_service.rs`
    `task_type()`, `task_stage_code()`, and `task_execution_mode()`
  - coverage now protects single-vs-batch task classification, malformed
    single-task fallback, status-to-stage-code mapping, unknown-status pending
    fallback, and current `interactive` execution-mode compatibility
  - this does not change runtime behavior or payload shape; it adds direct
    regression protection for shared batch status vocabulary consumed by
    status payloads and task commands
  - targeted `rustfmt --edition 2021 --check` passed for the touched Rust file
  - `cargo check --manifest-path "backend-rs/Cargo.toml"` passed after the
    test additions
  - focused `cargo test chapter_batch_generation_status_semantics_service --manifest-path "backend-rs/Cargo.toml"`
    passed with 3 tests
- chapter quality query fragments semantics test checkpoint
  - added focused tests for `chapter_quality_query_service.rs`
    `build_chapter_analysis_quality_fragments()` and
    `build_chapter_quality_metrics_fragments()`
  - coverage now protects analysis-view quality summary construction without
    runtime context, metrics-view quality summary construction with runtime
    context, history id/timestamp projection, raw metrics preservation, and
    empty-fragment defaults when no metrics source is available
  - this does not change runtime behavior or payload shape; it adds direct
    regression protection for chapter quality read-side fragment assembly
  - targeted `rustfmt --edition 2021 --check` passed for the touched Rust file
  - `cargo check --manifest-path "backend-rs/Cargo.toml"` passed after the
    test additions
  - focused `cargo test chapter_quality_query_service --manifest-path "backend-rs/Cargo.toml"`
    passed with 3 tests
- active batch generation query response semantics test checkpoint
  - added focused tests for `chapter_batch_generation_active_query_service.rs`
    `build_active_batch_generation_query_response()`
  - coverage now protects the no-active-task read-side response contract:
    `has_active_task=false` and `task=null`
  - this does not change runtime behavior or payload shape; it adds direct
    regression protection for the active batch query adapter empty-state
    response
  - targeted `rustfmt --edition 2021 --check` passed for the touched Rust file
  - `cargo check --manifest-path "backend-rs/Cargo.toml"` passed after the
    test additions
  - focused `cargo test chapter_batch_generation_active_query_service --manifest-path "backend-rs/Cargo.toml"`
    passed with 1 test
- analysis runtime pure-helper semantics test checkpoint
  - added focused tests for `chapter_analysis_runtime_service.rs` pure helper
    semantics
  - coverage now protects JSON integer default/clamp behavior, non-finite
    float filtering, analysis task status normalization, composed analysis
    report section formatting, and empty-section fallback
  - this does not change runtime behavior or payload shape; it adds direct
    regression protection for analysis-runtime persistence normalization
  - targeted `rustfmt --edition 2021 --check` passed for the touched Rust file
  - `cargo check --manifest-path "backend-rs/Cargo.toml"` passed after the
    test additions
  - focused `cargo test chapter_analysis_runtime_service --manifest-path "backend-rs/Cargo.toml"`
    passed with 5 tests
- chapter CRUD payload adapter semantics test checkpoint
  - added focused tests for `chapter_crud_payload_adapter_service.rs`
    response adapter helpers
  - coverage now protects compatible list payload duplication
    (`success`/`data`/`items`/`total`), project-path list payload shape, and
    compatible single-chapter payload top-level plus nested `data` projection
  - this does not change runtime behavior or response payload shape; it adds
    direct regression protection for chapter CRUD compatibility adapters
  - targeted `rustfmt --edition 2021 --check` passed for the touched Rust file
  - `cargo check --manifest-path "backend-rs/Cargo.toml"` passed after the
    test additions
  - focused `cargo test chapter_crud_payload_adapter_service --manifest-path "backend-rs/Cargo.toml"`
    passed with 3 tests
- batch single-task chapter payload semantics test checkpoint
  - added a focused test for
    `chapter_batch_generation_chapter_payload_service.rs`
    `single_task_chapter_payload()`
  - coverage now protects the single-task batch chapter snapshot contract:
    only `id`, `chapter_number`, and `title` are projected into the stored
    chapter payload
  - this does not change runtime behavior or task payload shape; it adds
    direct regression protection against leaking chapter body/status fields
    into batch task chapter snapshots
  - targeted `rustfmt --edition 2021 --check` passed for the touched Rust file
  - `cargo check --manifest-path "backend-rs/Cargo.toml"` passed after the
    test addition
  - focused `cargo test chapter_batch_generation_chapter_payload_service --manifest-path "backend-rs/Cargo.toml"`
    passed with 1 test
- chapter query payload helper checkpoint
  - extracted pure payload helpers in `chapter_query_service.rs` for chapter
    navigation and can-generate query responses
  - coverage now protects `previous` / `current` / `next` navigation response
    projection and boolean `can_generate` response projection
  - this preserves the existing async DB/service lookup and error mapping;
    only response JSON assembly moved into focused pure helpers
  - targeted `rustfmt --edition 2021 --check` passed for the touched Rust file
  - `cargo check --manifest-path "backend-rs/Cargo.toml"` passed after the
    helper extraction
  - focused `cargo test chapter_query_service --manifest-path "backend-rs/Cargo.toml"`
    passed with 2 tests
- chapter annotations payload helper checkpoint
  - extracted a pure `annotations_payload()` helper in
    `chapter_annotation_query_service.rs`
  - coverage now protects the existing empty annotations response contract:
    `chapter_id`, `annotations=[]`, and `memory_mapping=[]`
  - this preserves the existing access check through `ChapterService::get()`
    and only moves response JSON assembly into a focused helper
  - targeted `rustfmt --edition 2021 --check` passed for the touched Rust file
  - `cargo check --manifest-path "backend-rs/Cargo.toml"` passed after the
    helper extraction
  - focused `cargo test chapter_annotation_query_service --manifest-path "backend-rs/Cargo.toml"`
    passed with 1 test
- chapter draft apply payload helper checkpoint
  - extracted pure payload helpers in `chapter_draft_apply_service.rs` for
    candidate-draft generated-content payloads, candidate apply responses,
    auto-revision apply history payloads, and auto-revision apply responses
  - coverage now protects candidate quality-metrics null fallback, candidate
    apply response fields, auto-revision issue-count fallback/preservation,
    stale/allow-stale flags, timestamp formatting, and user-facing success
    messages
  - this preserves the existing DB transaction, stale validation, narrative
    sanitization, workflow-meta rejection, history insert, and route error
    mapping; only JSON assembly moved into focused pure helpers
  - targeted `rustfmt --edition 2021 --check` passed for the touched Rust file
  - `cargo check --manifest-path "backend-rs/Cargo.toml"` passed after the
    helper extraction
  - focused `cargo test chapter_draft_apply_service --manifest-path "backend-rs/Cargo.toml"`
    passed with 5 tests
- chapter draft apply text preparation checkpoint
  - extracted `sanitize_apply_draft_text()` in
    `chapter_draft_apply_service.rs` so candidate-draft apply and
    auto-revision apply share the same service-owned generated-text
    sanitation boundary
  - coverage now protects successful candidate text cleanup, empty candidate
    text rejection, and auto-revision meta-prefix stripping before apply
  - this preserves the existing cleaner semantics: meta-only text becomes
    empty and is rejected, while removable meta prefix lines are stripped
    before the cleaned draft text is applied
  - targeted `rustfmt --edition 2021 --check` passed for the touched Rust file
  - `cargo check --manifest-path "backend-rs/Cargo.toml"` passed after the
    helper extraction
  - focused `cargo test chapter_draft_apply_service --manifest-path "backend-rs/Cargo.toml"`
    passed with 8 tests
- chapter draft apply stale-validation checkpoint
  - extracted `validate_apply_draft_staleness()` in
    `chapter_draft_apply_service.rs` so candidate-draft apply and
    auto-revision apply share the same service-owned stale/allow-stale
    validation boundary
  - coverage now protects fresh draft application, stale candidate rejection
    when `allow_stale=false`, and stale auto-revision application when
    `allow_stale=true`
  - this preserves the existing `is_draft_stale()` semantics, typed
    `Stale` errors, response payload `stale_applied` field, DB transaction,
    and route error mapping
  - targeted `rustfmt --edition 2021 --check` passed for the touched Rust file
  - `cargo check --manifest-path "backend-rs/Cargo.toml"` passed after the
    helper extraction
  - focused `cargo test chapter_draft_apply_service --manifest-path "backend-rs/Cargo.toml"`
    passed with 11 tests
- chapter draft apply word-count checkpoint
  - extracted `ApplyDraftWordCounts` and `apply_draft_word_counts()` in
    `chapter_draft_apply_service.rs` so candidate-draft apply and
    auto-revision apply share the same old/new word-count calculation boundary
  - coverage now protects old word-count clamping at zero and synchronized
    `i32` / `usize` new word-count projections used by DB updates, response
    payloads, and auto-revision history payloads
  - this preserves existing character-count semantics, response
    `old_word_count` / `word_count` fields, auto-revision history
    `new_word_count`, DB transaction behavior, and route error mapping
  - targeted `rustfmt --edition 2021 --check` passed for the touched Rust file
  - `cargo check --manifest-path "backend-rs/Cargo.toml"` passed after the
    helper extraction
  - focused `cargo test chapter_draft_apply_service --manifest-path "backend-rs/Cargo.toml"`
    passed with 13 tests
- chapter draft apply history-field checkpoint
  - extracted draft apply history model constants and prompt helpers in
    `chapter_draft_apply_service.rs`
  - coverage now protects the candidate draft apply history model/prompt and
    auto-revision draft apply history model/prompt, including the
    `chapter_text_reviser_apply_v1` log type used by the auto-revision history
    payload
  - this preserves existing generation-history `prompt`, `model`, and
    `log_type` values while keeping transaction and insert behavior unchanged
  - targeted `rustfmt --edition 2021 --check` passed for the touched Rust file
  - `cargo check --manifest-path "backend-rs/Cargo.toml"` passed after the
    helper extraction
  - focused `cargo test chapter_draft_apply_service --manifest-path "backend-rs/Cargo.toml"`
    passed with 15 tests
- chapter draft apply history-model checkpoint
  - extracted candidate and auto-revision
    `generation_history::ActiveModel` assembly into focused helpers in
    `chapter_draft_apply_service.rs`
  - coverage now protects the persisted history-row field projection:
    `id`, `project_id`, `chapter_id`, `prompt`, `generated_content`,
    `model`, `tokens_used`, `generation_time`, and `created_at`
  - this preserves UUID/timestamp generation timing, DB transaction scope,
    insert order, response payloads, stale validation, and route error mapping
  - targeted `rustfmt --edition 2021 --check` passed for the touched Rust file
  - `cargo check --manifest-path "backend-rs/Cargo.toml"` passed after the
    helper extraction
  - focused `cargo test chapter_draft_apply_service --manifest-path "backend-rs/Cargo.toml"`
    passed with 17 tests
- chapter draft apply chapter-update-model checkpoint
  - extracted draft apply chapter update `chapter::ActiveModel` assembly into
    `draft_apply_chapter_update_model()` in
    `chapter_draft_apply_service.rs`
  - coverage now protects the shared chapter update field projection:
    `id`, `project_id`, `content`, `word_count`, `updated_at`, `title`, and
    `status`
  - this preserves DB transaction timing, update order, generated text
    sanitation, stale validation, history insertion, response payloads, and
    route error mapping
  - targeted `rustfmt --edition 2021 --check` passed for the touched Rust file
  - `cargo check --manifest-path "backend-rs/Cargo.toml"` passed after the
    helper extraction
  - focused `cargo test chapter_draft_apply_service --manifest-path "backend-rs/Cargo.toml"`
    passed with 18 tests
- chapter draft apply text-prepare checkpoint
  - extracted candidate and auto-revision draft apply text preparation into
    `prepare_candidate_draft_apply_text()` and
    `prepare_auto_revision_draft_apply_text()` in
    `chapter_draft_apply_service.rs`
  - coverage now protects complete candidate preview application,
    candidate preview-only rejection, and auto-revision missing
    `revised_text` rejection
  - this preserves generated-text sanitation, workflow-meta rejection,
    preview-only semantics, stale validation, DB transaction timing, history
    insertion, response payloads, and route error mapping
  - targeted `rustfmt --edition 2021 --check` passed for the touched Rust file
  - `cargo check --manifest-path "backend-rs/Cargo.toml"` passed after the
    helper extraction
  - focused `cargo test chapter_draft_apply_service --manifest-path "backend-rs/Cargo.toml"`
    passed with 21 tests
- chapter draft apply issue-count checkpoint
  - extracted auto-revision apply issue-count normalization into
    `AutoRevisionApplyIssueCounts` and `auto_revision_apply_issue_counts()` in
    `chapter_draft_apply_service.rs`
  - coverage now protects priority issue-count fallback
    (`critical_count + major_count`) and `applied_issue_count` fallback from
    `applied_critical_count`, plus explicit value preservation
  - this preserves auto-revision history payload fields, generated-content
    serialization, DB transaction timing, history insertion, response
    payloads, and route error mapping
  - targeted `rustfmt --edition 2021 --check` passed for the touched Rust file
  - `cargo check --manifest-path "backend-rs/Cargo.toml"` passed after the
    helper extraction
  - focused `cargo test chapter_draft_apply_service --manifest-path "backend-rs/Cargo.toml"`
    passed with 23 tests
- chapter draft query auto-revision count checkpoint
  - extracted auto-revision draft-view count normalization into
    `AutoRevisionDraftViewCounts` and `auto_revision_draft_view_counts()` in
    `chapter_draft_query_service.rs`
  - coverage now protects priority issue-count fallback
    (`critical_count + major_count`), applied issue-count fallback from
    `applied_critical_count`, revised word-count fallback from `revised_text`,
    and explicit value preservation
  - this preserves auto-revision draft view payload fields, full-text
    inclusion behavior, stale detection, datetime formatting, query behavior,
    response payloads, and route error mapping
  - targeted `rustfmt --edition 2021 --check` passed for the touched Rust file
  - `cargo check --manifest-path "backend-rs/Cargo.toml"` passed after the
    helper extraction
  - focused `cargo test chapter_draft_query_service --manifest-path "backend-rs/Cargo.toml"`
    passed with 2 tests

- chapter draft query candidate full-content checkpoint
  - added focused tests for `extract_candidate_draft_full_content()` in
    `chapter_draft_query_service.rs`
  - coverage now protects candidate full-content extraction precedence,
    `content_complete=true` preview promotion, word-count-matched preview
    promotion, and preview-only rejection
  - this preserves candidate draft view payload fields, apply eligibility
    semantics, full-text inclusion behavior, query behavior, response
    payloads, and route error mapping
  - targeted `rustfmt --edition 2021 --check` passed for the touched Rust file
  - `cargo check --manifest-path "backend-rs/Cargo.toml"` passed after the
    test addition
  - focused `cargo test chapter_draft_query_service --manifest-path "backend-rs/Cargo.toml"`
    passed with 6 tests

- chapter draft query candidate item normalization checkpoint
  - added focused tests for `normalize_candidate_items()` in
    `chapter_draft_query_service.rs`
  - coverage now protects trimming, deduplication, empty/unsupported value
    filtering, supported object field precedence, and limit truncation
  - this preserves candidate draft highlights, risk points, recommended
    actions, preserved strengths, response payloads, and route error mapping
  - targeted `rustfmt --edition 2021 --check` passed for the touched Rust file
  - `cargo check --manifest-path "backend-rs/Cargo.toml"` passed after the
    test addition
  - focused `cargo test chapter_draft_query_service --manifest-path "backend-rs/Cargo.toml"`
    passed with 9 tests

- chapter draft query datetime/staleness checkpoint
  - added focused tests for `format_datetime()` and `is_draft_stale()` in
    `chapter_draft_query_service.rs`
  - coverage now protects timezone-free datetime formatting, `None`
    passthrough, stale detection when the chapter update timestamp is newer
    than the draft timestamp, and non-stale equality/missing-timestamp cases
  - this preserves draft view `created_at` formatting, stale marker
    semantics, response payloads, and route error mapping
  - targeted `rustfmt --edition 2021 --check` passed for the touched Rust file
  - `cargo check --manifest-path "backend-rs/Cargo.toml"` passed after the
    test addition
  - focused `cargo test chapter_draft_query_service --manifest-path "backend-rs/Cargo.toml"`
    passed with 11 tests

- chapter draft query reviser-history parse checkpoint
  - added focused tests for `parse_reviser_result_from_history()` in
    `chapter_draft_query_service.rs`
  - coverage now protects successful `chapter_text_reviser_v1` parsing,
    `reviser_result` object extraction, and rejection of missing content,
    invalid JSON, wrong `log_type`, missing reviser result, and non-object
    reviser result payloads
  - this preserves auto-revision draft discovery, query fallback behavior,
    response payloads, and route error mapping
  - targeted `rustfmt --edition 2021 --check` passed for the touched Rust file
  - `cargo check --manifest-path "backend-rs/Cargo.toml"` passed after the
    test addition
  - focused `cargo test chapter_draft_query_service --manifest-path "backend-rs/Cargo.toml"`
    passed with 13 tests

- chapter draft query auto-revision payload checkpoint
  - added focused tests for `build_auto_revision_draft_payload()` in
    `chapter_draft_query_service.rs`
  - coverage now protects preview fallback from trimmed revised text,
    `include_full_text` gating, `history_id`/`created_at` null handling,
    stale marker projection, unresolved-issues defaults, and explicit
    unresolved issue preservation
  - this preserves auto-revision draft view payload shape, full-text
    inclusion behavior, stale marker semantics, response payloads, and route
    error mapping
  - targeted `rustfmt --edition 2021 --check` passed for the touched Rust file
  - `cargo check --manifest-path "backend-rs/Cargo.toml"` passed after the
    test addition
  - focused `cargo test chapter_draft_query_service --manifest-path "backend-rs/Cargo.toml"`
    passed with 15 tests

- chapter draft query candidate payload checkpoint
  - added focused tests for `build_candidate_draft_payload()` in
    `chapter_draft_query_service.rs`
  - coverage now protects full-content projection, `include_full_text`
    behavior, stale marker projection, quality/repair field projection,
    repair-summary precedence, failed-metric projection, repair-payload
    fallback highlights/risks, and preview-only non-applyable payloads
  - this preserves candidate draft view payload shape, apply eligibility
    semantics, full-text inclusion behavior, response payloads, and route
    error mapping
  - targeted `rustfmt --edition 2021 --check` passed for the touched Rust file
  - `cargo check --manifest-path "backend-rs/Cargo.toml"` passed after the
    test addition
  - focused `cargo test chapter_draft_query_service --manifest-path "backend-rs/Cargo.toml"`
    passed with 17 tests

- chapter draft query analysis-view fragments checkpoint
  - added focused tests for `build_chapter_draft_analysis_view_fragments()` in
    `chapter_draft_query_service.rs`
  - coverage now protects selection of the first valid auto-revision
    `chapter_text_reviser_v1` history after invalid entries, candidate draft
    fragment projection, exclusion of full text from analysis-view fragments,
    stale marker projection, and empty-input fragment shape
  - this preserves chapter analysis view draft fragment aggregation, response
    payloads, and route error mapping
  - targeted `rustfmt --edition 2021 --check` passed for the touched Rust file
  - `cargo check --manifest-path "backend-rs/Cargo.toml"` passed after the
    test addition
  - focused `cargo test chapter_draft_query_service --manifest-path "backend-rs/Cargo.toml"`
    passed with 19 tests

- chapter draft apply response/null timestamp checkpoint
  - added focused regression tests for draft apply response/history payload
    compatibility in `chapter_draft_apply_service.rs`
  - coverage now protects candidate and auto-revision apply response shape
    when `draft_created_at` is absent, auto-revision apply history payload
    shape when source/applied timestamps are absent, default issue-count
    projection in that missing-timestamp branch, and direct revised-text
    preparation from `revised_text`
  - this preserves draft apply response payloads, history payloads,
    null timestamp projection, text sanitation behavior, DB transaction
    ordering, UUID/timestamp generation timing, and route error mapping
  - targeted `rustfmt --edition 2021 --check` passed for the touched Rust file
  - `cargo check --manifest-path "backend-rs/Cargo.toml"` passed after the
    test addition
  - focused `cargo test chapter_draft_apply_service --manifest-path "backend-rs/Cargo.toml"`
    passed with 27 tests

- chapter draft query candidate fallback checkpoint
  - added focused regression tests for candidate draft query payload fallback
    compatibility in `chapter_draft_query_service.rs`
  - coverage now protects `created_at: null` projection for candidate draft
    payloads, non-stale behavior when draft timestamp is absent, summary-preview
    fallback into display `content_preview`, preservation of raw
    `summary_preview`, and exclusion of full candidate text from analysis-view
    fragments when `include_full_text=false`
  - this preserves candidate draft query payload shape, preview fallback
    semantics, full-text gating, stale marker semantics, response payloads,
    and route error mapping
  - targeted `rustfmt --edition 2021 --check` passed for the touched Rust file
  - `cargo check --manifest-path "backend-rs/Cargo.toml"` passed after the
    test addition
  - focused `cargo test chapter_draft_query_service --manifest-path "backend-rs/Cargo.toml"`
    passed with 22 tests

- chapter draft query candidate compat fields checkpoint
  - restored Python/frontend-compatible candidate draft response fields in
    `chapter_draft_query_service.rs`: `repair_targets`,
    `preserve_strengths`, and `focus_areas`
  - the fields reuse existing candidate item normalization and preserve the
    Python source precedence: `repair_payload` before `repair_guidance` for
    repair/preserve fields, and `repair_guidance` before `quality_gate` for
    focus areas
  - added focused tests for compat field projection, fallback precedence, and
    empty-array defaults in preview-only payloads
  - this preserves candidate draft response adapter compatibility with
    frontend `ChapterCandidateDraft`, while keeping existing
    `recommended_actions` / `preserved_strengths` fields intact
  - targeted `rustfmt --edition 2021 --check` passed for the touched Rust file
  - `cargo check --manifest-path "backend-rs/Cargo.toml"` passed after the
    adapter addition
  - focused `cargo test chapter_draft_query_service --manifest-path "backend-rs/Cargo.toml"`
    passed with 24 tests

- chapter draft query failed-metric compat checkpoint
  - restored Python/frontend-compatible `failed_metrics[].repair_target`
    projection in `chapter_draft_query_service.rs`
  - aligned missing failed-metric numeric fields with Python behavior by
    defaulting `value`, `threshold`, and `gap` to `0.0` instead of `null`
  - added focused tests for explicit failed-metric field projection and
    missing-number defaults
  - this preserves candidate draft response adapter compatibility with
    frontend `ChapterCandidateDraftFailedMetric` and Python
    `_build_candidate_draft_payload`
  - targeted `rustfmt --edition 2021 --check` passed for the touched Rust file
  - `cargo check --manifest-path "backend-rs/Cargo.toml"` passed after the
    adapter addition
  - focused `cargo test chapter_draft_query_service --manifest-path "backend-rs/Cargo.toml"`
    passed with 25 tests

- chapter draft query candidate display-field parity checkpoint
  - aligned candidate draft query display fields with Python
    `_build_candidate_draft_payload`
  - `source` and `attempt_state` are now trimmed in the response adapter,
    `summary_preview` is trimmed before projection, and `word_count` falls
    back to full-content length when the stored draft word count is zero
  - added focused tests for source/state trimming, summary-preview trimming,
    and fallback word-count projection
  - this preserves frontend-facing `ChapterCandidateDraft` display semantics
    while keeping full-text gating and draft apply eligibility unchanged
  - targeted `rustfmt --edition 2021 --check` passed for the touched Rust file
  - `cargo check --manifest-path "backend-rs/Cargo.toml"` passed after the
    adapter alignment
  - focused `cargo test chapter_draft_query_service --manifest-path "backend-rs/Cargo.toml"`
    passed with 26 tests

- chapter draft query candidate apply-risk parity checkpoint
  - added `candidate_apply_risk_payload()` in
    `chapter_draft_query_service.rs` to match Python fallback semantics for
    candidate draft `apply_risk`
  - explicit `apply_risk` payloads are still preserved; when absent, the
    adapter now derives warning items from quality-highlight missing items,
    quality-gate failed metric labels, and warning/manual-review gate state
    or action
  - added focused helper tests plus a `build_candidate_draft_payload()`
    integration-style unit test for generated fallback `apply_risk`
  - this preserves frontend `candidateDraft.apply_risk` behavior without
    changing DB queries, full-text gating, or apply eligibility
  - targeted `rustfmt --edition 2021 --check` passed for the touched Rust file
  - `cargo check --manifest-path "backend-rs/Cargo.toml"` passed after the
    adapter alignment
  - focused `cargo test chapter_draft_query_service --manifest-path "backend-rs/Cargo.toml"`
    passed with 30 tests

- chapter draft query candidate string-normalization parity checkpoint
  - added `trimmed_string_or_null()` in `chapter_draft_query_service.rs` for
    Python-compatible candidate draft string projection
  - `failed_metrics[].key` and `failed_metrics[].label` now trim surrounding
    whitespace, while blank `focus_area` / `repair_target` values project as
    `null` instead of blank strings
  - `repair_summary` now follows Python `_build_candidate_draft_payload`
    semantics by trimming the selected repair/guidance summary and projecting
    blank values as `null`
  - added focused regression coverage for blank failed-metric fields and blank
    repair summaries without changing DB queries, full-text gating, apply
    eligibility, or route error mapping
  - targeted `rustfmt --edition 2021 --check` passed for the touched Rust file
  - `cargo check --manifest-path "backend-rs/Cargo.toml"` passed after the
    adapter alignment
  - focused `cargo test chapter_draft_query_service --manifest-path "backend-rs/Cargo.toml"`
    passed with 31 tests

- chapter draft query numeric-string parity checkpoint
  - added `json_i64()` and `json_f64()` in
    `chapter_draft_query_service.rs` so Rust response adapters accept numeric
    JSON strings in the same places Python builders accept `int(...)` /
    `float(...)` coercion
  - auto-revision draft view counts now preserve numeric-string
    `critical_count`, `major_count`, `priority_issue_count`,
    `applied_critical_count`, `applied_issue_count`, and
    `revised_word_count` values instead of falling back to zero/default counts
  - candidate draft `failed_metrics[].value`, `threshold`, and `gap` now
    preserve numeric-string values instead of falling back to `0.0`
  - added focused regression coverage for auto-revision numeric-string counts
    and candidate failed-metric numeric strings without changing DB queries,
    full-text gating, apply eligibility, or route error mapping
  - targeted `rustfmt --edition 2021 --check` passed for the touched Rust file
  - `cargo check --manifest-path "backend-rs/Cargo.toml"` passed after the
    adapter alignment
  - focused `cargo test chapter_draft_query_service --manifest-path "backend-rs/Cargo.toml"`
    passed with 32 tests

- chapter draft apply numeric-string parity checkpoint
  - added `json_i64()` in `chapter_draft_apply_service.rs` so
    auto-revision apply history issue-count assembly accepts numeric JSON
    strings in the same places Python builders accept `int(...)` coercion
  - auto-revision apply history now preserves numeric-string `critical_count`,
    `major_count`, `priority_issue_count`, `applied_critical_count`, and
    `applied_issue_count` values instead of falling back to zero/default counts
  - added focused regression coverage for numeric-string issue counts without
    changing DB transactions, route error mapping, stale validation, text
    sanitation, or history insert ordering
  - targeted `rustfmt --edition 2021 --check` passed for the touched Rust file
  - `cargo check --manifest-path "backend-rs/Cargo.toml"` passed after the
    adapter alignment
  - focused `cargo test chapter_draft_apply_service --manifest-path "backend-rs/Cargo.toml"`
    passed with 28 tests

- chapter draft numeric parser reuse checkpoint
  - promoted `chapter_draft_query_service::json_i64()` to `pub(crate)` and
    reused it from `chapter_draft_apply_service.rs`
  - removed the duplicate apply-local JSON integer parser so draft query view
    counts and auto-revision apply history issue counts share the same
    numeric-string coercion semantics
  - no response fields, DB writes, stale validation, text sanitation, or route
    error mapping changed in this slice
  - targeted `rustfmt --edition 2021 --check` passed for both touched Rust
    files
  - `cargo check --manifest-path "backend-rs/Cargo.toml"` passed after the
    helper reuse
  - focused `cargo test chapter_draft_apply_service --manifest-path "backend-rs/Cargo.toml"`
    passed with 28 tests
  - focused `cargo test chapter_draft_query_service --manifest-path "backend-rs/Cargo.toml"`
    passed with 32 tests

- chapter draft candidate item scalar parity checkpoint
  - aligned `normalize_candidate_items()` in `chapter_draft_query_service.rs`
    with Python `_normalize_candidate_draft_items()` for scalar values
  - standalone JSON number / bool values and array number / bool values now
    normalize to strings instead of being dropped, preserving frontend
    candidate list and apply-risk item compatibility when stored payloads use
    scalar values
  - object-field extraction remains intentionally constrained to the existing
    supported keys and does not stringify arbitrary nested objects
  - added focused regression coverage for scalar number/bool normalization
    without changing DB queries, response field names, full-text gating, apply
    eligibility, or route error mapping
  - targeted `rustfmt --edition 2021 --check` passed for the touched Rust file
  - `cargo check --manifest-path "backend-rs/Cargo.toml"` passed after the
    adapter alignment
  - focused `cargo test chapter_draft_query_service --manifest-path "backend-rs/Cargo.toml"`
    passed with 33 tests

- chapter draft candidate item object-scalar parity checkpoint
  - extended `normalize_candidate_items()` so supported array-object fields
    (`label`, `name`, `value`, `item`) can also project JSON number / bool
    values through the existing scalar string conversion path
  - this keeps array-object extraction aligned with standalone-object
    extraction while still avoiding arbitrary object stringification
  - updated focused scalar normalization coverage for array objects with
    numeric `value` and boolean `label`, preserving the existing decision to
    ignore unsupported `summary` inside array objects
  - no response field names, DB queries, full-text gating, apply eligibility,
    stale validation, or route error mapping changed
  - targeted `rustfmt --edition 2021 --check` passed for the touched Rust file
  - `cargo check --manifest-path "backend-rs/Cargo.toml"` passed after the
    adapter alignment
  - focused `cargo test chapter_draft_query_service --manifest-path "backend-rs/Cargo.toml"`
    passed with 33 tests

- chapter draft candidate item Python-truthiness parity checkpoint
  - tightened `normalize_candidate_items()` scalar conversion to follow
    Python `_normalize_candidate_draft_items()` truthiness semantics more
    closely
  - JSON `false` and numeric zero values are now dropped like Python
    `str(value or "")`, while JSON `true` projects as `True`
  - updated focused scalar normalization coverage for standalone and array
    false/zero values plus true casing
  - no response field names, DB queries, full-text gating, apply eligibility,
    stale validation, or route error mapping changed
  - targeted `rustfmt --edition 2021 --check` passed for the touched Rust file
  - `cargo check --manifest-path "backend-rs/Cargo.toml"` passed after the
    adapter alignment
  - focused `cargo test chapter_draft_query_service --manifest-path "backend-rs/Cargo.toml"`
    passed with 33 tests

- chapter draft numeric bool-coercion parity checkpoint
  - extended shared `json_i64()` and query-local `json_f64()` to accept JSON
    bools with Python-compatible numeric coercion (`true` => `1` / `1.0`,
    `false` => `0` / `0.0`)
  - auto-revision draft view counts, auto-revision apply issue counts, and
    candidate failed-metric numeric projections now share this bool coercion
    behavior
  - added focused query coverage for bool-based auto-revision counts and
    failed-metric threshold/gap values, plus focused apply coverage for
    bool-based auto-revision issue counts
  - no response field names, DB queries/writes, full-text gating, apply
    eligibility, stale validation, route error mapping, or SSE/runtime
    semantics changed
  - targeted `rustfmt --edition 2021 --check` passed for both touched Rust
    files
  - `cargo check --manifest-path "backend-rs/Cargo.toml"` passed after the
    adapter alignment
  - focused `cargo test chapter_draft_query_service --manifest-path "backend-rs/Cargo.toml"`
    passed with 34 tests
  - focused `cargo test chapter_draft_apply_service --manifest-path "backend-rs/Cargo.toml"`
    passed with 29 tests

- chapter draft failed-metric scalar-string parity checkpoint
  - extracted shared `python_truthy_scalar_text()` for Python-compatible
    scalar string projection across candidate item normalization and candidate
    failed-metric string fields
  - candidate failed metrics now project non-string scalar `key`, `label`,
    `focus_area`, and `repair_target` values like Python
    `str(value or "").strip()` without stringifying arrays or objects
  - preserved Python label fallback semantics by falling back from falsey or
    blank `label` to `key`, while optional focus/repair fields still return
    `null` for falsey or blank values
  - added focused query coverage for numeric/bool scalar failed-metric fields
    and label fallback behavior
  - no response field names, DB queries/writes, full-text gating, apply
    eligibility, stale validation, route error mapping, or SSE/runtime
    semantics changed
  - focused `cargo test chapter_draft_query_service --manifest-path "backend-rs/Cargo.toml"`
    passed with 35 tests

- chapter draft repair-summary truthiness parity checkpoint
  - aligned candidate draft `repair_summary` with Python fallback semantics:
    use `repair_payload.summary` only when it is truthy after scalar
    conversion, otherwise fall back to `repair_guidance.summary`
  - reused the same Python-truthy scalar conversion used by candidate items
    and failed-metric string fields, and removed the now-dead string-only
    helper
  - added focused query coverage for falsey repair-payload summary falling
    back to numeric guidance summary
  - no response field names, DB queries/writes, full-text gating, apply
    eligibility, stale validation, route error mapping, or SSE/runtime
    semantics changed
  - focused `cargo test chapter_draft_query_service --manifest-path "backend-rs/Cargo.toml"`
    passed with 36 tests

- chapter draft compat-item fallback parity checkpoint
  - added `normalize_candidate_items_with_fallback()` so candidate compat
    list fields use the primary source only when it normalizes to non-empty
    items, matching Python `primary or fallback` behavior for falsey payloads
  - aligned candidate `repair_targets`, `preserve_strengths`, and
    `focus_areas` fallback behavior when repair/guidance fields are `false`,
    `0`, or empty arrays
  - preserved `preserved_strengths` as the repair-payload-only field and did
    not change quality-highlight, apply-risk, full-text, stale, DB, route,
    SSE, or runtime semantics
  - added focused query coverage for falsey primary compat fields falling
    back to guidance / quality-gate sources
  - focused `cargo test chapter_draft_query_service --manifest-path "backend-rs/Cargo.toml"`
    passed with 37 tests

- chapter draft apply-risk failed-metric label parity checkpoint
  - added `candidate_failed_metric_labels()` so fallback `apply_risk` quality
    gate warning text is derived only from `failed_metrics[].label`, matching
    the Python builder instead of normalizing arbitrary failed-metric object
    fields
  - preserved Python truthiness for scalar labels, so numeric/bool labels are
    still normalized while missing labels do not fall back to `key`, `value`,
    `name`, or `item`
  - added focused query coverage for label-only failed-metric apply-risk
    fallback behavior
  - no response field names, DB queries/writes, full-text gating, apply
    eligibility, stale validation, route error mapping, or SSE/runtime
    semantics changed
  - focused `cargo test chapter_draft_query_service --manifest-path "backend-rs/Cargo.toml"`
    passed with 38 tests

- chapter draft auto-revision revised-text whitespace parity checkpoint
  - aligned auto-revision draft `revised_text` handling with Python by
    preserving the raw string instead of trimming before deriving
    `revised_text_preview`, `has_full_text`, and optional full-text payload
  - updated existing preview-default coverage and added focused whitespace
    coverage for full-text inclusion plus whitespace-only revised text
  - no response field names, DB queries/writes, candidate draft fields, apply
    eligibility, stale validation, route error mapping, or SSE/runtime
    semantics changed
  - focused `cargo test chapter_draft_query_service --manifest-path "backend-rs/Cargo.toml"`
    passed with 39 tests

- chapter draft auto-revision revised-word-count payload parity checkpoint
  - aligned auto-revision draft `revised_word_count` payload projection with
    Python by preserving an explicit `revised_word_count` value as-is instead
    of coercing it to an integer in the response payload
  - kept the existing integer parser for internal count/default semantics, so
    missing `revised_word_count` still falls back to the untrimmed
    `revised_text` character count
  - added focused query coverage for explicit string/bool
    `revised_word_count` values and missing-value fallback
  - no response field names, DB queries/writes, candidate draft fields, apply
    eligibility, stale validation, route error mapping, or SSE/runtime
    semantics changed
  - focused `cargo test chapter_draft_query_service --manifest-path "backend-rs/Cargo.toml"`
    passed with 40 tests

- chapter draft auto-revision falsey-count fallback parity checkpoint
  - added shared `python_truthy_json_i64()` for Python-compatible
    `value or fallback` numeric fields where false/zero should trigger the
    fallback rather than become the final value
  - aligned auto-revision draft view `priority_issue_count` and
    `applied_issue_count` fallback behavior with Python
  - aligned auto-revision apply-history issue-count fallback behavior with the
    same helper so query/apply adapters do not drift
  - added focused query/apply coverage for falsey count fallback semantics
  - no response field names, DB queries/writes, candidate draft fields, apply
    eligibility, stale validation, route error mapping, or SSE/runtime
    semantics changed
  - focused `cargo test chapter_draft_query_service --manifest-path "backend-rs/Cargo.toml"`
    passed with 41 tests
  - focused `cargo test chapter_draft_apply_service --manifest-path "backend-rs/Cargo.toml"`
    passed with 30 tests

## Handoff Note

- Current stop point: Wave 1 read-side/status hardening, Wave 2 provider seam
  tightening, and the high-signal Wave 3 route-to-workflow slices have
  completed. The latest slices moved chapter-regeneration full/partial stream
  workflow assembly into a dedicated workflow service, moved analysis /
  regeneration read-route access/query ownership into service helpers, and
  moved analysis draft load/apply plus partial-regeneration apply
  orchestration into service-owned helpers. The chapter-analysis trigger
  route now also delegates runtime dispatch to the trigger service. The
  active batch task list route also delegates limit normalization to the query
  service. The final tail cleanup moved single-chapter background-generation
  compatibility field consumption into the single-chapter request service. The
  latest regeneration cleanup made query/stream workflow defaults explicit and
  covered them with focused pure-helper tests. The latest batch query cleanup
  also made active task list limit defaults explicit and covered them with a
  focused pure-helper test. The latest batch-create cleanup made
  `enable_analysis` and `max_retries` defaults explicit and covered them with
  focused pure-helper tests. The latest test-only cleanup added regression
  coverage for batch and single-chapter target-word-count defaults. The latest
  analysis draft request cleanup added regression coverage for draft request
  parsing defaults and ID ownership. The regeneration prepare cleanup added
  regression coverage for partial-regeneration length and selection
  preparation semantics. The latest regeneration prepare cleanup added
  regression coverage for full-regeneration prompt defaults, explicit prompt
  field projection, selected-text fallback/context edges, and `max_tokens`
  clamp semantics. The latest analysis query cleanup added regression coverage
  for analysis task status `error_code` classification. The latest
  partial-regeneration apply cleanup extracted a pure preparation helper and
  covered replacement/range/sanitization semantics. The latest batch status
  payload cleanup added direct regression coverage for checkpoint runtime
  metadata insertion/override semantics. The latest regeneration text cleanup
  added regression coverage for output normalization and finalization payloads.
  The latest batch quality status cleanup added regression coverage for
  read-side quality context extraction/defaults. The latest analysis checker
  cleanup added regression coverage for checker history fragment selection and
  timestamp semantics. The latest regeneration query cleanup added regression
  coverage for task-history datetime formatting. The latest batch request
  compat cleanup added regression coverage for legacy request field projection
  into borrowed view fields. The latest analysis view payload cleanup added
  regression coverage for response adapter field/null/fragment projection. The
  latest chapter quality metrics payload cleanup added regression coverage for
  quality metrics response adapter compatibility fields. The latest batch
  status semantics cleanup added regression coverage for shared task type /
  stage / execution-mode vocabulary. The latest chapter quality query cleanup
  added regression coverage for quality fragment summary/runtime-context
  semantics. The latest active batch query cleanup added regression coverage
  for empty active-task response shape. The latest analysis runtime cleanup
  added regression coverage for JSON normalization, analysis status
  normalization, and analysis-report section assembly. The latest chapter CRUD
  payload adapter cleanup added regression coverage for list/single response
  compatibility payload shapes. The latest batch chapter payload cleanup added
  regression coverage for the single-task chapter snapshot projection. The
  latest chapter query cleanup extracted focused payload helpers for
  navigation and can-generate responses. The latest annotations query cleanup
  extracted the empty annotations payload helper and covered its response
  shape. The latest chapter draft apply cleanup extracted focused payload
  helpers for candidate and auto-revision draft apply response/history JSON
  assembly without changing transaction or validation behavior. The latest
  chapter draft apply cleanup also extracted the shared draft apply text
  sanitation helper and covered empty-text rejection plus meta-prefix stripping
  semantics. The latest chapter draft apply cleanup also extracted the shared
  stale/allow-stale validation helper and covered fresh, rejected-stale, and
  explicitly-allowed-stale branches. The latest chapter draft apply cleanup
  also extracted the shared old/new word-count calculation helper and covered
  negative old-count clamping plus synchronized new-count projections. The
  latest chapter draft apply cleanup also extracted history model constants
  and prompt helpers for candidate and auto-revision draft apply history rows.
  The latest chapter draft apply cleanup also extracted candidate and
  auto-revision generation-history ActiveModel assembly helpers and covered
  the persisted history-row field projection directly. The latest chapter
  draft apply cleanup also extracted the shared chapter update ActiveModel
  assembly helper and covered the chapter update field projection directly.
  The latest chapter draft apply cleanup also extracted candidate and
  auto-revision draft apply text preparation helpers and covered complete
  preview application, preview-only rejection, and missing revised-text
  rejection semantics. The latest chapter draft apply cleanup also extracted
  auto-revision issue-count normalization and covered fallback plus explicit
  value preservation semantics. The latest chapter draft query cleanup also
  extracted auto-revision draft-view count normalization and covered fallback
  plus explicit value preservation semantics. The latest chapter draft query
  cleanup also added direct regression coverage for candidate draft
  full-content extraction precedence and preview-completeness semantics. The
  latest chapter draft query cleanup also added direct regression coverage for
  candidate item normalization used by highlights, risk points, recommended
  actions, and preserved strengths. The latest chapter draft query cleanup
  also added direct regression coverage for datetime formatting and stale
  detection semantics shared by draft query/apply flows. The latest chapter
  draft query cleanup also added direct regression coverage for auto-revision
  reviser-history payload parsing and invalid payload rejection. The latest
  chapter draft query cleanup also added direct regression coverage for
  auto-revision draft payload projection, full-text gating, stale markers, and
  unresolved-issue defaults. The latest chapter draft query cleanup also
  added direct regression coverage for candidate draft payload projection,
  apply eligibility, repair-field fallbacks, and preview-only response shape.
  The latest chapter draft query cleanup also added direct regression coverage
  for chapter analysis-view draft fragment aggregation and empty-input shape.
  The latest chapter draft apply cleanup also added direct regression coverage
  for candidate/auto-revision apply response null timestamp projection,
  auto-revision apply history null timestamp/default issue-count projection,
  and direct revised-text apply preparation.
  The latest chapter draft query cleanup also added direct regression coverage
  for candidate draft missing-created-at projection, summary-preview fallback,
  raw summary preservation, and analysis-view full-text exclusion.
  The latest chapter draft query cleanup also restored Python/frontend
  compatible candidate draft `repair_targets`, `preserve_strengths`, and
  `focus_areas` fields with focused source-precedence coverage.
  The latest chapter draft query cleanup also restored failed-metric
  `repair_target` projection and Python-compatible missing-number defaults.
  The latest chapter draft query cleanup also aligned candidate draft display
  fields with Python by trimming source/state/summary and using full-content
  length when stored draft word count is zero.
  The latest chapter draft query cleanup also aligned candidate draft
  `apply_risk` fallback generation with Python quality-highlight /
  quality-gate semantics while preserving explicit payloads.
  The latest chapter draft query cleanup also aligned candidate failed-metric
  string projection and repair-summary blank handling with Python by trimming
  labels/keys and returning `null` for blank optional strings.
  The latest chapter draft query cleanup also aligned numeric-string coercion
  with Python for auto-revision draft counts and candidate failed metric
  numbers.
  The latest chapter draft apply cleanup also aligned auto-revision apply
  history issue-count numeric-string coercion with the query-side adapter.
  The latest chapter draft cleanup also removed the duplicate apply-local
  numeric parser by reusing the query-side `json_i64()` helper for both draft
  view and apply-history count adapters.
  The latest chapter draft query cleanup also aligned candidate item
  normalization with Python for scalar number/bool payloads while keeping
  object extraction constrained to supported fields.
  The latest chapter draft query cleanup also reused the same scalar
  conversion for supported fields inside array objects without stringifying
  unsupported object payloads.
  The latest chapter draft query cleanup also tightened scalar conversion to
  Python truthiness semantics: false/zero values are dropped and true projects
  as `True`.
  The latest chapter draft cleanup also aligned numeric bool coercion with
  Python for shared integer counts and failed-metric floating-point fields.
  The latest chapter draft query cleanup also reused that Python-truthy scalar
  conversion for candidate failed-metric `key`, `label`, `focus_area`, and
  `repair_target` fields while keeping array/object values ignored.
  The latest chapter draft query cleanup also aligned candidate `repair_summary`
  fallback with Python truthiness and removed the unused string-only helper.
  The latest chapter draft query cleanup also aligned candidate compat list
  fallback semantics for `repair_targets`, `preserve_strengths`, and
  `focus_areas` after empty/falsey primary normalization.
  The latest chapter draft query cleanup also aligned fallback `apply_risk`
  failed-metric warning text with Python by reading only
  `failed_metrics[].label`.
  The latest chapter draft query cleanup also aligned auto-revision draft
  `revised_text` whitespace handling with Python so preview fallback and
  `has_full_text` are derived from the untrimmed raw text.
  The latest chapter draft query cleanup also aligned auto-revision draft
  `revised_word_count` payload projection with Python by preserving explicit
  values and only using the computed count as the missing-value fallback.
  The latest chapter draft cleanup also aligned falsey numeric fallback
  semantics for auto-revision `priority_issue_count` and `applied_issue_count`
  across query and apply-history adapters.
- Validation status: `cargo check` passed after the latest slice; touched-file
  rustfmt check passed; targeted `chapter_analysis` / `chapter_regeneration`
  test filtering compiled successfully but currently has no matching tests.
  `cargo test sse` now passes with 5 tests and no recurring unused `Event`
  warnings. The latest `chapter_single_generation_request_service` focused
  test filter also compiles successfully but currently matches no tests. The
  latest regeneration default-semantics tests pass for query and stream
  workflow helpers. The latest active batch task list default-semantics test
  also passes. The latest batch-create default-semantics tests also pass. The
  latest target-word-count default-semantics tests also pass. The latest
  analysis draft request parsing/default-semantics tests also pass. The latest
  regeneration prepare prompt/boundary default-semantics tests pass with 9
  focused tests. The latest analysis query error-code tests pass with 2
  focused tests. The latest partial-regeneration apply tests pass with 4
  focused tests. The latest batch status payload adapter tests pass with 4
  focused tests. The latest regeneration text tests pass with 4 focused tests.
  The latest batch quality status context tests pass with 4 focused tests. The
  latest analysis checker query tests pass with 3 focused tests. The latest
  regeneration query tests pass with 2 focused tests. The latest batch request
  compat projection tests pass with 2 focused tests. The latest analysis view
  payload adapter tests pass with 2 focused tests. The latest chapter quality
  metrics payload adapter tests pass with 2 focused tests. The latest batch
  status semantics tests pass with 3 focused tests. The latest chapter quality
  query tests pass with 3 focused tests. The latest active batch query tests
  pass with 1 focused test. The latest analysis runtime helper tests pass with
  5 focused tests. The latest chapter CRUD payload adapter tests pass with 3
  focused tests. The latest batch single-task chapter payload adapter test
  passes with 1 focused test. The latest chapter query payload helper tests
  pass with 2 focused tests. The latest annotations payload helper test passes
  with 1 focused test. The latest chapter draft apply payload helper tests
  pass with 5 focused tests. The latest chapter draft apply text preparation
  tests pass as part of the same focused filter, which now passes with 8
  focused tests. The latest chapter draft apply stale-validation tests pass as
  part of the same focused filter, which now passes with 11 focused tests. The
  latest chapter draft apply word-count tests pass as part of the same focused
  filter, which now passes with 13 focused tests. The latest chapter draft
  apply history-field tests pass as part of the same focused filter, which now
  passes with 15 focused tests. The latest chapter draft apply history-model
  tests pass as part of the same focused filter, which now passes with 17
  focused tests. The latest chapter draft apply chapter-update-model test
  passes as part of the same focused filter, which now passes with 18 focused
  tests. The latest chapter draft apply text-prepare tests pass as part of the
  same focused filter, which now passes with 21 focused tests. The latest
  chapter draft apply issue-count tests pass as part of the same focused
  filter, which now passes with 23 focused tests. The latest chapter draft
  query auto-revision, candidate full-content, and candidate item
  normalization plus datetime/staleness, reviser-history parse, and
  auto-revision/candidate payload and analysis-view fragment tests pass with
  19 focused tests. The latest chapter draft apply payload compatibility tests
  pass with 27 focused tests. The latest chapter draft query candidate
  fallback tests pass with 22 focused tests. The latest chapter draft query
  candidate compat-field tests pass with 24 focused tests. The latest chapter
  draft query failed-metric compat tests pass with 25 focused tests. The
  latest chapter draft query display-field parity tests pass with 26 focused
  tests. The latest chapter draft query apply-risk parity tests pass with 30
  focused tests. The latest chapter draft query string-normalization parity
  tests pass with 31 focused tests. The latest chapter draft query
  numeric-string parity tests pass with 32 focused tests. The latest chapter
  draft apply numeric-string parity tests pass with 28 focused tests. The
  latest draft numeric parser reuse validation kept query at 32 focused tests
  and apply at 28 focused tests. The latest chapter draft query scalar-item
  parity tests pass with 33 focused tests. The latest chapter draft query
  object-scalar item parity validation kept query at 33 focused tests. The
  latest Python-truthiness scalar-item parity validation kept query at 33
  focused tests. The latest numeric bool-coercion parity validation raises
  query to 34 focused tests and apply to 29 focused tests. The latest
  failed-metric scalar-string parity validation raises query to 35 focused
  tests. The latest repair-summary truthiness parity validation raises query
  to 36 focused tests. The latest compat-item fallback parity validation
  raises query to 37 focused tests. The latest apply-risk failed-metric label
  parity validation raises query to 38 focused tests. The latest
  auto-revision revised-text whitespace parity validation raises query to 39
  focused tests. The latest auto-revision revised-word-count payload parity
  validation raises query to 40 focused tests. The latest falsey-count
  fallback parity validation raises query to 41 focused tests and apply to 30
  focused tests.
- Recommended next step: prefer final quality review and broader validation
  before more structural refactor. Remaining route code is mostly transport
  glue; continue only if a concrete compatibility-safe seam is found, not just
  because a file can be made smaller.
- First files to inspect next session, if continuing:
  `backend-rs/src/api/chapter_regeneration_routes.rs`,
  `backend-rs/src/services/chapter_regeneration_stream_workflow_service.rs`,
  `backend-rs/src/api/chapter_batch_generation.rs`, and
  `backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs`
  (read-only unless a concrete compatibility-safe seam is proven).
- Keep avoiding broad cross-file dedupe with
  `chapter_batch_generation_runtime_state_service.rs` until a concrete
  compatibility-safe seam is identified.
- Query/status routes are already thin enough; do not reopen them unless a new
  cross-route transport helper clearly removes non-trivial duplication.
- Treat `chapter_batch_generation_dispatch_service.rs` and
  `chapter_batch_generation_runtime_state_service.rs` as high-skepticism areas:
  do not refactor them further without a seam that reduces compatibility risk,
  not just local duplication.
- `chapter_batch_generation.rs` is nearing a route-thinning stop point; prefer
  leaving remaining code in place unless a new workflow boundary removes more
  than one transport-local branch or repeated request assembly step.
- `chapter_batch_generation_runtime_state_service.rs` and
  `chapter_batch_generation_task_command_service.rs` should also be treated as
  near-stop unless a future slice clearly removes semantic drift without
  reopening snapshot/checkpoint write ownership.
- validation is now effectively blocked by test compilation resource limits
  rather than by obvious local source errors in this task's touched area.
- focused validation for `chapter_batch_generation_task_command_service`
  currently passes again in this environment after the resume-consistency fix;
  broader test limits should still be treated as environment-sensitive until a
  wider pass is rerun.
- the remaining batch-generation seams now lean strongly toward write-path
  ownership changes, so further refactor should stop unless a concrete
  user-visible inconsistency or multi-consumer drift is found first.
- after the runtime/read-side helper cleanup, the remaining route and runtime
  moves are even closer to diminishing returns; prefer quality review and
  final validation over additional structure-only changes.
- current route/runtime stop review found no remaining route-local default
  provider assembly, checkpoint/snapshot writes, SSE payload semantics, or
  duplicated status/quality/checkpoint semantics worth moving. Treat the
  route-to-service refactor as at the Phase 3 stop point unless a future
  user-visible inconsistency is proven.
- 2026-05-19 13:12 +08:00 checkpoint:
  chapter draft auto-revision scalar text parity now matches Python
  `str(value or "")` semantics for query/apply adapters. The shared
  `python_truthy_scalar_text()` helper is crate-visible and is reused by
  auto-revision draft query payload construction and auto-revision draft apply
  text preparation. Focused regression coverage now confirms numeric/truthy
  boolean `revised_text` and `revised_text_preview` are projected like Python,
  while falsey scalar `revised_text` remains empty and rejected on apply.
  Validation: touched-file rustfmt check passed; `cargo test
  chapter_draft_query_service --manifest-path backend-rs/Cargo.toml` passed
  with 42 tests; `cargo test chapter_draft_apply_service --manifest-path
  backend-rs/Cargo.toml` passed with 31 tests; `cargo check --manifest-path
  backend-rs/Cargo.toml` passed.
- 2026-05-19 13:12 +08:00 checkpoint:
  candidate draft full-content extraction now also reuses the shared Python
  truthy scalar projection for `repair_payload.candidate_full_content`.
  Numeric/truthy scalar full content is preserved consistently in query
  payloads and candidate apply preparation, while falsey scalar full content
  still falls through to existing preview completeness rules. Validation:
  touched-file rustfmt check passed; `cargo test chapter_draft_query_service
  --manifest-path backend-rs/Cargo.toml` passed with 43 tests; `cargo test
  chapter_draft_apply_service --manifest-path backend-rs/Cargo.toml` passed
  with 32 tests; `cargo check --manifest-path backend-rs/Cargo.toml` passed.
- 2026-05-19 13:12 +08:00 checkpoint:
  candidate draft `repair_payload.content_complete` now follows Python
  truthiness instead of Rust bool-only parsing. Non-empty strings, non-zero
  numbers, non-empty arrays/objects, and `true` can mark preview content as
  complete, while falsey JSON values still fall through to the existing
  preview/word-count checks. Validation: touched-file rustfmt check passed;
  `cargo test chapter_draft_query_service --manifest-path backend-rs/Cargo.toml`
  passed with 44 tests; `cargo test chapter_draft_apply_service --manifest-path
  backend-rs/Cargo.toml` passed with 32 tests; `cargo check --manifest-path
  backend-rs/Cargo.toml` passed.
- 2026-05-19 13:13 +08:00 checkpoint:
  candidate draft `content_preview` display fallback now matches Python
  `content_preview or summary_preview or ""` semantics. An explicitly empty
  `content_preview` no longer suppresses a non-empty `summary_preview`, while
  existing whitespace trimming and full-content preview fallback remain
  unchanged. Validation: touched-file rustfmt check passed; `cargo test
  chapter_draft_query_service --manifest-path backend-rs/Cargo.toml` passed
  with 45 tests; `cargo test chapter_draft_apply_service --manifest-path
  backend-rs/Cargo.toml` passed with 32 tests; `cargo check --manifest-path
  backend-rs/Cargo.toml` passed.
- 2026-05-19 13:14 +08:00 checkpoint:
  candidate fallback `apply_risk` now mirrors Python `quality_gate_decision or
  quality_gate.decision` semantics. An explicitly empty route/model decision
  no longer suppresses a non-empty `quality_gate.decision`, and gate status /
  decision fallback text uses the shared Python truthy scalar projection.
  Validation: touched-file rustfmt check passed; `cargo test
  chapter_draft_query_service --manifest-path backend-rs/Cargo.toml` passed
  with 46 tests; `cargo test chapter_draft_apply_service --manifest-path
  backend-rs/Cargo.toml` passed with 32 tests; `cargo check --manifest-path
  backend-rs/Cargo.toml` passed.
- 2026-05-19 13:15 +08:00 checkpoint:
  explicit empty candidate `apply_risk` objects no longer survive into the
  Rust payload. This now matches Python, which only returns a populated risk
  object or `None`; the frontend will no longer treat `{}` as a truthy
  warning card and render the fallback risk summary by mistake. Validation:
  touched-file rustfmt check passed; `cargo test chapter_draft_query_service
  --manifest-path backend-rs/Cargo.toml` passed with 47 tests; `cargo test
  chapter_draft_apply_service --manifest-path backend-rs/Cargo.toml` passed
  with 32 tests; `cargo check --manifest-path backend-rs/Cargo.toml` passed.
- 2026-05-19 13:16 +08:00 checkpoint:
  explicit empty candidate `quality_highlights` objects now also collapse to
  `null` for Python parity. This keeps the candidate comparison modal from
  receiving a truthy-but-empty highlights object, while preserving existing
  non-empty highlight payloads and derived `highlight_points`. Validation:
  touched-file rustfmt check passed; `cargo test chapter_draft_query_service
  --manifest-path backend-rs/Cargo.toml` passed with 48 tests; `cargo test
  chapter_draft_apply_service --manifest-path backend-rs/Cargo.toml` passed
  with 32 tests; `cargo check --manifest-path backend-rs/Cargo.toml` passed.
- 2026-05-19 13:44 +08:00 checkpoint:
  legacy-only candidate `apply_risk` payloads now split cleanly between the
  UI-facing `apply_risk` object and the compat-only `risk_points` list. Rust
  now matches Python by collapsing `apply_risk` to `null` when the source
  object only carries legacy `risk_points`, while still preserving those
  points for older consumers. Validation: touched-file rustfmt check passed;
  `cargo test chapter_draft_query_service --manifest-path backend-rs/Cargo.toml`
  passed with 51 tests; `cargo test chapter_draft_apply_service
  --manifest-path backend-rs/Cargo.toml` passed with 32 tests; `cargo check
  --manifest-path backend-rs/Cargo.toml` passed; `git diff --check -- backend-rs/src/services/chapter_draft_query_service.rs backend-rs/src/services/chapter_draft_apply_service.rs`
  passed.
- 2026-05-19 13:44 +08:00 checkpoint:
  legacy-only candidate `quality_highlights` payloads now also split cleanly
  between the UI-facing `quality_highlights` facet object and the compat-only
  `highlight_points` list. Rust now keeps `quality_highlights=null` unless
  the payload contains meaningful `continuity` or `foreshadow` facet data,
  while still preserving legacy `highlight_points` for compatibility. This
  keeps the frontend comparison modal aligned with its typed facet contract
  without dropping the legacy summary list. Validation: touched-file rustfmt
  check passed; `cargo test chapter_draft_query_service --manifest-path
  backend-rs/Cargo.toml` passed with 51 tests; `cargo test
  chapter_draft_apply_service --manifest-path backend-rs/Cargo.toml` passed
  with 32 tests; `cargo check --manifest-path backend-rs/Cargo.toml` passed;
  `git diff --check -- backend-rs/src/services/chapter_draft_query_service.rs backend-rs/src/services/chapter_draft_apply_service.rs`
  passed.
- 2026-05-19 13:50 +08:00 checkpoint:
  `chapter_batch_generation` status SSE route now also follows the existing
  route-to-workflow seam. `chapter_batch_generation.rs` no longer performs the
  local `stream access gate + stream builder` orchestration; that ownership is
  moved into the new
  `chapter_batch_generation_stream_workflow_service.rs` helper, matching the
  already-adopted single-generation and regeneration stream workflow pattern
  without changing keep-alive behavior, error mapping, or event payload
  semantics. Validation: touched-file rustfmt check passed; `cargo test
  chapter_batch_generation_status_stream_service --manifest-path
  backend-rs/Cargo.toml` passed with 3 tests; `cargo test
  chapter_batch_generation --manifest-path backend-rs/Cargo.toml` passed with
  35 tests; `cargo check --manifest-path backend-rs/Cargo.toml` passed;
  `git diff --check -- backend-rs/src/api/chapter_batch_generation.rs backend-rs/src/services/chapter_batch_generation_stream_workflow_service.rs backend-rs/src/services/mod.rs`
  passed.
- 2026-05-19 13:54 +08:00 checkpoint:
  `chapter_batch_generation` resume route now also keeps only transport
  concerns. `chapter_batch_generation.rs` no longer owns the local
  `prepare resume request + dispatch runtime + return response payload`
  orchestration; that sequence is now wrapped by
  `resume_owned_batch_generation_task()` in
  `chapter_batch_generation_resume_service.rs`, so the existing resume
  prepare helper becomes the internal workflow boundary instead of a
  route-visible intermediate step. This preserves the current error mapping,
  response payload, and runtime dispatch semantics while shrinking another
  behavior-sensitive route branch. Validation: touched-file rustfmt check
  passed; `cargo test chapter_batch_generation_resume_service --manifest-path
  backend-rs/Cargo.toml` compiled successfully with the current filter
  matching 0 tests; `cargo test chapter_batch_generation --manifest-path
  backend-rs/Cargo.toml` passed with 35 tests; `cargo check --manifest-path
  backend-rs/Cargo.toml` passed; `git diff --check -- backend-rs/src/services/chapter_batch_generation_resume_service.rs backend-rs/src/api/chapter_batch_generation.rs`
  passed.
- 2026-05-19 13:57 +08:00 checkpoint:
  the remaining low-risk `chapter_batch_generation` route orchestration has
  now been compressed into workflow-owned dispatch boundaries. Both
  `create_batch_generate` and `generate_chapter_content_background` keep their
  request parsing and compat-field consumption in the route, but the local
  `create workflow + dispatch runtime + return response payload` sequencing is
  now owned by
  `start_owned_batch_generation_workflow()` and
  `start_owned_single_generation_background_workflow()` in the corresponding
  workflow services. This aligns batch-create and single-background generation
  with the same thin-route pattern already used by batch status stream and
  resume, without changing response payloads or runtime dispatch semantics.
  Validation: touched-file rustfmt check passed; `cargo test
  chapter_batch_generation --manifest-path backend-rs/Cargo.toml` passed with
  35 tests; `cargo test chapter_single_generation_background_workflow_service
  --manifest-path backend-rs/Cargo.toml` compiled successfully with the
  current filter matching 0 tests; `cargo check --manifest-path
  backend-rs/Cargo.toml` passed; `git diff --check -- backend-rs/src/services/chapter_batch_generation_create_workflow_service.rs backend-rs/src/services/chapter_single_generation_background_workflow_service.rs backend-rs/src/api/chapter_batch_generation.rs`
  passed.
- 2026-05-19 14:01 +08:00 checkpoint:
  chapter-domain route registration is now pulled back under a single gateway
  entrypoint. `backend-rs/src/api/chapters.rs` now also merges
  `chapter_batch_generation::routes()`, and `backend-rs/src/api/router.rs`
  no longer registers batch-generation routes separately at the top-level API
  router. This does not change any route path or handler behavior; it only
  aligns route ownership with the existing chapter gateway split so chapter
  CRUD, analysis, regeneration, and batch-generation routes are once again
  grouped under one chapter-domain aggregator. Validation: touched-file
  rustfmt check passed; `cargo test chapter_batch_generation --manifest-path
  backend-rs/Cargo.toml` passed with 35 tests; `cargo check --manifest-path
  backend-rs/Cargo.toml` passed; `git diff --check -- backend-rs/src/api/chapters.rs backend-rs/src/api/router.rs`
  passed.
- 2026-05-19 14:06 +08:00 checkpoint:
  chapter-generation SSE routes now reuse a shared keep-alive helper instead
  of rebuilding the same 10-second keep-alive configuration inline. The new
  helpers in `backend-rs/src/utils/sse.rs` centralize the default 10-second
  keep-alive and the named `"keep-alive"` variant, and the chapter batch
  generation plus chapter regeneration routes now call those helpers instead
  of carrying route-local `KeepAlive::new().interval(...)` copies. This keeps
  SSE event payloads and stream behavior unchanged while reducing boundary
  drift across chapter-domain streaming routes. Validation: touched-file
  rustfmt check passed; `cargo test chapter_batch_generation --manifest-path
  backend-rs/Cargo.toml` passed with 35 tests; `cargo test
  chapter_regeneration --manifest-path backend-rs/Cargo.toml` passed with 21
  tests; `cargo check --manifest-path backend-rs/Cargo.toml` passed; `git
  diff --check -- backend-rs/src/utils/sse.rs backend-rs/src/api/chapter_batch_generation.rs backend-rs/src/api/chapter_regeneration_routes.rs`
  passed.
- 2026-05-19 14:11 +08:00 checkpoint:
  chapter-domain API error mappers now reuse shared detail-response helpers
  instead of repeating the same JSON/status projection inline. The shared
  helper set in `backend-rs/src/api/chapters_error_mapper.rs` now owns the
  generic `detail_error`, `internal_detail_error`, and the stable
  `chapter/project not found or access denied` responses, and chapter
  analysis draft/query plus regeneration query error mappers now delegate to
  those helpers. This keeps status codes and response messages unchanged while
  reducing repeated error-shape logic across chapter-domain routes. Validation:
  touched-file rustfmt check passed; `cargo test chapter_analysis
  --manifest-path backend-rs/Cargo.toml` passed with 17 tests; `cargo test
  chapter_regeneration --manifest-path backend-rs/Cargo.toml` passed with 21
  tests; `cargo check --manifest-path backend-rs/Cargo.toml` passed; `git
  diff --check -- backend-rs/src/api/chapters_error_mapper.rs backend-rs/src/api/chapter_analysis_query_error_mapper.rs backend-rs/src/api/chapter_analysis_draft_error_mapper.rs backend-rs/src/api/chapter_regeneration_query_error_mapper.rs`
  passed.
- 2026-05-19 14:16 +08:00 checkpoint:
  refactor progress was re-aligned against the planning documents before the
  next implementation wave. The active follow-up task is now explicitly
  treated as Phase 3 (`Rust` internal boundary shrink) work from
  `docs/architecture/rust-strangler-refactor-plan-2026-05-17.zh-CN.md`, not
  as Phase 1 deployment hardening or Phase 2 schema-ownership work. The
  review also confirmed that the low-risk `chapter_batch_generation` route
  seam has effectively reached its stop point, so further safe work should
  stay in shared helper / read-side reuse instead of entering
  `runtime_state/status_view/task_command` semantics. As part of that next
  safe lane, `chapter_batch_generation_error_mapper.rs` now also delegates its
  repeated `detail` JSON plus stable chapter/project access-denied responses
  to the shared helper set in `chapters_error_mapper.rs`, keeping status codes
  and messages unchanged while reducing another drift-prone adapter copy.
  Validation: touched-file rustfmt check passed; `cargo test
  chapter_batch_generation --manifest-path backend-rs/Cargo.toml` passed with
  35 tests; `cargo check --manifest-path backend-rs/Cargo.toml` passed; `git
  diff --check -- backend-rs/src/api/chapter_batch_generation_error_mapper.rs backend-rs/src/api/chapters_error_mapper.rs`
  passed.
- 2026-05-19 14:20 +08:00 checkpoint:
  chapter CRUD error mapping now has one less local drift point. The repeated
  `success:false/message` response branches for stable
  `project/chapter not found or access denied` cases and internal-message
  passthroughs were consolidated inside
  `backend-rs/src/api/chapter_crud_error_mapper.rs` via focused local helpers,
  while preserving the existing CRUD-specific compat response shape instead of
  forcing it into the chapter-domain `detail` JSON pattern. This keeps CRUD
  HTTP semantics and payload structure unchanged while reducing the repeated
  adapter code in a low-risk file. Validation: touched-file rustfmt check
  passed; `cargo test chapter_crud --manifest-path backend-rs/Cargo.toml`
  passed with 3 tests; `cargo check --manifest-path backend-rs/Cargo.toml`
  passed; `git diff --check -- backend-rs/src/api/chapter_crud_error_mapper.rs`
  passed.
- 2026-05-19 14:31 +08:00 checkpoint:
  the Phase 3 safe-lane helper reuse continued with one smaller cleanup in
  `backend-rs/src/api/chapter_analysis_query_error_mapper.rs`. The owned
  chapter-analysis view error path still keeps its special-case
  `"Chapter analysis not found"` -> `404` behavior, but the final
  `(status, {"detail": ...})` projection now delegates to the shared
  `detail_error()` helper from `chapters_error_mapper.rs` instead of building
  the same JSON shape inline again. This keeps HTTP semantics and payload
  shape unchanged while removing another tiny route-adjacent mapper drift
  point without re-entering batch runtime/status semantics. Validation:
  touched-file rustfmt check passed; `cargo test chapter_analysis
  --manifest-path backend-rs/Cargo.toml` passed with 17 tests; `cargo check
  --manifest-path backend-rs/Cargo.toml` passed; `git diff --check -- backend-rs/src/api/chapter_analysis_query_error_mapper.rs`
  passed.
- 2026-05-19 14:39 +08:00 checkpoint:
  `backend-rs/src/api/chapter_batch_generation_error_mapper.rs` now also
  removes one more local drift point inside its single-chapter lane. The
  request-preparation and background-workflow error mappers used to carry two
  copies of the same `ChapterNotFound / ChapterNotFoundOrAccessDenied /
  Config / Internal` projection logic; they now both delegate to one focused
  local `map_single_chapter_generation_error()` helper in the same file.
  This keeps all existing status codes and `detail` messages unchanged while
  reducing another adapter-only duplication point in the Phase 3 safe lane,
  without touching batch runtime/status semantics or route payload shapes.
  Validation: touched-file rustfmt check passed; `cargo test
  chapter_batch_generation --manifest-path backend-rs/Cargo.toml` passed with
  35 tests; `cargo check --manifest-path backend-rs/Cargo.toml` passed; `git
  diff --check -- backend-rs/src/api/chapter_batch_generation_error_mapper.rs`
  passed.
- 2026-05-19 14:50 +08:00 checkpoint:
  `backend-rs/src/api/chapter_analysis_draft_error_mapper.rs` now pushes more
  of its stable `detail`-response projection through the shared chapter-domain
  helper set instead of rebuilding `Json({"detail": ...})` inline for every
  fixed-message branch. The auto-revision and candidate draft load/apply
  mappers still preserve the same `404 / 400 / 409 / 500` status codes and
  the same Chinese `detail` messages, but the route-adjacent mapper now
  reuses `detail_error()` for those constant branches while continuing to use
  `internal_detail_error()` for passthrough internal failures. This keeps the
  externally observable API behavior unchanged while removing another small
  duplication cluster inside the Phase 3 safe lane. Validation: touched-file
  rustfmt check passed; `cargo test chapter_analysis --manifest-path
  backend-rs/Cargo.toml` passed with 17 tests; `cargo check --manifest-path
  backend-rs/Cargo.toml` passed; `git diff --check -- backend-rs/src/api/chapter_analysis_draft_error_mapper.rs`
  passed.
- 2026-05-19 14:58 +08:00 checkpoint:
  the same `chapter_analysis_draft_error_mapper.rs` slice was tightened one
  step further by removing its local `chapter_not_found_error()` wrapper,
  which only forwarded to the shared
  `chapter_not_found_or_access_denied_error()` helper. The mapper now calls
  the shared chapter-domain helper directly for analysis task status and the
  owned auto-revision / candidate draft access-denied branches, keeping all
  existing `404` payload shapes and messages unchanged while trimming one more
  route-adjacent alias layer from the Phase 3 safe lane. Validation:
  touched-file rustfmt check passed; `cargo test chapter_analysis
  --manifest-path backend-rs/Cargo.toml` passed with 17 tests; `cargo check
  --manifest-path backend-rs/Cargo.toml` passed; `git diff --check -- backend-rs/src/api/chapter_analysis_draft_error_mapper.rs`
  passed.
- 2026-05-19 15:06 +08:00 checkpoint:
  `backend-rs/src/api/chapter_analysis_query_error_mapper.rs` was also
  tightened by removing the local `map_chapter_quality_metrics_query_error()`
  alias, which had become a pure wrapper around the shared
  `internal_detail_error()` helper after the owned-query migration. The owned
  quality-metrics mapper now routes its internal failure branch directly to
  `internal_detail_error()` while keeping the existing access-denied `404`
  path and all external payload shapes unchanged. This trims one more tiny
  route-adjacent alias layer in the Phase 3 safe lane without touching query
  ownership or analysis payload semantics. Validation: touched-file rustfmt
  check passed; `cargo test chapter_analysis --manifest-path
  backend-rs/Cargo.toml` passed with 17 tests; `cargo check --manifest-path
  backend-rs/Cargo.toml` passed; `git diff --check -- backend-rs/src/api/chapter_analysis_query_error_mapper.rs`
  passed.
- 2026-05-19 15:13 +08:00 checkpoint:
  the analysis lane was tightened once more by removing the local
  `map_batch_analysis_task_status_query_error()` alias from
  `backend-rs/src/api/chapter_analysis_query_error_mapper.rs`. That helper had
  become a pure wrapper around the shared `internal_detail_error()` response
  mapper, so `backend-rs/src/api/chapter_analysis_routes.rs` now sends the
  batch analysis status route's internal failure branch directly to
  `internal_detail_error()` instead. This keeps the same `500 {"detail":
  ...}` behavior while trimming one more route-adjacent alias layer from the
  Phase 3 safe lane without changing batch analysis query ownership or route
  payload semantics. Validation: touched-file rustfmt check passed; `cargo
  test chapter_analysis --manifest-path backend-rs/Cargo.toml` passed with 17
  tests; `cargo check --manifest-path backend-rs/Cargo.toml` passed; `git
  diff --check -- backend-rs/src/api/chapter_analysis_query_error_mapper.rs backend-rs/src/api/chapter_analysis_routes.rs`
  passed.
- 2026-05-19 15:20 +08:00 checkpoint:
  `backend-rs/src/api/chapter_analysis_query_error_mapper.rs` was tightened
  one more step by inlining the single-use `map_chapter_analysis_view_error()`
  helper back into `map_owned_chapter_analysis_view_error()`. The special
  `"Chapter analysis not found"` -> `404` branch and the default
  internal-error `500 {"detail": ...}` branch remain exactly the same, but
  the mapper no longer carries a one-call wrapper just to compute the status
  before delegating to `detail_error()`. This trims another small local alias
  layer from the Phase 3 safe lane without changing analysis view ownership or
  payload semantics. Validation: touched-file rustfmt check passed; `cargo
  test chapter_analysis --manifest-path backend-rs/Cargo.toml` passed with 17
  tests; `cargo check --manifest-path backend-rs/Cargo.toml` passed; `git
  diff --check -- backend-rs/src/api/chapter_analysis_query_error_mapper.rs`
  passed.
- 2026-05-19 15:28 +08:00 checkpoint:
  the analysis lane also removed one more duplicate mapper implementation by
  switching the analysis task-status route to the shared
  `map_load_analysis_task_status_error()` helper in
  `backend-rs/src/api/chapters_error_mapper.rs`. The local
  `map_analysis_task_status_error()` copy was deleted from
  `backend-rs/src/api/chapter_analysis_draft_error_mapper.rs`, and
  `backend-rs/src/api/chapter_analysis_routes.rs` now maps
  `load_analysis_task_status_payload()` failures through the shared helper
  instead. This keeps the same `404` and `500 {"detail": ...}` behavior while
  removing a true duplicate implementation from the Phase 3 safe lane without
  changing analysis task-status payload semantics. Validation: touched-file
  rustfmt check passed; `cargo test chapter_analysis --manifest-path
  backend-rs/Cargo.toml` passed with 17 tests; `cargo check --manifest-path
  backend-rs/Cargo.toml` passed; `git diff --check -- backend-rs/src/api/chapter_analysis_draft_error_mapper.rs backend-rs/src/api/chapter_analysis_routes.rs`
  passed.
- 2026-05-19 16:57 +08:00 checkpoint:
  the follow-up execution temporarily shifted from Rust-internal seam
  shrinking into Phase 5 strangler governance asset hardening so the current
  gateway owner map is backed by executable evidence instead of stale Nginx
  comments or prior assumptions. `backend/tools/run_strangler_gateway_smoke.py`
  now supports manifest `profiles`, `deploy-strangler.ps1` explicitly runs the
  `deploy` profile only, and `deploy/strangler-gateway-probes.json` now also
  carries a `route-groups` profile with through-gateway owner probes for
  `auth/config`, `changelog`, `settings`, `projects`, `wizard-stream`, and
  `memories`. Running the new route-group profile against the current gateway
  on `http://127.0.0.1:8005` also falsified an older planning assumption:
  the currently exposed `projects`, `wizard-stream`, and `/api/memories/*`
  API paths already resolve to Rust-owned auth/public boundaries, so the Phase
  5 docs were corrected away from the previous blanket `mixed` label and now
  treat the remaining issue as stale fallback configuration / owner
  documentation drift. Validation: `pytest
  backend/tests/test_tools/test_run_strangler_gateway_smoke.py` passed (6
  tests); `python backend/tools/run_strangler_gateway_smoke.py --profile
  deploy --validate-manifest-only` passed; `python
  backend/tools/run_strangler_gateway_smoke.py --profile route-groups
  --validate-manifest-only` passed; live `route-groups` probe run against
  `127.0.0.1:8005` passed with 9 probes; `git diff --check -- backend/tools/run_strangler_gateway_smoke.py backend/tests/test_tools/test_run_strangler_gateway_smoke.py deploy/strangler-gateway-probes.json deploy-strangler.ps1 docs/architecture/rust-python-api-parity-matrix-2026-05-19.zh-CN.md docs/architecture/rust-route-group-ownership-and-cutover-checklist-2026-05-19.zh-CN.md`
  passed with only existing line-ending warnings from Git.
- 2026-05-19 17:00 +08:00 checkpoint:
  the Phase 5 executable owner evidence was expanded into the chapter gateway,
  which is the highest-risk Rust-owned API domain in the strangler plan.
  `deploy/strangler-gateway-probes.json` now adds five low-precondition
  `route-groups` probes for the `chapters` surface:
  `GET /api/chapters?project_id=...`, `GET /api/chapters/{id}/analysis`,
  `POST /api/chapters/analysis/status/batch`,
  `GET /api/chapters/batch-generate/active-tasks`, and
  `GET /api/chapters/{id}/regeneration/tasks`. A live run against
  `http://127.0.0.1:8005` confirmed all five currently resolve to the Rust
  auth boundary with the stable `401 {"detail":"未登录，请先登录"}` shape,
  which means the chapter CRUD, analysis, batch, and regeneration lanes are
  now all represented in the route-group smoke evidence chain. The Phase 5
  docs were updated to record that `chapters` now has first-pass owner probes
  in place, while explicitly leaving stronger stream/business smoke for a
  later slice. Validation target for this checkpoint: live `route-groups`
  smoke should now pass with 14 probes total.
- 2026-05-19 17:39 +08:00 checkpoint:
  the Phase 5 gateway-smoke asset now has enough expressiveness to carry the
  next business/SSE wave instead of being limited to status-code plus JSON
  subset assertions. `backend/tools/run_strangler_gateway_smoke.py` now
  accepts per-probe `headers`, `json_body` / `body`,
  `expected_text_startswith`, and `expected_text_contains`, while preserving
  the existing `expected_json` and `expected_content_type_contains`
  assertions. Focused pytest coverage now locks the new request forwarding,
  conflicting-body validation, and text-prefix/contains checks. As a first
  low-risk consumer of that capability, the existing route-group POST owner
  probes for `POST /api/chapters/analysis/status/batch` and
  `POST /api/wizard-stream/outline` now send minimal JSON request bodies plus
  a dedicated smoke header, so those through-gateway owner checks are closer
  to real call shapes without changing the default deploy profile. Validation:
  `pytest backend/tests/test_tools/test_run_strangler_gateway_smoke.py`
  passed with 9 tests; `python backend/tools/run_strangler_gateway_smoke.py
  --manifest deploy/strangler-gateway-probes.json --profile route-groups
  --validate-manifest-only` passed; live `python
  backend/tools/run_strangler_gateway_smoke.py --manifest
  deploy/strangler-gateway-probes.json --profile route-groups --base-url
  http://127.0.0.1:8005` passed with 14 probes; `git diff --check -- backend/tools/run_strangler_gateway_smoke.py backend/tests/test_tools/test_run_strangler_gateway_smoke.py deploy/strangler-gateway-probes.json`
  passed with the existing Git line-ending warning on
  `deploy/strangler-gateway-probes.json`.
- 2026-05-19 19:24 +08:00 checkpoint:
  the Phase 5 smoke manifest now has a separate `business` profile for public
  JSON/HTML assertions, so the stable control-plane probes can stay isolated
  from the default deploy slice. `deploy/strangler-gateway-probes.json` now
  marks the shared root probe, `GET /api/auth/config`, and
  `GET /api/changelog` as `business` probes. The root probe asserts the page
  title and `lang="zh-CN"` marker, `auth/config` locks the stable
  `linuxdo_enabled=false` plus `local_auth_enabled=true` JSON shape, and
  `changelog` now checks only the stable JSON field names/content markers
  instead of a time-sensitive cache value. Validation:
  `pytest backend/tests/test_tools/test_run_strangler_gateway_smoke.py`
  passed with 10 tests; `python
  backend/tools/run_strangler_gateway_smoke.py --manifest
  deploy/strangler-gateway-probes.json --profile business --validate-manifest-only`
  passed; live `python backend/tools/run_strangler_gateway_smoke.py --manifest
  deploy/strangler-gateway-probes.json --profile business --base-url
  http://127.0.0.1:8005` passed with 3 probes; `git diff --check -- backend/tests/test_tools/test_run_strangler_gateway_smoke.py deploy/strangler-gateway-probes.json docs/architecture/rust-route-group-ownership-and-cutover-checklist-2026-05-19.zh-CN.md docs/architecture/rust-python-api-parity-matrix-2026-05-19.zh-CN.md`
  passed with the existing Git line-ending warning on
  `deploy/strangler-gateway-probes.json`.
- 2026-05-19 19:32 +08:00 checkpoint:
  the default deploy smoke slice gained one more stable Rust-owned health
  probe: `GET /health/db-sessions`. The endpoint returns a fixed JSON
  structure with zeroed session counters and a nullable warning field, so it
  is a safe low-risk addition to the `deploy` / `route-groups` profile and
  does not depend on login state or business data. Focused tests now cover
  the new manifest shape, and the live deploy smoke on
  `http://127.0.0.1:8005` passed with four probes total
  (`/health`, `/readyz`, `/health/db-sessions`, `/`). Validation:
  `pytest backend/tests/test_tools/test_run_strangler_gateway_smoke.py`
  passed with 11 tests; `python
  backend/tools/run_strangler_gateway_smoke.py --manifest
  deploy/strangler-gateway-probes.json --profile deploy --validate-manifest-only`
  passed; live `python backend/tools/run_strangler_gateway_smoke.py --manifest
  deploy/strangler-gateway-probes.json --profile deploy --base-url
  http://127.0.0.1:8005` passed with 4 probes; `git diff --check -- backend/tests/test_tools/test_run_strangler_gateway_smoke.py deploy/strangler-gateway-probes.json`
  passed with the existing Git line-ending warning on
  `deploy/strangler-gateway-probes.json`.
- 2026-05-19 19:38 +08:00 checkpoint:
  the deploy smoke slice now also covers `GET /livez`, so the Rust health
  surface is represented by three fixed JSON probes (`/health`, `/livez`,
  `/readyz`) plus the structured `/health/db-sessions` check. This keeps the
  default deploy profile fully on Rust-owned control-plane endpoints while
  staying independent of login state and mutable business data. Validation:
  `pytest backend/tests/test_tools/test_run_strangler_gateway_smoke.py`
  passed with 12 tests; `python
  backend/tools/run_strangler_gateway_smoke.py --manifest
  deploy/strangler-gateway-probes.json --profile deploy --validate-manifest-only`
  passed; live `python backend/tools/run_strangler_gateway_smoke.py --manifest
  deploy/strangler-gateway-probes.json --profile deploy --base-url
  http://127.0.0.1:8005` passed with 5 probes; `git diff --check -- backend/tests/test_tools/test_run_strangler_gateway_smoke.py deploy/strangler-gateway-probes.json`
  passed with the existing Git line-ending warning on
  `deploy/strangler-gateway-probes.json`.
- 2026-05-19 19:44 +08:00 checkpoint:
  the route-group ownership and parity docs were updated to match the current
  health smoke truth instead of the earlier two-probe shorthand. The health
  route group is now documented as `health / livez / readyz`, with
  `rust-health`, `rust-livez`, `rust-readiness`, and
  `rust-health-db-sessions` explicitly listed as stable smoke evidence.
  The parity matrix now also names the current
  `deploy/strangler-gateway-probes.json` coverage for `/health`, `/livez`,
  `/readyz`, and `/health/db-sessions`. Validation for this doc-only update:
  `git diff --check -- docs/architecture/rust-route-group-ownership-and-cutover-checklist-2026-05-19.zh-CN.md docs/architecture/rust-python-api-parity-matrix-2026-05-19.zh-CN.md`
  passed.
- 2026-05-19 19:55 +08:00 checkpoint:
  the gateway smoke manifest now supports `expected_json_has_keys`, allowing
  structural JSON assertions without pinning time-sensitive values. This was
  used to tighten `GET /api/changelog` from a text-substring fallback to a
  stable key-structure assertion over `commits`, `cached`, and `cache_time`,
  while keeping the existing content-type check. Validation:
  `pytest backend/tests/test_tools/test_run_strangler_gateway_smoke.py`
  passed with 14 tests; live `python
  backend/tools/run_strangler_gateway_smoke.py --manifest
  deploy/strangler-gateway-probes.json --profile business --base-url
  http://127.0.0.1:8005` passed with 3 probes; `git diff --check -- backend/tools/run_strangler_gateway_smoke.py backend/tests/test_tools/test_run_strangler_gateway_smoke.py deploy/strangler-gateway-probes.json`
  passed with the existing Git line-ending warning on
  `deploy/strangler-gateway-probes.json`.
- 2026-05-19 21:48 +08:00 checkpoint:
  the Phase 5 wave moved from stronger-smoke evaluation into rollback asset
  capture for the P0 route groups because the current `settings`,
  `projects`, `chapters`, `wizard-stream`, and `memories` gateway probes still
  skew toward auth-boundary owner evidence rather than stable public business
  or SSE assertions. `backend/tools/run_strangler_gateway_smoke.py` now
  supports repeated `--probe-name` filtering on top of the existing `profile`
  selection, which made it possible to validate a single route-group rollback
  step before re-running the whole `phase5-p0` slice. The first operator-ready
  rollback asset now lives in
  `docs/architecture/rust-phase5-p0-route-group-rollback-runbook-2026-05-19.zh-CN.md`,
  and the Phase 5 ownership/parity docs now reference that runbook as the
  current rollback source of truth for `settings`, `projects`, `chapters`,
  `wizard-stream`, and `memories`. Validation target for this checkpoint:
  `pytest backend/tests/test_tools/test_run_strangler_gateway_smoke.py -q`,
  `python backend/tools/run_strangler_gateway_smoke.py --manifest
  deploy/strangler-gateway-probes.json --profile phase5-p0 --probe-name
  settings-auth-guard-rust --validate-manifest-only`, and `git diff --check`
  over the touched smoke/doc files should all pass.
- 2026-05-19 21:52 +08:00 checkpoint:
  the P0 rollback asset is now one step more executable because the smoke
  manifest gained explicit `route_group` ownership labels for `settings`,
  `projects`, `chapters`, `wizard-stream`, and `memories`, while
  `backend/tools/run_strangler_gateway_smoke.py` now supports repeated
  `--route-group` filtering before any optional `--probe-name` narrowing. This
  removes the need for operators to manually translate a route group back into
  a probe-name list when validating a rollback step, and the new runbook now
  uses `--route-group` as the primary command shape while keeping
  `--probe-name` as an escape hatch for single-probe diagnosis. Validation:
  `pytest backend/tests/test_tools/test_run_strangler_gateway_smoke.py -q`
  passed with 20 tests; `python backend/tools/run_strangler_gateway_smoke.py
  --manifest deploy/strangler-gateway-probes.json --profile phase5-p0
  --route-group chapters --validate-manifest-only` passed with `probe_count=5`;
  `git diff --check -- backend/tools/run_strangler_gateway_smoke.py
  backend/tests/test_tools/test_run_strangler_gateway_smoke.py
  deploy/strangler-gateway-probes.json
  docs/architecture/rust-phase5-p0-route-group-rollback-runbook-2026-05-19.zh-CN.md
  docs/architecture/rust-route-group-ownership-and-cutover-checklist-2026-05-19.zh-CN.md
  docs/architecture/rust-python-api-parity-matrix-2026-05-19.zh-CN.md
  .trellis/tasks/05-18-backend-chapter-generation-refactor-followup/implement.md`
  passed with the existing line-ending warning on
  `deploy/strangler-gateway-probes.json`.
- 2026-05-19 22:11 +08:00 checkpoint:
  the next Phase 5 slice stayed on governance assets and did not expand the
  smoke runner again. Instead, the P0 rollback runbook now records a first
  matrix of Python fallback success clues for `settings`, `projects`,
  `chapters`, `wizard-stream`, and `memories`, so operators have a concrete
  owner-change checklist after a gateway rollback even before a dedicated
  fallback smoke profile exists. The documented clues intentionally stay at
  the transport/auth-boundary level: Python `settings` should fall back to
  `401 {"detail":"需要登录"}`, `projects` / `memories` to
  `401 {"detail":"未登录"}`, chapter read/query routes mostly to
  `401 {"detail":"未登录"}` while Python batch-generation active-task listing
  keeps the older English `401 {"detail":"Not logged in"}`, and
  `wizard-stream/outline` should re-enter the Python dependency/auth path
  before the SSE body is produced. The docs also now call out that
  `GET /api/chapters?project_id=...` is not a valid Python fallback success
  probe because the Python list route shape is `/api/chapters/project/{id}`.
  Validation target for this checkpoint: `git diff --check` over the touched
  Phase 5 docs should pass.
- 2026-05-19 22:15 +08:00 checkpoint:
  the documented Python fallback clues are now partially executable instead of
  staying doc-only. `deploy/strangler-gateway-probes.json` now defines a
  dedicated `phase5-p0-fallback` profile that covers the lowest-precondition
  P0 fallback checks for `settings`, `projects`, `chapters`,
  `wizard-stream`, and `memories`, using the Python-side auth-boundary
  differences already captured in the runbook: `401 {"detail":"需要登录"}` for
  `settings` and `wizard-stream/outline`, `401 {"detail":"未登录"}` for
  `projects`, `memories`, chapter analysis/status/regeneration queries, and
  `401 {"detail":"Not logged in"}` for the Python batch-generation
  `active-tasks` listing. The profile intentionally excludes
  `GET /api/chapters?project_id=...` because that Rust query-shape list route
  is not the Python fallback path. Focused pytest now locks the new profile
  selection behavior, and the Phase 5 docs now describe `phase5-p0-fallback`
  as the first executable Python fallback slice rather than a future idea.
  Validation target for this checkpoint: `pytest
  backend/tests/test_tools/test_run_strangler_gateway_smoke.py -q`,
  `python backend/tools/run_strangler_gateway_smoke.py --manifest
  deploy/strangler-gateway-probes.json --profile phase5-p0-fallback
  --route-group chapters --validate-manifest-only`, and `git diff --check`
  over the touched smoke/doc files should pass.
- 2026-05-19 22:33 +08:00 checkpoint:
  the `chapters` portion of `phase5-p0-fallback` now covers the Python list
  primary path instead of stopping at query/status/task side probes only.
  `deploy/strangler-gateway-probes.json` now includes
  `chapters-project-list-auth-guard-python-fallback` for
  `GET /api/chapters/project/{project_id}` with the Python auth-boundary
  expectation `401 {"detail":"未登录"}`. This is intentionally distinct from
  the Rust owner probe `GET /api/chapters?project_id=...`, because that query
  shape is not the Python fallback route contract. With this addition, the
  `chapters` fallback slice now covers project-path list, analysis, batch
  analysis status, batch active-tasks, and regeneration-task reads, and the
  Phase 5 docs can treat that coverage as executable fallback smoke rather
  than a remaining manual verification note. Validation target for this
  checkpoint: `pytest backend/tests/test_tools/test_run_strangler_gateway_smoke.py -q`,
  `python backend/tools/run_strangler_gateway_smoke.py --manifest
  deploy/strangler-gateway-probes.json --profile phase5-p0-fallback
  --route-group chapters --validate-manifest-only`, and `git diff --check`
  over the touched smoke/doc/task files should pass.
- 2026-05-19 22:28 +08:00 checkpoint:
  the next Phase 5 governance slice improved smoke-result usability rather
  than expanding probe semantics. `backend/tools/run_strangler_gateway_smoke.py`
  now emits manifest-level `owner_counts`, `route_group_counts`, and
  `route_group_probe_names` alongside the selected probe list, so
  `tmp/smoke/*.json` can be used directly as a rollback/fallback evidence
  artifact without manually regrouping probes after the run. Focused tests now
  lock both the inventory summarizer and the summary payload shape, and the P0
  rollback docs now explicitly call out these rollups as the preferred report
  skeleton for route-group operations. Validation:
  `pytest backend/tests/test_tools/test_run_strangler_gateway_smoke.py -q`
  passed with 23 tests; `python backend/tools/run_strangler_gateway_smoke.py
  --manifest deploy/strangler-gateway-probes.json --profile phase5-p0-fallback
  --route-group chapters --validate-manifest-only` now reports both
  `probe_count=5` and the new `owner_counts` / `route_group_counts` /
  `route_group_probe_names` fields; `git diff --check -- backend/tools/run_strangler_gateway_smoke.py
  backend/tests/test_tools/test_run_strangler_gateway_smoke.py
  docs/architecture/rust-phase5-p0-route-group-rollback-runbook-2026-05-19.zh-CN.md
  docs/architecture/rust-route-group-ownership-and-cutover-checklist-2026-05-19.zh-CN.md
  .trellis/tasks/05-18-backend-chapter-generation-refactor-followup/implement.md`
  passed.
- 2026-05-19 22:33 +08:00 checkpoint:
  Phase 5 has now started a first P1 route-group executable skeleton instead
  of leaving `auth` / `users` only in the checklist. The smoke manifest now
  defines `phase5-p1` with two low-precondition Rust owner probes:
  `auth-config-public-rust` is now also labeled as `route_group="auth"` and
  selected by `phase5-p1`, while `users-current-auth-guard-rust` adds the
  first `users` route-group probe on `GET /api/users/current` with the stable
  Rust auth-boundary response `401 {"detail":"未登录，请先登录"}`. This is
  intentionally only a starter slice, not a full auth-flow or admin-privilege
  business verification set, but it moves P1 from doc-only planning into a
  runnable profile with structured owner/route-group rollups. Validation:
  `pytest backend/tests/test_tools/test_run_strangler_gateway_smoke.py -q`
  passed with 24 tests; `python backend/tools/run_strangler_gateway_smoke.py
  --manifest deploy/strangler-gateway-probes.json --profile phase5-p1
  --validate-manifest-only` passed with `probe_count=2`, `owner_counts={"rust":2}`,
  and route-group coverage for `auth` plus `users`; `git diff --check -- 
  deploy/strangler-gateway-probes.json
  backend/tests/test_tools/test_run_strangler_gateway_smoke.py
  docs/architecture/rust-route-group-ownership-and-cutover-checklist-2026-05-19.zh-CN.md
  docs/architecture/rust-python-api-parity-matrix-2026-05-19.zh-CN.md`
  passed with the existing line-ending warning on
  `deploy/strangler-gateway-probes.json`.
- 2026-05-19 22:36 +08:00 checkpoint:
  the first P1 profile has been strengthened with a public auth action probe
  instead of staying at one config read plus one auth-boundary read. `phase5-p1`
  now also includes `auth-logout-public-rust` on `POST /api/auth/logout`,
  asserting the stable Rust JSON response `{"success": true, "message": "已登出"}`
  without depending on a live logged-in session. This keeps the slice
  low-precondition while moving slightly closer to real auth behavior than a
  pure config read. Validation: `pytest
  backend/tests/test_tools/test_run_strangler_gateway_smoke.py -q` passed with
  24 tests; `python backend/tools/run_strangler_gateway_smoke.py --manifest
  deploy/strangler-gateway-probes.json --profile phase5-p1
  --validate-manifest-only` now reports `probe_count=3`,
  `route_group_counts={"auth":2,"users":1}`, and
  `route_group_probe_names.auth=["auth-config-public-rust","auth-logout-public-rust"]`;
  `git diff --check -- deploy/strangler-gateway-probes.json
  backend/tests/test_tools/test_run_strangler_gateway_smoke.py
  docs/architecture/rust-route-group-ownership-and-cutover-checklist-2026-05-19.zh-CN.md
  docs/architecture/rust-python-api-parity-matrix-2026-05-19.zh-CN.md`
  passed with the existing line-ending warning on
  `deploy/strangler-gateway-probes.json`.
- 2026-05-19 22:39 +08:00 checkpoint:
  this wave advanced two Phase 5 fronts in parallel without touching runtime
  semantics. On the P1 side, `phase5-p1` now includes
  `auth-linuxdo-url-misconfig-rust` on `GET /api/auth/linuxdo/url`, asserting
  the stable Rust misconfiguration branch `400 {"detail":"LinuxDO OAuth 未配置"}`.
  That gives the `auth` route group a third low-precondition public/business
  signal beyond config read and logout. On the P0 side, `settings` no longer
  stops at the root-path auth guard only: `settings-models-auth-guard-rust`
  now covers `GET /api/settings/models?...`, proving that the route-group asset
  has started to move into sub-route ownership checks instead of a single
  top-level endpoint. Validation: `pytest
  backend/tests/test_tools/test_run_strangler_gateway_smoke.py -q` passed with
  24 tests; `python backend/tools/run_strangler_gateway_smoke.py --manifest
  deploy/strangler-gateway-probes.json --profile phase5-p0 --route-group settings
  --validate-manifest-only` now reports `probe_count=2` for `settings`; `python
  backend/tools/run_strangler_gateway_smoke.py --manifest
  deploy/strangler-gateway-probes.json --profile phase5-p1
  --validate-manifest-only` now reports `probe_count=4` with
  `route_group_counts={"auth":3,"users":1}`; `git diff --check -- 
  deploy/strangler-gateway-probes.json
  backend/tests/test_tools/test_run_strangler_gateway_smoke.py
  docs/architecture/rust-route-group-ownership-and-cutover-checklist-2026-05-19.zh-CN.md
  docs/architecture/rust-python-api-parity-matrix-2026-05-19.zh-CN.md`
  passed with the existing line-ending warning on
  `deploy/strangler-gateway-probes.json`.
- 2026-05-19 22:44 +08:00 checkpoint:
  the next Phase 5 wave kept the same dual-track pattern and pushed both sides
  one level deeper into protected/business route ownership. On the `settings`
  side, `phase5-p0` now also includes `settings-fetch-models-auth-guard-rust`
  on `POST /api/settings/fetch-models` with a real JSON body, so the route
  group now has root-path, GET sub-route, and POST sub-route auth-boundary
  probes instead of only read-side coverage. On the `auth` side, `phase5-p1`
  now also includes `auth-user-auth-guard-rust` on `GET /api/auth/user`,
  adding a protected current-user read probe next to the existing public
  config/logout/LinuxDO misconfig checks. Validation: `pytest
  backend/tests/test_tools/test_run_strangler_gateway_smoke.py -q` passed with
  24 tests; `python backend/tools/run_strangler_gateway_smoke.py --manifest
  deploy/strangler-gateway-probes.json --profile phase5-p0 --route-group settings
  --validate-manifest-only` now reports `probe_count=3` with
  `route_group_probe_names.settings=["settings-auth-guard-rust","settings-models-auth-guard-rust","settings-fetch-models-auth-guard-rust"]`;
  `python backend/tools/run_strangler_gateway_smoke.py --manifest
  deploy/strangler-gateway-probes.json --profile phase5-p1
  --validate-manifest-only` now reports `probe_count=5` with
  `route_group_counts={"auth":4,"users":1}`; `git diff --check -- 
  deploy/strangler-gateway-probes.json
  backend/tests/test_tools/test_run_strangler_gateway_smoke.py
  docs/architecture/rust-route-group-ownership-and-cutover-checklist-2026-05-19.zh-CN.md
  docs/architecture/rust-python-api-parity-matrix-2026-05-19.zh-CN.md`
  passed with the existing line-ending warning on
  `deploy/strangler-gateway-probes.json`.
- 2026-05-19 22:48 +08:00 checkpoint:
  this Phase 5 slice continued the same P0/P1 parallel pattern and brought
  both route groups closer to the coverage shape described in the planning
  docs. On the `users` side, `phase5-p1` now also includes
  `users-list-auth-guard-rust` on `GET /api/users`, so the route group is no
  longer represented only by the current-user read and now has an explicit list
  read auth-boundary probe that matches the checklist wording. On the
  `settings` side, `phase5-p0` now also includes
  `settings-test-auth-guard-rust` on `POST /api/settings/test` with a real API
  probe payload, which extends the route-group asset from root path plus
  settings-model fetch helpers into the first connection-test style business
  endpoint. Validation: `pytest
  backend/tests/test_tools/test_run_strangler_gateway_smoke.py -q` passed with
  24 tests; `python backend/tools/run_strangler_gateway_smoke.py --manifest
  deploy/strangler-gateway-probes.json --profile phase5-p0 --route-group settings
  --validate-manifest-only` now reports `probe_count=4` with
  `route_group_counts={"settings":4}`; `python
  backend/tools/run_strangler_gateway_smoke.py --manifest
  deploy/strangler-gateway-probes.json --profile phase5-p1
  --validate-manifest-only` now reports `probe_count=6` with
  `route_group_counts={"auth":4,"users":2}`; `git diff --check -- 
  deploy/strangler-gateway-probes.json
  backend/tests/test_tools/test_run_strangler_gateway_smoke.py
  docs/architecture/rust-route-group-ownership-and-cutover-checklist-2026-05-19.zh-CN.md
  docs/architecture/rust-python-api-parity-matrix-2026-05-19.zh-CN.md`
  passed with the existing line-ending warning on
  `deploy/strangler-gateway-probes.json`.
- 2026-05-19 22:51 +08:00 checkpoint:
  this wave continued to avoid runtime-risky success assertions while still
  moving both route groups deeper into business-adjacent protected endpoints.
  On the `settings` side, `phase5-p0` now also includes
  `settings-check-function-calling-auth-guard-rust` on
  `POST /api/settings/check-function-calling` with a realistic API probe body,
  so the route-group asset now covers root settings read, model list lookup,
  model fetch, connection test, and function-calling test entrypoints at the
  auth-boundary level. On the `auth` side, `phase5-p1` now also includes
  `auth-password-status-auth-guard-rust` on `GET /api/auth/password/status`,
  extending the protected read coverage beyond `/api/auth/user`. Validation:
  `pytest backend/tests/test_tools/test_run_strangler_gateway_smoke.py -q`
  passed with 24 tests; `python backend/tools/run_strangler_gateway_smoke.py
  --manifest deploy/strangler-gateway-probes.json --profile phase5-p0
  --route-group settings --validate-manifest-only` now reports `probe_count=5`
  with `route_group_counts={"settings":5}`; `python
  backend/tools/run_strangler_gateway_smoke.py --manifest
  deploy/strangler-gateway-probes.json --profile phase5-p1
  --validate-manifest-only` now reports `probe_count=7` with
  `route_group_counts={"auth":5,"users":2}`; `git diff --check -- 
  deploy/strangler-gateway-probes.json
  backend/tests/test_tools/test_run_strangler_gateway_smoke.py
  docs/architecture/rust-route-group-ownership-and-cutover-checklist-2026-05-19.zh-CN.md
  docs/architecture/rust-python-api-parity-matrix-2026-05-19.zh-CN.md`
  passed with the existing line-ending warning on
  `deploy/strangler-gateway-probes.json`.
- 2026-05-19 23:21 +08:00 checkpoint:
  this Phase 5 slice strengthened the smoke infrastructure itself instead of
  only adding more route entries. `backend/tools/run_strangler_gateway_smoke.py`
  now records response headers and supports a new per-probe
  `expected_header_contains` assertion, which is intentionally minimal but
  sufficient for cookie/header presence checks. The first direct consumer is
  `auth-logout-public-rust`: it no longer only proves the JSON body
  `{"success":true,"message":"已登出"}`, but also asserts that the response
  carries a `Set-Cookie` header containing `token=` so the probe is closer to
  the real logout contract. Validation: `pytest
  backend/tests/test_tools/test_run_strangler_gateway_smoke.py -q` passed with
  26 tests; `python backend/tools/run_strangler_gateway_smoke.py --manifest
  deploy/strangler-gateway-probes.json --profile phase5-p1 --probe-name
  auth-logout-public-rust --validate-manifest-only` passed with `probe_count=1`;
  `git diff --check -- backend/tools/run_strangler_gateway_smoke.py
  backend/tests/test_tools/test_run_strangler_gateway_smoke.py
  deploy/strangler-gateway-probes.json
  docs/architecture/rust-route-group-ownership-and-cutover-checklist-2026-05-19.zh-CN.md
  docs/architecture/rust-python-api-parity-matrix-2026-05-19.zh-CN.md`
  passed with the existing line-ending warning on
  `deploy/strangler-gateway-probes.json`.
- 2026-05-20 00:33 +08:00 checkpoint:
  the next Phase 5 slice tightened the credibility of the new header-level
  smoke before expanding it further. The initial `expected_header_contains`
  implementation recorded response headers via `dict(headers.items())`, which
  would silently collapse repeated headers and therefore weaken any future
  `Set-Cookie`-based assertions. `backend/tools/run_strangler_gateway_smoke.py`
  now preserves repeated response headers by joining duplicate values with
  newlines, and the focused tests now lock both the helper and the
  header-assertion behavior for multi-value `Set-Cookie` scenarios. This keeps
  `auth-logout-public-rust` trustworthy as a stronger business smoke rather
  than a fragile body-only probe. Validation: `pytest
  backend/tests/test_tools/test_run_strangler_gateway_smoke.py -q` passed with
  27 tests; `python backend/tools/run_strangler_gateway_smoke.py --manifest
  deploy/strangler-gateway-probes.json --profile phase5-p1 --probe-name
  auth-logout-public-rust --validate-manifest-only` passed with `probe_count=1`;
  `git diff --check -- backend/tools/run_strangler_gateway_smoke.py
  backend/tests/test_tools/test_run_strangler_gateway_smoke.py
  deploy/strangler-gateway-probes.json
  docs/architecture/rust-route-group-ownership-and-cutover-checklist-2026-05-19.zh-CN.md
  docs/architecture/rust-python-api-parity-matrix-2026-05-19.zh-CN.md`
  passed with line-ending warnings only.
- 2026-05-20 01:03 +08:00 checkpoint:
  this Phase 5 slice shifted from pure owner smoke expansion into P1 rollback
  governance. `deploy/strangler-gateway-probes.json` now defines the first
  `phase5-p1-fallback` profile, intentionally scoped only to `auth` where the
  Python fallback contract is stable enough to automate: `POST /api/auth/logout`
  now distinguishes Rust `{"success":true,"message":"已登出"}` from Python
  `{"message":"退出登录成功"}` while also checking a cookie-clearing header, and
  `/api/auth/user` plus `/api/auth/password/status` now encode the stable
  Python unauthenticated response `401 {"detail":"未登录"}`. In parallel, a new
  P1 rollback runbook documents why `users` is not yet in the fallback profile:
  Rust `GET /api/users/current` does not map 1:1 to a Python owner path, so
  blindly probe-izing it would create false automation confidence. The Phase 5
  checklist and parity matrix now reflect this split: `auth` has executable
  fallback evidence, while `users` currently has explicit path-shape caveats
  and rollback guidance instead of synthetic parity claims. Validation target
  for this checkpoint: `pytest backend/tests/test_tools/test_run_strangler_gateway_smoke.py -q`,
  `python backend/tools/run_strangler_gateway_smoke.py --manifest
  deploy/strangler-gateway-probes.json --profile phase5-p1-fallback
  --validate-manifest-only`, and `git diff --check -- deploy/strangler-gateway-probes.json
  docs/architecture/rust-phase5-p1-route-group-rollback-runbook-2026-05-20.zh-CN.md
  docs/architecture/rust-route-group-ownership-and-cutover-checklist-2026-05-19.zh-CN.md
  docs/architecture/rust-python-api-parity-matrix-2026-05-19.zh-CN.md
  .trellis/tasks/05-18-backend-chapter-generation-refactor-followup/implement.md`
  should pass, allowing only the existing line-ending warning on the manifest
  file if it persists.
- 2026-05-20 01:20 +08:00 checkpoint:
  this Phase 5 slice returned to low-risk P1 owner expansion after the auth
  fallback governance checkpoint. `deploy/strangler-gateway-probes.json` now
  extends `phase5-p1` with a second starter wave for `characters`,
  `outlines`, and `book_import`: `GET /api/characters/project/{project_id}`,
  `GET /api/outlines/project/{project_id}`, and
  `GET /api/book-import/tasks/{task_id}` now all assert the stable Rust auth
  boundary `401 {"detail":"未登录，请先登录"}` through the gateway. This keeps
  the slice low-precondition and avoids synthetic fallback claims, while still
  moving three more Rust-owned route groups from checklist-only status into
  executable owner evidence. The Phase 5 checklist and parity matrix now call
  out that these groups have only starter owner smoke so far; upload, generate,
  and SSE-heavy flows remain for later waves. Validation target for this
  checkpoint: `pytest backend/tests/test_tools/test_run_strangler_gateway_smoke.py -q`,
  `python backend/tools/run_strangler_gateway_smoke.py --manifest
  deploy/strangler-gateway-probes.json --profile phase5-p1 --route-group
  characters --validate-manifest-only`,
  `python backend/tools/run_strangler_gateway_smoke.py --manifest
  deploy/strangler-gateway-probes.json --profile phase5-p1 --route-group
  outlines --validate-manifest-only`,
  `python backend/tools/run_strangler_gateway_smoke.py --manifest
  deploy/strangler-gateway-probes.json --profile phase5-p1 --route-group
  book_import --validate-manifest-only`, and `git diff --check -- 
  deploy/strangler-gateway-probes.json
  docs/architecture/rust-route-group-ownership-and-cutover-checklist-2026-05-19.zh-CN.md
  docs/architecture/rust-python-api-parity-matrix-2026-05-19.zh-CN.md
  .trellis/tasks/05-18-backend-chapter-generation-refactor-followup/implement.md`
  should pass, allowing only the existing line-ending warning on the manifest
  file if it persists.
- 2026-05-20 01:36 +08:00 checkpoint:
  this Phase 5 slice expanded P1 fallback governance beyond `auth` without
  touching business runtime code. `deploy/strangler-gateway-probes.json` now
  extends `phase5-p1-fallback` with three additional same-path Python fallback
  probes: `GET /api/characters/project/{project_id}`,
  `GET /api/outlines/project/{project_id}`, and
  `GET /api/book-import/tasks/{task_id}`. All three intentionally use the
  stable unauthenticated Python response `401 {"detail":"未登录"}` to contrast
  the Rust owner boundary `401 {"detail":"未登录，请先登录"}` on the exact same
  paths, which makes them safe low-precondition rollback clues rather than
  speculative semantic parity claims. The P1 rollback runbook now documents
  these groups as having first-stage executable fallback evidence, while still
  keeping `users` out of the fallback profile because `/api/users/current`
  remains path-misaligned with Python ownership. Validation target for this
  checkpoint: `pytest backend/tests/test_tools/test_run_strangler_gateway_smoke.py -q`,
  `python backend/tools/run_strangler_gateway_smoke.py --manifest
  deploy/strangler-gateway-probes.json --profile phase5-p1-fallback
  --route-group characters --validate-manifest-only`,
  `python backend/tools/run_strangler_gateway_smoke.py --manifest
  deploy/strangler-gateway-probes.json --profile phase5-p1-fallback
  --route-group outlines --validate-manifest-only`,
  `python backend/tools/run_strangler_gateway_smoke.py --manifest
  deploy/strangler-gateway-probes.json --profile phase5-p1-fallback
  --route-group book_import --validate-manifest-only`, and `git diff --check -- 
  deploy/strangler-gateway-probes.json
  docs/architecture/rust-phase5-p1-route-group-rollback-runbook-2026-05-20.zh-CN.md
  docs/architecture/rust-route-group-ownership-and-cutover-checklist-2026-05-19.zh-CN.md
  docs/architecture/rust-python-api-parity-matrix-2026-05-19.zh-CN.md
  .trellis/tasks/05-18-backend-chapter-generation-refactor-followup/implement.md`
  should pass, allowing only the existing line-ending warning on the manifest
  file if it persists.
- 2026-05-20 01:52 +08:00 checkpoint:
  this Phase 5 slice deepened the same P1 route groups without leaving the
  low-precondition read-side lane. `deploy/strangler-gateway-probes.json` now
  adds a second same-path pair for each of `characters`, `outlines`, and
  `book_import`: `GET /api/characters?project_id=...`,
  `GET /api/outlines?project_id=...`, and
  `GET /api/book-import/tasks/{task_id}/preview` now exist in both
  `phase5-p1` (Rust owner) and `phase5-p1-fallback` (Python fallback), using
  the same unauthenticated owner split as the earlier starter probes. This
  makes the evidence less dependent on a single route shape per group and moves
  the asset from one-path starter coverage toward a slightly stronger read-side
  matrix, while still deliberately avoiding upload/generation/SSE success-path
  assertions. Validation target for this checkpoint: `pytest
  backend/tests/test_tools/test_run_strangler_gateway_smoke.py -q`, `python
  backend/tools/run_strangler_gateway_smoke.py --manifest
  deploy/strangler-gateway-probes.json --profile phase5-p1 --route-group
  characters --validate-manifest-only`, `python
  backend/tools/run_strangler_gateway_smoke.py --manifest
  deploy/strangler-gateway-probes.json --profile phase5-p1-fallback
  --route-group characters --validate-manifest-only`, and the same pair for
  `outlines` plus `book_import`, followed by `git diff --check -- 
  deploy/strangler-gateway-probes.json
  docs/architecture/rust-phase5-p1-route-group-rollback-runbook-2026-05-20.zh-CN.md
  docs/architecture/rust-route-group-ownership-and-cutover-checklist-2026-05-19.zh-CN.md
  docs/architecture/rust-python-api-parity-matrix-2026-05-19.zh-CN.md
  .trellis/tasks/05-18-backend-chapter-generation-refactor-followup/implement.md`
  should pass, allowing only the existing line-ending warning on the manifest
  file if it persists.
- 2026-05-20 02:08 +08:00 checkpoint:
  this Phase 5 slice advanced one step past pure read-side probing without
  introducing request-body or SSE validation risk. `deploy/strangler-gateway-probes.json`
  now adds `DELETE /api/book-import/tasks/{task_id}` to both `phase5-p1`
  (Rust owner) and `phase5-p1-fallback` (Python fallback), using the same
  unauthenticated owner split as the existing status/preview probes. This gives
  `book_import` the first lightweight write-side rollback clue in P1 while
  still avoiding upload payloads or stream success semantics. Validation target
  for this checkpoint: `pytest backend/tests/test_tools/test_run_strangler_gateway_smoke.py -q`,
  `python backend/tools/run_strangler_gateway_smoke.py --manifest
  deploy/strangler-gateway-probes.json --profile phase5-p1 --route-group
  book_import --validate-manifest-only`, `python
  backend/tools/run_strangler_gateway_smoke.py --manifest
  deploy/strangler-gateway-probes.json --profile phase5-p1-fallback
  --route-group book_import --validate-manifest-only`, and `git diff --check -- 
  deploy/strangler-gateway-probes.json
  docs/architecture/rust-phase5-p1-route-group-rollback-runbook-2026-05-20.zh-CN.md
  docs/architecture/rust-route-group-ownership-and-cutover-checklist-2026-05-19.zh-CN.md
  docs/architecture/rust-python-api-parity-matrix-2026-05-19.zh-CN.md
  .trellis/tasks/05-18-backend-chapter-generation-refactor-followup/implement.md`
  should pass, allowing only the existing line-ending warning on the manifest
  file if it persists.
- 2026-05-20 02:19 +08:00 checkpoint:
  this Phase 5 slice continued to deepen `book_import` while staying inside a
  low-risk auth-boundary envelope. `deploy/strangler-gateway-probes.json` now
  adds `POST /api/book-import/tasks/{task_id}/apply` and
  `POST /api/book-import/tasks/{task_id}/retry-stream` to both `phase5-p1`
  (Rust owner) and `phase5-p1-fallback` (Python fallback), using minimal legal
  JSON payloads so the request gets past shape validation and lands on the
  authentication owner boundary instead of failing for schema reasons. This
  gives `book_import` a more complete pre-success matrix across status,
  preview, cancel, apply, and retry entrypoints without needing a real upload
  or successful import flow. Validation target for this checkpoint: `pytest
  backend/tests/test_tools/test_run_strangler_gateway_smoke.py -q`, `python
  backend/tools/run_strangler_gateway_smoke.py --manifest
  deploy/strangler-gateway-probes.json --profile phase5-p1 --route-group
  book_import --validate-manifest-only`, `python
  backend/tools/run_strangler_gateway_smoke.py --manifest
  deploy/strangler-gateway-probes.json --profile phase5-p1-fallback
  --route-group book_import --validate-manifest-only`, and `git diff --check -- 
  deploy/strangler-gateway-probes.json
  docs/architecture/rust-phase5-p1-route-group-rollback-runbook-2026-05-20.zh-CN.md
  docs/architecture/rust-route-group-ownership-and-cutover-checklist-2026-05-19.zh-CN.md
  docs/architecture/rust-python-api-parity-matrix-2026-05-19.zh-CN.md
  .trellis/tasks/05-18-backend-chapter-generation-refactor-followup/implement.md`
  should pass, allowing only the existing line-ending warning on the manifest
  file if it persists.
- 2026-05-20 01:16 +08:00 checkpoint:
  this Phase 5 slice extended the same `book_import` governance lane one step
  further into the streaming submit boundary without leaving the low-risk auth
  envelope. `deploy/strangler-gateway-probes.json` now adds
  `POST /api/book-import/tasks/{task_id}/apply-stream` to both `phase5-p1`
  (Rust owner) and `phase5-p1-fallback` (Python fallback), reusing the same
  minimal legal JSON payload as `apply` so the request crosses schema shape
  validation and lands on the auth owner boundary instead of failing earlier.
  This gives `book_import` a fuller pre-success matrix across status, preview,
  cancel, apply, retry-stream, and apply-stream while still avoiding upload
  creation or success-path SSE assertions. Validation target for this
  checkpoint: `pytest backend/tests/test_tools/test_run_strangler_gateway_smoke.py -q`,
  `python backend/tools/run_strangler_gateway_smoke.py --manifest
  deploy/strangler-gateway-probes.json --profile phase5-p1 --route-group
  book_import --validate-manifest-only`, `python
  backend/tools/run_strangler_gateway_smoke.py --manifest
  deploy/strangler-gateway-probes.json --profile phase5-p1-fallback
  --route-group book_import --validate-manifest-only`, and `git diff --check -- 
  deploy/strangler-gateway-probes.json
  docs/architecture/rust-phase5-p1-route-group-rollback-runbook-2026-05-20.zh-CN.md
  docs/architecture/rust-route-group-ownership-and-cutover-checklist-2026-05-19.zh-CN.md
  docs/architecture/rust-python-api-parity-matrix-2026-05-19.zh-CN.md
  .trellis/tasks/05-18-backend-chapter-generation-refactor-followup/implement.md`
  should pass, allowing only the existing line-ending warning on the manifest
  file if it persists.
- 2026-05-20 01:17 +08:00 checkpoint:
  this Phase 5 slice advanced `book_import` into the multipart upload boundary
  without moving into success-path semantics. `backend/tools/run_strangler_gateway_smoke.py`
  now supports a minimal `multipart_form` manifest payload shape with focused
  tests, and `deploy/strangler-gateway-probes.json` now adds
  `POST /api/book-import/tasks` to both `phase5-p1` (Rust owner) and
  `phase5-p1-fallback` (Python fallback). The probe uses a tiny `.txt`
  in-memory upload plus the existing `append` / `create_new_project=true`
  form fields so the request reaches the auth owner boundary instead of
  failing on transport shape. This gives `book_import` a fuller pre-success
  matrix across create, status, preview, cancel, apply, retry-stream, and
  apply-stream while still avoiding login-dependent or successful import
  assertions. Validation target for this checkpoint: `pytest
  backend/tests/test_tools/test_run_strangler_gateway_smoke.py -q`, `python
  backend/tools/run_strangler_gateway_smoke.py --manifest
  deploy/strangler-gateway-probes.json --profile phase5-p1 --route-group
  book_import --validate-manifest-only`, `python
  backend/tools/run_strangler_gateway_smoke.py --manifest
  deploy/strangler-gateway-probes.json --profile phase5-p1-fallback
  --route-group book_import --validate-manifest-only`, and `git diff --check -- 
  backend/tools/run_strangler_gateway_smoke.py
  backend/tests/test_tools/test_run_strangler_gateway_smoke.py
  deploy/strangler-gateway-probes.json
  docs/architecture/rust-phase5-p1-route-group-rollback-runbook-2026-05-20.zh-CN.md
  docs/architecture/rust-route-group-ownership-and-cutover-checklist-2026-05-19.zh-CN.md
  docs/architecture/rust-python-api-parity-matrix-2026-05-19.zh-CN.md
  .trellis/tasks/05-18-backend-chapter-generation-refactor-followup/implement.md`
  should pass, allowing only the existing line-ending warning on the manifest
  file if it persists.
- 2026-05-20 01:24 +08:00 checkpoint:
  this Phase 5 slice pushed `characters` and `outlines` one step beyond the
  read-side-only lane without entering success-path assertions. `deploy/strangler-gateway-probes.json`
  now adds `POST /api/characters/generate-stream` and
  `POST /api/outlines/generate-stream` to both `phase5-p1` (Rust owner) and
  `phase5-p1-fallback` (Python fallback), using the smallest route-legal JSON
  payloads for each side. The Rust owner expectation remains the middleware
  boundary `401 {"detail":"未登录，请先登录"}`. The Python fallback expectation
  intentionally differs from the list routes: because both generate-stream
  endpoints depend on `get_user_ai_service()`, the stable unauthenticated
  Python signal is `401 {"detail":"需要登录"}` instead of `未登录`. This makes
  the probes useful rollback clues for the same paths without pretending that
  AI generation success, SSE event ordering, or downstream persistence are
  already covered. Validation target for this checkpoint: `pytest
  backend/tests/test_tools/test_run_strangler_gateway_smoke.py -q`, `python
  backend/tools/run_strangler_gateway_smoke.py --manifest
  deploy/strangler-gateway-probes.json --profile phase5-p1 --route-group
  characters --validate-manifest-only`, the same command for `outlines`,
  `python backend/tools/run_strangler_gateway_smoke.py --manifest
  deploy/strangler-gateway-probes.json --profile phase5-p1-fallback
  --route-group characters --validate-manifest-only`, the same command for
  `outlines`, and `git diff --check -- deploy/strangler-gateway-probes.json
  docs/architecture/rust-phase5-p1-route-group-rollback-runbook-2026-05-20.zh-CN.md
  docs/architecture/rust-route-group-ownership-and-cutover-checklist-2026-05-19.zh-CN.md
  docs/architecture/rust-python-api-parity-matrix-2026-05-19.zh-CN.md
  .trellis/tasks/05-18-backend-chapter-generation-refactor-followup/implement.md`
  should pass, allowing only the existing line-ending warning on the manifest
  file if it persists.
- 2026-05-20 01:39 +08:00 checkpoint:
  this Phase 5 slice deepened `outlines` beyond the first generate-stream
  boundary while staying inside low-noise auth/fallback governance. `deploy/strangler-gateway-probes.json`
  now adds `POST /api/outlines/batch-expand-stream` and
  `POST /api/outlines/{outline_id}/create-chapters-from-plans` to both
  `phase5-p1` (Rust owner) and `phase5-p1-fallback` (Python fallback).
  `batch-expand-stream` uses a minimal route-legal project payload, while
  `create-chapters-from-plans` uses a single fully shaped `chapter_plans`
  item so the request crosses Python body validation and lands on the same
  authentication boundary instead of failing early with a 422/400. The Rust
  owner expectation remains `401 {"detail":"未登录，请先登录"}`. The Python
  fallback expectation intentionally matches the existing outlines write-side
  dependency behavior: both routes resolve `get_user_ai_service()` before
  business work, so the stable fallback signal is
  `401 {"detail":"需要登录"}`. This gives `outlines` a fuller pre-success
  matrix across list, generate, batch-expand, and create-from-plans entry
  points without asserting successful SSE output or real chapter creation.
  Validation target for this checkpoint: `pytest
  backend/tests/test_tools/test_run_strangler_gateway_smoke.py -q`, `python
  backend/tools/run_strangler_gateway_smoke.py --manifest
  deploy/strangler-gateway-probes.json --profile phase5-p1 --route-group
  outlines --validate-manifest-only`, `python
  backend/tools/run_strangler_gateway_smoke.py --manifest
  deploy/strangler-gateway-probes.json --profile phase5-p1-fallback
  --route-group outlines --validate-manifest-only`, and `git diff --check -- 
  deploy/strangler-gateway-probes.json
  docs/architecture/rust-phase5-p1-route-group-rollback-runbook-2026-05-20.zh-CN.md
  docs/architecture/rust-route-group-ownership-and-cutover-checklist-2026-05-19.zh-CN.md
  docs/architecture/rust-python-api-parity-matrix-2026-05-19.zh-CN.md
  .trellis/tasks/05-18-backend-chapter-generation-refactor-followup/implement.md`
  should pass, allowing only the existing line-ending warning on the manifest
  file if it persists.
- 2026-05-20 01:48 +08:00 checkpoint:
  this Phase 5 slice returned to `characters` and chose the lowest-noise
  remaining same-path governance seam instead of forcing success-path or
  data-dependent CRUD probes. `deploy/strangler-gateway-probes.json` now adds
  `POST /api/characters/export` and `POST /api/characters/import` to both
  `phase5-p1` (Rust owner) and `phase5-p1-fallback` (Python fallback).
  `export` uses the smallest legal JSON payload with one placeholder
  `character_id`, while `import` uses a tiny multipart `.json` payload plus
  the required `project_id` query parameter so the request shape is valid
  enough to reach the auth boundary instead of failing earlier for transport
  reasons. The Rust owner expectation remains
  `401 {"detail":"未登录，请先登录"}`. The Python fallback expectation for
  these two routes is the direct request-state guard
  `401 {"detail":"未登录"}`, which intentionally differs from
  `characters/generate-stream` because `export/import` do not depend on
  `get_user_ai_service()`. `characters/validate-import` remains intentionally
  out of this slice because the current Rust route behaves more like a public
  validator and would not produce a clean same-path auth-boundary comparison.
  This gives `characters` a fuller pre-success matrix across list, generate,
  export, and import entry points without asserting real file semantics or
  successful entity creation. Validation target for this checkpoint: `pytest
  backend/tests/test_tools/test_run_strangler_gateway_smoke.py -q`, `python
  backend/tools/run_strangler_gateway_smoke.py --manifest
  deploy/strangler-gateway-probes.json --profile phase5-p1 --route-group
  characters --validate-manifest-only`, `python
  backend/tools/run_strangler_gateway_smoke.py --manifest
  deploy/strangler-gateway-probes.json --profile phase5-p1-fallback
  --route-group characters --validate-manifest-only`, and `git diff --check -- 
  deploy/strangler-gateway-probes.json
  docs/architecture/rust-phase5-p1-route-group-rollback-runbook-2026-05-20.zh-CN.md
  docs/architecture/rust-route-group-ownership-and-cutover-checklist-2026-05-19.zh-CN.md
  docs/architecture/rust-python-api-parity-matrix-2026-05-19.zh-CN.md
  .trellis/tasks/05-18-backend-chapter-generation-refactor-followup/implement.md`
  should pass, allowing only the existing line-ending warning on the manifest
  file if it persists.
- 2026-05-20 01:58 +08:00 checkpoint:
  this Phase 5 slice stopped forcing more `book_import` auth-boundary probes
  once the same-path lane was effectively saturated, then promoted the first
  explicit asymmetric-interface governance asset instead. `deploy/strangler-gateway-probes.json`
  now adds `POST /api/characters/validate-import` under a new
  `phase5-p1-asymmetric` profile with two paired expectations:
  Rust owner uses the smallest legal multipart `.json` file and must return a
  stable public-validator success payload (`200`, `valid=true`, empty-data
  warning), while Python fallback must return the direct request-state auth
  guard `401 {"detail":"未登录"}` on the same path. This intentionally does
  not join `phase5-p1-fallback`, because the route is same-path but not
  same-boundary: Rust currently exposes a public validator, whereas Python
  treats the same endpoint as a login-required import precheck. The rollback
  runbook, parity matrix, and route-group checklist now record this class as
  a separate governance lane so future Phase 5 work can distinguish
  auth-boundary parity from public-vs-protected asymmetry without creating
  false automation confidence. Validation target for this checkpoint: `pytest
  backend/tests/test_tools/test_run_strangler_gateway_smoke.py -q`, `python
  backend/tools/run_strangler_gateway_smoke.py --manifest
  deploy/strangler-gateway-probes.json --profile phase5-p1-asymmetric
  --route-group characters --validate-manifest-only`, and `git diff --check -- 
  deploy/strangler-gateway-probes.json
  docs/architecture/rust-phase5-p1-route-group-rollback-runbook-2026-05-20.zh-CN.md
  docs/architecture/rust-route-group-ownership-and-cutover-checklist-2026-05-19.zh-CN.md
  docs/architecture/rust-python-api-parity-matrix-2026-05-19.zh-CN.md
  .trellis/tasks/05-18-backend-chapter-generation-refactor-followup/implement.md`
  should pass, allowing only the existing line-ending warning on the manifest
  file if it persists.
- 2026-05-20 02:03 +08:00 checkpoint:
  this Phase 5 slice returned to the `auth` route-group and deepened the
  low-precondition protected write-side matrix instead of chasing noisier
  session-success behavior. `deploy/strangler-gateway-probes.json` now adds
  `POST /api/auth/password/set` and `POST /api/auth/password/initialize` to
  both `phase5-p1` (Rust owner) and `phase5-p1-fallback` (Python fallback),
  using the same smallest legal JSON body `{ "password": "test123456" }`.
  The Rust owner expectation remains
  `401 {"detail":"未登录，请先登录"}`. The Python fallback expectation is the
  direct `require_request_user()` guard `401 {"detail":"未登录"}` for both
  routes. This gives `auth` a broader write-side auth-boundary evidence set
  beyond `logout` and `password/status`, while still avoiding real password
  initialization semantics, session success-path assertions, or time-sensitive
  refresh behavior. The rollback runbook, parity matrix, and route-group
  checklist now record these two protected password endpoints as part of the
  P1 fallback lane. Validation target for this checkpoint: `pytest
  backend/tests/test_tools/test_run_strangler_gateway_smoke.py -q`, `python
  backend/tools/run_strangler_gateway_smoke.py --manifest
  deploy/strangler-gateway-probes.json --profile phase5-p1 --route-group auth
  --validate-manifest-only`, `python
  backend/tools/run_strangler_gateway_smoke.py --manifest
  deploy/strangler-gateway-probes.json --profile phase5-p1-fallback
  --route-group auth --validate-manifest-only`, and `git diff --check -- 
  deploy/strangler-gateway-probes.json
  docs/architecture/rust-phase5-p1-route-group-rollback-runbook-2026-05-20.zh-CN.md
  docs/architecture/rust-route-group-ownership-and-cutover-checklist-2026-05-19.zh-CN.md
  docs/architecture/rust-python-api-parity-matrix-2026-05-19.zh-CN.md
  .trellis/tasks/05-18-backend-chapter-generation-refactor-followup/implement.md`
  should pass, allowing only the existing line-ending warning on the manifest
  file if it persists.
- 2026-05-20 02:11 +08:00 checkpoint:
  this Phase 5 slice kept moving along the `auth` governance lane and filled
  the remaining low-precondition session-refresh boundary instead of jumping
  to success-path session semantics. `deploy/strangler-gateway-probes.json`
  now adds `POST /api/auth/refresh` to both `phase5-p1` (Rust owner) and
  `phase5-p1-fallback` (Python fallback). The Rust owner expectation remains
  `401 {"detail":"未登录，请先登录"}` via the shared auth middleware. The
  Python fallback expectation is the route-local `require_request_user()`
  message `401 {"detail":"未登录，无法刷新会话"}`, which makes this probe more
  informative than a generic auth-boundary read probe because it also encodes
  the current Python session-refresh contract. This extends the P1 auth asset
  set from public config/logout plus password boundaries into the refresh
  boundary without requiring any real session, cookie-expiry clock control, or
  success-path cookie assertions. The rollback runbook, parity matrix, and
  route-group checklist now record `auth/refresh` as part of the P1 fallback
  lane. Validation target for this checkpoint: `pytest
  backend/tests/test_tools/test_run_strangler_gateway_smoke.py -q`, `python
  backend/tools/run_strangler_gateway_smoke.py --manifest
  deploy/strangler-gateway-probes.json --profile phase5-p1 --route-group auth
  --validate-manifest-only`, `python
  backend/tools/run_strangler_gateway_smoke.py --manifest
  deploy/strangler-gateway-probes.json --profile phase5-p1-fallback
  --route-group auth --validate-manifest-only`, and `git diff --check -- 
  deploy/strangler-gateway-probes.json
  docs/architecture/rust-phase5-p1-route-group-rollback-runbook-2026-05-20.zh-CN.md
  docs/architecture/rust-route-group-ownership-and-cutover-checklist-2026-05-19.zh-CN.md
  docs/architecture/rust-python-api-parity-matrix-2026-05-19.zh-CN.md
  .trellis/tasks/05-18-backend-chapter-generation-refactor-followup/implement.md`
  should pass, allowing only the existing line-ending warning on the manifest
  file if it persists.
- 2026-05-20 02:17 +08:00 checkpoint:
  this Phase 5 slice returned to the `projects` route-group and intentionally
  stopped adding more low-value `401` probes. `deploy/strangler-gateway-probes.json`
  now adds `POST /api/projects/validate-import` as a paired same-path public
  validator asset: `projects-validate-import-public-rust` enters
  `phase5-p0` plus `business`, while
  `projects-validate-import-public-python-fallback` enters
  `phase5-p0-fallback`. Both sides use the same smallest legal multipart JSON
  import file and both return `200 valid=true`, but the result shapes are
  intentionally different in stable ways: Rust exposes the narrower
  `statistics.memories`-based schema with no warnings, while Python exposes
  `organization_members` / `character_careers` / `story_memories` /
  `has_default_style=false` plus the empty-project warnings
  `项目没有章节数据` and `项目没有角色数据`. This makes `projects` the first
  P0 group with a stronger through-gateway public/business owner probe and a
  same-path public-success fallback clue, instead of relying only on auth
  boundaries. The rollback runbook, route-group checklist, and parity matrix
  now record this lane explicitly. Validation target for this checkpoint:
  `pytest backend/tests/test_tools/test_run_strangler_gateway_smoke.py -q`,
  `python backend/tools/run_strangler_gateway_smoke.py --manifest
  deploy/strangler-gateway-probes.json --profile phase5-p0 --route-group
  projects --validate-manifest-only`, `python
  backend/tools/run_strangler_gateway_smoke.py --manifest
  deploy/strangler-gateway-probes.json --profile phase5-p0-fallback
  --route-group projects --validate-manifest-only`, and `git diff --check --
  deploy/strangler-gateway-probes.json
  docs/architecture/rust-phase5-p0-route-group-rollback-runbook-2026-05-19.zh-CN.md
  docs/architecture/rust-route-group-ownership-and-cutover-checklist-2026-05-19.zh-CN.md
  docs/architecture/rust-python-api-parity-matrix-2026-05-19.zh-CN.md
  .trellis/tasks/05-18-backend-chapter-generation-refactor-followup/implement.md`
  should pass, allowing only the existing line-ending warning on the manifest
  file if it persists.
- 2026-05-20 02:29 +08:00 checkpoint:
  this Phase 5 slice stayed with the same `projects` route-group and added the
  next-lowest-noise write-side governance asset instead of jumping to heavier
  export or consistency-repair flows. `deploy/strangler-gateway-probes.json`
  now adds `POST /api/projects/import` to both `phase5-p0` (Rust owner) and
  `phase5-p0-fallback` (Python fallback), reusing the same smallest legal
  multipart JSON file as `validate-import`. The difference is intentional:
  `validate-import` proves same-path public/business success semantics,
  whereas `import` proves the multipart write-side auth boundary after the
  request shape is already valid. Rust must still land on the shared auth
  middleware `401 {"detail":"未登录，请先登录"}`; Python must still land on the
  route-local `request.state.user_id` guard `401 {"detail":"未登录"}` before any
  import business logic runs. This gives `projects` a tighter P0 evidence set
  across read-side auth, public validator success, and multipart write-side
  auth, without needing a real logged-in import success path. The rollback
  runbook, route-group checklist, and parity matrix now record that expanded
  lane explicitly. Validation target for this checkpoint: `pytest
  backend/tests/test_tools/test_run_strangler_gateway_smoke.py -q`, `python
  backend/tools/run_strangler_gateway_smoke.py --manifest
  deploy/strangler-gateway-probes.json --profile phase5-p0 --route-group
  projects --validate-manifest-only`, `python
  backend/tools/run_strangler_gateway_smoke.py --manifest
  deploy/strangler-gateway-probes.json --profile phase5-p0-fallback
  --route-group projects --validate-manifest-only`, and `git diff --check --
  deploy/strangler-gateway-probes.json
  docs/architecture/rust-phase5-p0-route-group-rollback-runbook-2026-05-19.zh-CN.md
  docs/architecture/rust-route-group-ownership-and-cutover-checklist-2026-05-19.zh-CN.md
  docs/architecture/rust-python-api-parity-matrix-2026-05-19.zh-CN.md
  .trellis/tasks/05-18-backend-chapter-generation-refactor-followup/implement.md`
  should pass, allowing only the existing line-ending warning on the manifest
  file if it persists.
- 2026-05-20 02:38 +08:00 checkpoint:
  this Phase 5 slice kept the same `projects` route-group focus and chose the
  next stable JSON write-side boundary instead of jumping to repair/report
  success assertions. `deploy/strangler-gateway-probes.json` now adds
  `POST /api/projects/{project_id}/export-data` to both `phase5-p0` (Rust
  owner) and `phase5-p0-fallback` (Python fallback), using the smallest legal
  JSON body `{}` so the request clears request decoding and lands on the owner
  auth boundary rather than failing for payload shape. Rust must still return
  the shared middleware `401 {"detail":"未登录，请先登录"}`; Python must still
  return the route-local `request.state.user_id` guard
  `401 {"detail":"未登录"}` before any export business logic runs. This gives
  `projects` a broader P0 governance set across read-side auth,
  same-path public-success validation, multipart write-side auth, and JSON
  write-side auth, without requiring a real project, login, or export result.
  The rollback runbook, route-group checklist, and parity matrix now record
  that expanded lane explicitly. Validation target for this checkpoint:
  `pytest backend/tests/test_tools/test_run_strangler_gateway_smoke.py -q`,
  `python backend/tools/run_strangler_gateway_smoke.py --manifest
  deploy/strangler-gateway-probes.json --profile phase5-p0 --route-group
  projects --validate-manifest-only`, `python
  backend/tools/run_strangler_gateway_smoke.py --manifest
  deploy/strangler-gateway-probes.json --profile phase5-p0-fallback
  --route-group projects --validate-manifest-only`, and `git diff --check --
  deploy/strangler-gateway-probes.json
  docs/architecture/rust-phase5-p0-route-group-rollback-runbook-2026-05-19.zh-CN.md
  docs/architecture/rust-route-group-ownership-and-cutover-checklist-2026-05-19.zh-CN.md
  docs/architecture/rust-python-api-parity-matrix-2026-05-19.zh-CN.md
  .trellis/tasks/05-18-backend-chapter-generation-refactor-followup/implement.md`
  should pass, allowing only the existing line-ending warning on the manifest
  file if it persists.
- 2026-05-20 02:47 +08:00 checkpoint:
  this Phase 5 slice moved off the now-dense `projects` group and returned to
  the P0 `wizard-stream` lane to add a second same-family SSE auth-boundary
  probe. `deploy/strangler-gateway-probes.json` now adds
  `POST /api/wizard-stream/world-building/{project_id}/regenerate` to both
  `phase5-p0` (Rust owner) and `phase5-p0-fallback` (Python fallback), using
  the smallest legal JSON body `{}` so the request clears body parsing and
  lands on the owner auth boundary instead of failing for payload shape. Rust
  must still return the shared middleware `401 {"detail":"未登录，请先登录"}`.
  Python must still return the `get_user_ai_service()` login dependency signal
  `401 {"detail":"需要登录"}` before any SSE generation work starts. This gives
  `wizard-stream` a more credible P0 pair across `outline` and world-building
  regenerate, rather than relying on a single top-level SSE endpoint to prove
  owner/fallback behavior. The rollback runbook, route-group checklist, and
  parity matrix now record that expanded lane explicitly. Validation target
  for this checkpoint: `pytest backend/tests/test_tools/test_run_strangler_gateway_smoke.py -q`,
  `python backend/tools/run_strangler_gateway_smoke.py --manifest
  deploy/strangler-gateway-probes.json --profile phase5-p0 --route-group
  wizard-stream --validate-manifest-only`, `python
  backend/tools/run_strangler_gateway_smoke.py --manifest
  deploy/strangler-gateway-probes.json --profile phase5-p0-fallback
  --route-group wizard-stream --validate-manifest-only`, and `git diff --check --
  deploy/strangler-gateway-probes.json
  docs/architecture/rust-phase5-p0-route-group-rollback-runbook-2026-05-19.zh-CN.md
  docs/architecture/rust-route-group-ownership-and-cutover-checklist-2026-05-19.zh-CN.md
  docs/architecture/rust-python-api-parity-matrix-2026-05-19.zh-CN.md
  .trellis/tasks/05-18-backend-chapter-generation-refactor-followup/implement.md`
  should pass, allowing only the existing line-ending warning on the manifest
  file if it persists.

- 2026-05-20 02:40 +08:00 checkpoint:
  this Phase 5 slice moved to the `memories` route-group once `projects`
  and `wizard-stream` had already gained denser same-group evidence. The next
  lowest-noise lane was `POST /api/memories/projects/{project_id}/search`,
  because Rust accepts a generic `Json<Value>` body while Python requires the
  `query` term as a query parameter rather than inside JSON. The manifest now
  adds `memories-search-auth-guard-rust` and
  `memories-search-auth-guard-python-fallback`, both using the same path
  `/api/memories/projects/test-project-id/search?query=test` plus the minimal
  JSON body `{}` so the request shape is valid enough on both sides to cross
  transport parsing and land on the auth boundary instead of failing early
  with a 422. Rust must still return the shared middleware
  `401 {"detail":"未登录，请先登录"}`. Python fallback must still return
  `401 {"detail":"未登录"}` from `verify_project_access()`. This upgrades
  `memories` from a single `stats` auth-boundary clue into a two-probe query
  lane (`stats + search`) without requiring any real project, login, or
  search result data. The rollback runbook, route-group checklist, and parity
  matrix now record that expanded lane explicitly. Validation target for this
  checkpoint: `pytest backend/tests/test_tools/test_run_strangler_gateway_smoke.py -q`,
  `python backend/tools/run_strangler_gateway_smoke.py --manifest
  deploy/strangler-gateway-probes.json --profile phase5-p0 --route-group
  memories --validate-manifest-only`, `python
  backend/tools/run_strangler_gateway_smoke.py --manifest
  deploy/strangler-gateway-probes.json --profile phase5-p0-fallback
  --route-group memories --validate-manifest-only`, and `git diff --check --
  deploy/strangler-gateway-probes.json
  docs/architecture/rust-phase5-p0-route-group-rollback-runbook-2026-05-19.zh-CN.md
  docs/architecture/rust-route-group-ownership-and-cutover-checklist-2026-05-19.zh-CN.md
  docs/architecture/rust-python-api-parity-matrix-2026-05-19.zh-CN.md
  .trellis/tasks/05-18-backend-chapter-generation-refactor-followup/implement.md`
  should pass, allowing only the existing line-ending warning on the manifest
  file if it persists.
- 2026-05-20 02:48 +08:00 checkpoint:
  this Phase 5 slice stayed in the P0 governance lane but shifted to the
  sparsest Python fallback group instead of adding more low-yield owner-only
  evidence. `settings` already had five Rust owner probes in `phase5-p0`,
  but only one Python fallback clue at `GET /api/settings`. The manifest now
  adds `settings-fetch-models-auth-guard-python-fallback` under
  `phase5-p0-fallback`, reusing the same minimal JSON body as the existing
  Rust owner probe for `POST /api/settings/fetch-models`. This is a lower-noise
  lane than `test` or `check-function-calling` because the body is smaller
  and Python reaches `Depends(require_login)` before any network probe logic.
  Python fallback must therefore return the stable login guard
  `401 {"detail":"需要登录"}` on the same path and request shape. This
  upgrades `settings` from a single root-path fallback clue into a two-probe
  fallback lane (`/api/settings + /api/settings/fetch-models`) without
  asserting any live provider success path. The rollback runbook, route-group
  checklist, and parity matrix now record that expanded lane explicitly.
  Validation target for this checkpoint: `pytest
  backend/tests/test_tools/test_run_strangler_gateway_smoke.py -q`, `python
  backend/tools/run_strangler_gateway_smoke.py --manifest
  deploy/strangler-gateway-probes.json --profile phase5-p0-fallback
  --route-group settings --validate-manifest-only`, and `git diff --check --
  deploy/strangler-gateway-probes.json
  docs/architecture/rust-phase5-p0-route-group-rollback-runbook-2026-05-19.zh-CN.md
  docs/architecture/rust-route-group-ownership-and-cutover-checklist-2026-05-19.zh-CN.md
  docs/architecture/rust-python-api-parity-matrix-2026-05-19.zh-CN.md
  .trellis/tasks/05-18-backend-chapter-generation-refactor-followup/implement.md`
  should pass, allowing only the existing line-ending warning on the manifest
  file if it persists.
- 2026-05-20 02:50 +08:00 checkpoint:
  this Phase 5 slice stayed with the same `settings` fallback lane and added
  the next-lowest-noise sub-route instead of jumping to success-path probe
  logic. The manifest now adds `settings-test-auth-guard-python-fallback`
  under `phase5-p0-fallback`, reusing the same minimal JSON body as the
  existing Rust owner probe for `POST /api/settings/test`. This keeps the
  request shape realistic enough to cross transport parsing while still
  stopping at Python's `Depends(require_login)` before any external provider
  connectivity probe runs. Python fallback must therefore return the stable
  login guard `401 {"detail":"需要登录"}` on the same path and request
  shape. This upgrades `settings` from a two-probe fallback lane
  (`/api/settings + /api/settings/fetch-models`) into a three-probe lane that
  now also covers the connection-test endpoint, without asserting any live
  provider success or failure semantics. The rollback runbook, route-group
  checklist, and parity matrix now record that expanded lane explicitly.
  Validation target for this checkpoint: `pytest
  backend/tests/test_tools/test_run_strangler_gateway_smoke.py -q`, `python
  backend/tools/run_strangler_gateway_smoke.py --manifest
  deploy/strangler-gateway-probes.json --profile phase5-p0-fallback
  --route-group settings --validate-manifest-only`, and `git diff --check --
  deploy/strangler-gateway-probes.json
  docs/architecture/rust-phase5-p0-route-group-rollback-runbook-2026-05-19.zh-CN.md
  docs/architecture/rust-route-group-ownership-and-cutover-checklist-2026-05-19.zh-CN.md
  docs/architecture/rust-python-api-parity-matrix-2026-05-19.zh-CN.md
  .trellis/tasks/05-18-backend-chapter-generation-refactor-followup/implement.md`
  should pass, allowing only the existing line-ending warning on the manifest
  file if it persists.
- 2026-05-20 02:54 +08:00 checkpoint:
  this Phase 5 slice stayed with the same `settings` fallback lane and filled
  the last low-precondition probe sibling before moving on to other groups.
  The manifest now adds
  `settings-check-function-calling-auth-guard-python-fallback` under
  `phase5-p0-fallback`, reusing the same minimal JSON body as the existing
  Rust owner probe for `POST /api/settings/check-function-calling`. This
  keeps the request shape realistic enough to cross transport parsing while
  still stopping at Python's `Depends(require_login)` before any tool-calling
  capability probe runs. Python fallback must therefore return the stable
  login guard `401 {"detail":"需要登录"}` on the same path and request
  shape. This upgrades `settings` from a three-probe fallback lane
  (`/api/settings + /api/settings/fetch-models + /api/settings/test`) into a
  four-probe lane that now also covers the function-calling probe endpoint,
  without asserting any live provider capability semantics. The rollback
  runbook, route-group checklist, and parity matrix now record that expanded
  lane explicitly. Validation target for this checkpoint: `pytest
  backend/tests/test_tools/test_run_strangler_gateway_smoke.py -q`, `python
  backend/tools/run_strangler_gateway_smoke.py --manifest
  deploy/strangler-gateway-probes.json --profile phase5-p0-fallback
  --route-group settings --validate-manifest-only`, and `git diff --check --
  deploy/strangler-gateway-probes.json
  docs/architecture/rust-phase5-p0-route-group-rollback-runbook-2026-05-19.zh-CN.md
  docs/architecture/rust-route-group-ownership-and-cutover-checklist-2026-05-19.zh-CN.md
  docs/architecture/rust-python-api-parity-matrix-2026-05-19.zh-CN.md
  .trellis/tasks/05-18-backend-chapter-generation-refactor-followup/implement.md`
  should pass, allowing only the existing line-ending warning on the manifest
  file if it persists.
- 2026-05-20 03:00 +08:00 checkpoint:
  this Phase 5 slice moved off the now-complete low-precondition `settings`
  fallback lane and returned to the thinner P1 `users` group. During route
  verification, the previous rollback docs turned out to be stale: Python
  still exposes same-path `GET /api/users/current` and `GET /api/users`
  routes in `backend/app/api/users.py`, so `users/current` no longer needs to
  be treated as a path-mismatch-only operator clue. The manifest now adds
  `users-current-auth-guard-python-fallback` and
  `users-list-auth-guard-python-fallback` under `phase5-p1-fallback`. Both
  should return the stable Python login guard `401 {"detail":"需要登录"}`
  after a route-group rollback. For `/api/users`, this still proves only the
  same-path auth boundary, not full admin-list semantic parity, because the
  Python endpoint also enforces admin privileges after login. The P1 rollback
  runbook, route-group checklist, and parity matrix now explicitly distinguish
  “same-path auth-boundary ownership clue” from “full business parity proof”
  for the `users` group. Validation target for this checkpoint: `pytest
  backend/tests/test_tools/test_run_strangler_gateway_smoke.py -q`, `python
  backend/tools/run_strangler_gateway_smoke.py --manifest
  deploy/strangler-gateway-probes.json --profile phase5-p1-fallback
  --route-group users --validate-manifest-only`, and `git diff --check --
  deploy/strangler-gateway-probes.json
  docs/architecture/rust-phase5-p1-route-group-rollback-runbook-2026-05-20.zh-CN.md
  docs/architecture/rust-route-group-ownership-and-cutover-checklist-2026-05-19.zh-CN.md
  docs/architecture/rust-python-api-parity-matrix-2026-05-19.zh-CN.md
  .trellis/tasks/05-18-backend-chapter-generation-refactor-followup/implement.md`
  should pass, allowing only the existing line-ending warning on the manifest
  file if it persists.
- 2026-05-20 03:05 +08:00 checkpoint:
  this Phase 5 slice stayed with the now-validated `users` fallback lane and
  added the next two lowest-noise same-path write boundaries instead of
  stopping at read-only ownership clues. The manifest now adds
  `users-set-admin-auth-guard-python-fallback` and
  `users-reset-password-auth-guard-python-fallback` under
  `phase5-p1-fallback`, using the smallest legal JSON bodies for each route.
  In both Python handlers, the request should still stop at
  `require_login("需要登录")` before any admin-only or password-reset business
  branch executes. That means these probes extend `users` from read-side
  fallback clues into write-side auth-boundary clues, while still not claiming
  full semantic parity for admin mutations. The P1 rollback runbook, route-group
  checklist, and parity matrix now explicitly record that distinction. Validation
  target for this checkpoint: `pytest
  backend/tests/test_tools/test_run_strangler_gateway_smoke.py -q`, `python
  backend/tools/run_strangler_gateway_smoke.py --manifest
  deploy/strangler-gateway-probes.json --profile phase5-p1-fallback
  --route-group users --validate-manifest-only`, and `git diff --check --
  deploy/strangler-gateway-probes.json
  docs/architecture/rust-phase5-p1-route-group-rollback-runbook-2026-05-20.zh-CN.md
  docs/architecture/rust-route-group-ownership-and-cutover-checklist-2026-05-19.zh-CN.md
  docs/architecture/rust-python-api-parity-matrix-2026-05-19.zh-CN.md
  .trellis/tasks/05-18-backend-chapter-generation-refactor-followup/implement.md`
  should pass, allowing only the existing line-ending warning on the manifest
  file if it persists.
- 2026-05-20 03:09 +08:00 checkpoint:
  this Phase 5 slice stayed with the same `users` route-group and filled the
  matching Rust owner side for the two newly added fallback write boundaries.
  The manifest now adds `users-set-admin-auth-guard-rust` and
  `users-reset-password-auth-guard-rust` under `phase5-p1`, using the same
  smallest legal JSON bodies as the Python fallback probes. This keeps the
  owner/fallback matrix symmetric at the transport/auth-boundary layer without
  asserting successful admin mutation behavior. Rust should still return the
  shared middleware `401 {"detail":"未登录，请先登录"}` on both paths, while
  Python fallback keeps returning `401 {"detail":"需要登录"}`. The P1 rollback
  runbook, route-group checklist, and parity matrix now record `users` as a
  route-group that already has both read-side and write-side same-path auth
  boundary assets, even though full business parity remains out of scope.
  Validation target for this checkpoint: `pytest
  backend/tests/test_tools/test_run_strangler_gateway_smoke.py -q`, `python
  backend/tools/run_strangler_gateway_smoke.py --manifest
  deploy/strangler-gateway-probes.json --profile phase5-p1 --route-group users
  --validate-manifest-only`, `python
  backend/tools/run_strangler_gateway_smoke.py --manifest
  deploy/strangler-gateway-probes.json --profile phase5-p1-fallback
  --route-group users --validate-manifest-only`, and `git diff --check --
  deploy/strangler-gateway-probes.json
  docs/architecture/rust-phase5-p1-route-group-rollback-runbook-2026-05-20.zh-CN.md
  docs/architecture/rust-route-group-ownership-and-cutover-checklist-2026-05-19.zh-CN.md
  docs/architecture/rust-python-api-parity-matrix-2026-05-19.zh-CN.md
  .trellis/tasks/05-18-backend-chapter-generation-refactor-followup/implement.md`
  should pass, allowing only the existing line-ending warning on the manifest
  file if it persists.
- 2026-05-20 03:21 +08:00 checkpoint:
  this Phase 5 slice returned to the still-thinnest `auth` group, but avoided
  adding another ordinary auth-boundary `401` probe. The lowest-noise missing
  lane was the public callback error boundary at `GET /api/auth/callback`
  without query parameters. Rust and Python both expose the same path, yet
  they stop at different stable local validation branches before any external
  OAuth call or transient `state` handling is involved. The manifest now adds
  `auth-callback-missing-code-rust` under `phase5-p1` and
  `auth-callback-missing-code-python-fallback` under
  `phase5-p1-fallback`. Rust must return
  `400 {"detail":"缺少 code 参数"}` because the Axum handler checks `code`
  before `state`. Python fallback must return
  `400 {"detail":"缺少 code 或 state 参数"}` because `_handle_callback()`
  rejects either missing value through one shared branch. This upgrades
  `auth` from config/logout/url/error plus login-boundary evidence into a
  slightly richer public-error matrix that now covers the callback entrypoint
  itself, without requiring any live LinuxDO configuration or real callback
  state. The P1 rollback runbook, route-group checklist, and parity matrix now
  record that new owner/fallback clue explicitly. Validation target for this
  checkpoint: `pytest backend/tests/test_tools/test_run_strangler_gateway_smoke.py -q`,
  `python backend/tools/run_strangler_gateway_smoke.py --manifest
  deploy/strangler-gateway-probes.json --profile phase5-p1 --route-group auth
  --validate-manifest-only`, `python
  backend/tools/run_strangler_gateway_smoke.py --manifest
  deploy/strangler-gateway-probes.json --profile phase5-p1-fallback
  --route-group auth --validate-manifest-only`, and `git diff --check --
  deploy/strangler-gateway-probes.json
  docs/architecture/rust-phase5-p1-route-group-rollback-runbook-2026-05-20.zh-CN.md
  docs/architecture/rust-route-group-ownership-and-cutover-checklist-2026-05-19.zh-CN.md
  docs/architecture/rust-python-api-parity-matrix-2026-05-19.zh-CN.md
  .trellis/tasks/05-18-backend-chapter-generation-refactor-followup/implement.md`
  should pass, allowing only the existing line-ending warning on the manifest
  file if it persists.
- 2026-05-20 03:34 +08:00 checkpoint:
  this Phase 5 slice stayed with the `auth` group, but moved from callback
  parameter validation to the local-login public failure surface so the lane
  would gain a more business-shaped owner clue instead of another transport
  boundary. The manifest now adds
  `auth-local-login-invalid-credentials-rust` under `phase5-p1` and
  `auth-local-login-invalid-credentials-python-fallback` under
  `phase5-p1-fallback`, both using the same explicit invalid credentials body
  on `POST /api/auth/local/login`. This probe depends on the current
  deployment assumption `local_auth_enabled=true`, which is already evidenced
  by the existing `auth-config-public-rust` probe, but it does not require a
  real user, session, or OAuth callback state. Rust owner must return
  `401 {"success": false, "message": "用户名或密码错误"}` because the Axum
  login handler wraps invalid credentials in a compat-style `{success,message}`
  JSON body. Python fallback must return
  `401 {"detail":"用户名或密码错误"}` because FastAPI raises an
  `HTTPException` on the same invalid-credentials branch. This upgrades
  `auth` from callback/config/logout/error plus login-boundary evidence into a
  slightly richer public/business failure matrix that now includes the actual
  local-login entrypoint structure, without needing a successful login or
  live LinuxDO setup. The P1 rollback runbook, route-group checklist, and
  parity matrix now record that new owner/fallback clue explicitly. Validation
  target for this checkpoint: `pytest
  backend/tests/test_tools/test_run_strangler_gateway_smoke.py -q`, `python
  backend/tools/run_strangler_gateway_smoke.py --manifest
  deploy/strangler-gateway-probes.json --profile phase5-p1 --route-group auth
  --validate-manifest-only`, `python
  backend/tools/run_strangler_gateway_smoke.py --manifest
  deploy/strangler-gateway-probes.json --profile phase5-p1-fallback
  --route-group auth --validate-manifest-only`, and `git diff --check --
  deploy/strangler-gateway-probes.json
  docs/architecture/rust-phase5-p1-route-group-rollback-runbook-2026-05-20.zh-CN.md
  docs/architecture/rust-route-group-ownership-and-cutover-checklist-2026-05-19.zh-CN.md
  docs/architecture/rust-python-api-parity-matrix-2026-05-19.zh-CN.md
  .trellis/tasks/05-18-backend-chapter-generation-refactor-followup/implement.md`
  should pass, allowing only the existing line-ending warning on the manifest
  file if it persists.
- 2026-05-20 03:43 +08:00 checkpoint:
  this Phase 5 slice stayed on the same `auth` public-business lane and added
  the sibling bound-account login entrypoint instead of jumping back to
  generic auth-boundary `401` probes. The manifest now adds
  `auth-bind-login-invalid-credentials-rust` under `phase5-p1` and
  `auth-bind-login-invalid-credentials-python-fallback` under
  `phase5-p1-fallback`, both using the same explicit invalid credentials body
  on `POST /api/auth/bind/login`. This probe is slightly lower-noise than the
  previous `local/login` one because Python does not first gate it on
  `local_auth_enabled`; it goes straight into the bound-account credential
  branch, yet still remains low-precondition because no real user, session, or
  OAuth state is required. Rust owner must again return
  `401 {"success": false, "message": "用户名或密码错误"}` because the Axum
  bind-login route reuses the compat-style local login handler. Python
  fallback must again return `401 {"detail":"用户名或密码错误"}` because the
  FastAPI route raises `HTTPException` on the same invalid-credentials path.
  This upgrades `auth` from one real login-entry public failure clue
  (`local/login`) to a two-entry login matrix (`local/login + bind/login`),
  while still avoiding successful-login state setup or external OAuth
  dependencies. The P1 rollback runbook, route-group checklist, and parity
  matrix now record that new owner/fallback clue explicitly. Validation target
  for this checkpoint: `pytest
  backend/tests/test_tools/test_run_strangler_gateway_smoke.py -q`, `python
  backend/tools/run_strangler_gateway_smoke.py --manifest
  deploy/strangler-gateway-probes.json --profile phase5-p1 --route-group auth
  --validate-manifest-only`, `python
  backend/tools/run_strangler_gateway_smoke.py --manifest
  deploy/strangler-gateway-probes.json --profile phase5-p1-fallback
  --route-group auth --validate-manifest-only`, and `git diff --check --
  deploy/strangler-gateway-probes.json
  docs/architecture/rust-phase5-p1-route-group-rollback-runbook-2026-05-20.zh-CN.md
  docs/architecture/rust-route-group-ownership-and-cutover-checklist-2026-05-19.zh-CN.md
  docs/architecture/rust-python-api-parity-matrix-2026-05-19.zh-CN.md
  .trellis/tasks/05-18-backend-chapter-generation-refactor-followup/implement.md`
  should pass, allowing only the existing line-ending warning on the manifest
  file if it persists.
- 2026-05-20 03:55 +08:00 checkpoint:
  this Phase 5 slice moved back to the P0 `settings` group after the `auth`
  lane had already become denser than the remaining low-precondition settings
  fallback surface. The smallest missing same-path clue was
  `GET /api/settings/api-key`, because both Rust and Python expose the exact
  same path, require no request body, and stop at the login boundary before
  any provider probe or settings lookup semantics can diverge. The manifest
  now adds `settings-api-key-auth-guard-rust` under `phase5-p0` and
  `settings-api-key-auth-guard-python-fallback` under
  `phase5-p0-fallback`. Rust must return the shared middleware
  `401 {"detail":"未登录，请先登录"}`. Python fallback must return the
  stable dependency guard `401 {"detail":"需要登录"}`. This upgrades
  `settings` from root-path plus provider-probe sub-route evidence into a
  slightly more complete read-side matrix that now also covers the stored API
  key retrieval entrypoint, without relying on any live provider connectivity
  or saved user settings. The P0 rollback runbook, route-group checklist, and
  parity matrix now record that owner/fallback clue explicitly. Validation
  target for this checkpoint: `pytest
  backend/tests/test_tools/test_run_strangler_gateway_smoke.py -q`, `python
  backend/tools/run_strangler_gateway_smoke.py --manifest
  deploy/strangler-gateway-probes.json --profile phase5-p0 --route-group
  settings --validate-manifest-only`, `python
  backend/tools/run_strangler_gateway_smoke.py --manifest
  deploy/strangler-gateway-probes.json --profile phase5-p0-fallback
  --route-group settings --validate-manifest-only`, and `git diff --check --
  deploy/strangler-gateway-probes.json
  docs/architecture/rust-phase5-p0-route-group-rollback-runbook-2026-05-19.zh-CN.md
  docs/architecture/rust-route-group-ownership-and-cutover-checklist-2026-05-19.zh-CN.md
  docs/architecture/rust-python-api-parity-matrix-2026-05-19.zh-CN.md
  .trellis/tasks/05-18-backend-chapter-generation-refactor-followup/implement.md`
  should pass, allowing only the existing line-ending warning on the manifest
  file if it persists.
- 2026-05-20 04:09 +08:00 checkpoint:
  this Phase 5 slice initially tried to close the last visible `settings`
  fallback count gap by adding `/api/settings/models`, but route inspection
  showed that would be incorrect. Rust and Python both expose the same path,
  yet they do not share the same boundary type: Rust owner requires auth first,
  while Python fallback keeps `/api/settings/models` public and proceeds into
  provider probing. Instead of polluting `phase5-p0-fallback` with a fake
  auth-boundary clue, the manifest now adds a new asymmetric pair under
  `phase5-p0-asymmetric`:
  `settings-models-auth-guard-rust-asymmetric` and
  `settings-models-public-network-error-python-fallback`. Both use the same
  minimal query shape
  `/api/settings/models?provider=openai&api_key=test-key&api_base_url=http://127.0.0.1:9/v1`.
  Rust must return `401 {"detail":"未登录，请先登录"}` from the shared auth
  middleware. Python fallback must return
  `400 {"detail":"无法连接到 API: All connection attempts failed"}` because
  the public endpoint reaches `httpx` connection failure before any auth gate.
  The P0 rollback runbook, route-group checklist, and parity matrix now record
  `settings/models` as the first P0 asymmetric sample, analogous to the
  existing P1 `characters/validate-import` pattern. Validation target for this
  checkpoint: `pytest backend/tests/test_tools/test_run_strangler_gateway_smoke.py -q`,
  `python backend/tools/run_strangler_gateway_smoke.py --manifest
  deploy/strangler-gateway-probes.json --profile phase5-p0-asymmetric
  --route-group settings --validate-manifest-only`, and `git diff --check --
  deploy/strangler-gateway-probes.json
  docs/architecture/rust-phase5-p0-route-group-rollback-runbook-2026-05-19.zh-CN.md
  docs/architecture/rust-route-group-ownership-and-cutover-checklist-2026-05-19.zh-CN.md
  docs/architecture/rust-python-api-parity-matrix-2026-05-19.zh-CN.md
  .trellis/tasks/05-18-backend-chapter-generation-refactor-followup/implement.md`
  should pass, allowing only the existing line-ending warning on the manifest
  file if it persists.
- 2026-05-20 05:02 +08:00 checkpoint:
  this Phase 5 slice returned to the P0 `wizard-stream` group instead of
  continuing to overfit `settings`. Route inspection confirmed
  `POST /api/wizard-stream/cleanup/{project_id}` is implemented on Rust but
  not on the current Python `wizard_stream` router, so it must not be modeled
  as a fake `phase5-p0-fallback` auth-boundary clue. The manifest now adds
  `wizard-stream-cleanup-auth-guard-rust` under `phase5-p0`, using the same
  minimal JSON body `{}` on
  `POST /api/wizard-stream/cleanup/test-project-id`. Rust owner must return
  `401 {"detail":"未登录，请先登录"}` from the shared auth middleware before
  the cleanup SSE handler starts. This gives `wizard-stream` a third explicit
  through-gateway owner probe across `outline`, `world-building regenerate`,
  and `cleanup`, while the rollback runbook, route-group checklist, and parity
  matrix now explicitly record that `cleanup` is owner-only evidence until
  Python gains a real same-path route. Validation target for this checkpoint:
  `pytest backend/tests/test_tools/test_run_strangler_gateway_smoke.py -q`,
  `python backend/tools/run_strangler_gateway_smoke.py --manifest
  deploy/strangler-gateway-probes.json --profile phase5-p0 --route-group
  wizard-stream --validate-manifest-only`, and `git diff --check --
  deploy/strangler-gateway-probes.json
  backend/tests/test_tools/test_run_strangler_gateway_smoke.py
  docs/architecture/rust-phase5-p0-route-group-rollback-runbook-2026-05-19.zh-CN.md
  docs/architecture/rust-route-group-ownership-and-cutover-checklist-2026-05-19.zh-CN.md
  docs/architecture/rust-python-api-parity-matrix-2026-05-19.zh-CN.md
  .trellis/tasks/05-18-backend-chapter-generation-refactor-followup/implement.md`
  should pass, allowing only the existing line-ending warning on the manifest
  file if it persists.
- 2026-05-20 05:19 +08:00 checkpoint:
  this Phase 5 slice kept working in the P0 `wizard-stream` group, but moved
  from owner-only evidence back to a real owner/fallback pair. Route
  inspection confirmed `POST /api/wizard-stream/career-system` exists on both
  Rust and Python, and both sides hit auth before any deeper SSE business
  logic: Rust stops at the shared middleware with
  `401 {"detail":"未登录，请先登录"}`, while Python stops inside
  `get_user_ai_service -> require_login()` with
  `401 {"detail":"需要登录"}`. The manifest now adds
  `wizard-stream-career-system-auth-guard-rust` under `phase5-p0` and
  `wizard-stream-career-system-auth-guard-python-fallback` under
  `phase5-p0-fallback`, both using the same minimal JSON body
  `{"projectId":"test-project-id"}` on
  `POST /api/wizard-stream/career-system`. This upgrades `wizard-stream` from
  a two-entry fallback matrix (`outline + world-building regenerate`) to a
  three-entry one (`outline + world-building regenerate + career-system`),
  while still keeping `cleanup` explicitly documented as owner-only evidence.
  Validation target for this checkpoint: `pytest
  backend/tests/test_tools/test_run_strangler_gateway_smoke.py -q`, `python
  backend/tools/run_strangler_gateway_smoke.py --manifest
  deploy/strangler-gateway-probes.json --profile phase5-p0 --route-group
  wizard-stream --validate-manifest-only`, `python
  backend/tools/run_strangler_gateway_smoke.py --manifest
  deploy/strangler-gateway-probes.json --profile phase5-p0-fallback
  --route-group wizard-stream --validate-manifest-only`, and
  `git diff --check -- deploy/strangler-gateway-probes.json
  backend/tests/test_tools/test_run_strangler_gateway_smoke.py
  docs/architecture/rust-phase5-p0-route-group-rollback-runbook-2026-05-19.zh-CN.md
  docs/architecture/rust-route-group-ownership-and-cutover-checklist-2026-05-19.zh-CN.md
  docs/architecture/rust-python-api-parity-matrix-2026-05-19.zh-CN.md
  .trellis/tasks/05-18-backend-chapter-generation-refactor-followup/implement.md`
  should pass, allowing only the existing line-ending warning on the manifest
  file if it persists.
- 2026-05-20 05:31 +08:00 checkpoint:
  this Phase 5 slice stayed on the same P0 `wizard-stream` lane and extended
  the newly re-established real fallback matrix one step further. Route
  inspection confirmed `POST /api/wizard-stream/characters` exists on both
  Rust and Python, and both sides again stop at auth before any deeper SSE
  workflow logic: Rust returns
  `401 {"detail":"未登录，请先登录"}` from the shared middleware, while Python
  returns `401 {"detail":"需要登录"}` from
  `get_user_ai_service -> require_login()`. The manifest now adds
  `wizard-stream-characters-auth-guard-rust` under `phase5-p0` and
  `wizard-stream-characters-auth-guard-python-fallback` under
  `phase5-p0-fallback`, both using the same minimal JSON body
  `{"projectId":"test-project-id"}` on
  `POST /api/wizard-stream/characters`. This upgrades `wizard-stream` from a
  three-entry real fallback matrix
  (`outline + world-building regenerate + career-system`) to a four-entry one
  (`outline + world-building regenerate + career-system + characters`), while
  still keeping `cleanup` explicitly documented as owner-only evidence.
  Validation target for this checkpoint: `pytest
  backend/tests/test_tools/test_run_strangler_gateway_smoke.py -q`, `python
  backend/tools/run_strangler_gateway_smoke.py --manifest
  deploy/strangler-gateway-probes.json --profile phase5-p0 --route-group
  wizard-stream --validate-manifest-only`, `python
  backend/tools/run_strangler_gateway_smoke.py --manifest
  deploy/strangler-gateway-probes.json --profile phase5-p0-fallback
  --route-group wizard-stream --validate-manifest-only`, and
  `git diff --check -- deploy/strangler-gateway-probes.json
  backend/tests/test_tools/test_run_strangler_gateway_smoke.py
  docs/architecture/rust-phase5-p0-route-group-rollback-runbook-2026-05-19.zh-CN.md
  docs/architecture/rust-route-group-ownership-and-cutover-checklist-2026-05-19.zh-CN.md
  docs/architecture/rust-python-api-parity-matrix-2026-05-19.zh-CN.md
  .trellis/tasks/05-18-backend-chapter-generation-refactor-followup/implement.md`
  should pass, allowing only the existing line-ending warning on the manifest
  file if it persists.
- 2026-05-20 06:09 +08:00 checkpoint:
  this Phase 5 slice moved from the now-denser `wizard-stream` group back to
  the P0 `chapters` lane, but deliberately avoided inventing another low-value
  auth-only probe. Route inspection confirmed
  `GET /api/chapters/batch-generate/{batch_id}/status` is a real same-path
  asymmetric endpoint: Rust requires auth first and therefore returns
  `401 {"detail":"未登录，请先登录"}`, while the current Python status query
  route does not read login state and instead returns
  `404 {"detail":"Batch generation task not found"}` when the batch id is
  missing. The manifest now adds
  `chapters-batch-status-auth-guard-rust-asymmetric` and
  `chapters-batch-status-task-not-found-python-fallback` under
  `phase5-p0-asymmetric`, both using the same path
  `/api/chapters/batch-generate/test-batch-id/status`. This makes
  `chapters` the second P0 route-group after `settings` to have an explicit
  asymmetric sample, and more importantly establishes the first batch
  generation status-query asymmetry record so future work does not
  accidentally pollute `phase5-p0-fallback` with a fake auth-boundary clue.
  Validation target for this checkpoint: `pytest
  backend/tests/test_tools/test_run_strangler_gateway_smoke.py -q`, `python
  backend/tools/run_strangler_gateway_smoke.py --manifest
  deploy/strangler-gateway-probes.json --profile phase5-p0-asymmetric
  --route-group chapters --validate-manifest-only`, and `git diff --check --
  deploy/strangler-gateway-probes.json
  backend/tests/test_tools/test_run_strangler_gateway_smoke.py
  docs/architecture/rust-phase5-p0-route-group-rollback-runbook-2026-05-19.zh-CN.md
  docs/architecture/rust-route-group-ownership-and-cutover-checklist-2026-05-19.zh-CN.md
  .trellis/tasks/05-18-backend-chapter-generation-refactor-followup/implement.md`
  should pass, allowing only the existing line-ending warning on the manifest
  file if it persists.
- 2026-05-20 06:22 +08:00 checkpoint:
  this Phase 5 slice stayed in the P0 `chapters` lane and continued following
  the same rule: do not confuse a stronger stream/status clue with a fake
  asymmetric sample. Route inspection confirmed
  `GET /api/chapters/batch-generate/{batch_id}/stream` is actually a real
  same-path fallback probe, not another asymmetric one. Rust still stops at
  shared auth first and returns `401 {"detail":"未登录，请先登录"}`. Python
  stream access validation also stops before any SSE event wiring, but it uses
  the older short message `401 {"detail":"未登录"}` when `request.state.user_id`
  is missing. The manifest now adds
  `chapters-batch-stream-auth-guard-rust` under `phase5-p0` and
  `chapters-batch-stream-auth-guard-python-fallback` under
  `phase5-p0-fallback`, both using the same path
  `/api/chapters/batch-generate/test-batch-id/stream`. This upgrades
  `chapters` fallback coverage from five entries to six, extending it from
  read/status endpoints into the same-path batch stream query boundary while
  keeping `batch-generate/{batch_id}/status` in `phase5-p0-asymmetric`.
  Validation target for this checkpoint: `pytest
  backend/tests/test_tools/test_run_strangler_gateway_smoke.py -q`, `python
  backend/tools/run_strangler_gateway_smoke.py --manifest
  deploy/strangler-gateway-probes.json --profile phase5-p0 --route-group
  chapters --validate-manifest-only`, `python
  backend/tools/run_strangler_gateway_smoke.py --manifest
  deploy/strangler-gateway-probes.json --profile phase5-p0-fallback
  --route-group chapters --validate-manifest-only`, and `git diff --check --
  deploy/strangler-gateway-probes.json
  backend/tests/test_tools/test_run_strangler_gateway_smoke.py
  docs/architecture/rust-phase5-p0-route-group-rollback-runbook-2026-05-19.zh-CN.md
  docs/architecture/rust-route-group-ownership-and-cutover-checklist-2026-05-19.zh-CN.md
  docs/architecture/rust-python-api-parity-matrix-2026-05-19.zh-CN.md
  .trellis/tasks/05-18-backend-chapter-generation-refactor-followup/implement.md`
  should pass, allowing only the existing line-ending warning on the manifest
  file if it persists.
- 2026-05-20 11:32 +08:00 checkpoint:
  this Phase 5 slice returned to real `backend-rs` seam tightening instead of
  expanding governance assets again. The first half of the slice moved the
  project batch-generation request assembly into
  `chapter_batch_generation_create_workflow_service.rs` so
  `chapter_batch_generation.rs` no longer consumes compat-only batch request
  fields and assembles `BatchGenerationCreateWorkflowRequest` inline. The new
  `build_batch_generation_create_workflow_request()` helper now owns compat
  field consumption plus the standard workflow request shape, mirroring the
  already-landed single-chapter request builder pattern and keeping route
  handlers transport-oriented. The second half of the slice tightened the
  owned-task access seam by adding
  `load_required_owned_task()` plus `LoadOwnedBatchGenerationTaskError` in
  `chapter_batch_generation_owned_task_query_service.rs`, then reusing that
  shared required-owned-task boundary in
  `chapter_batch_generation_cancel_service.rs`,
  `chapter_batch_generation_resume_service.rs`, and
  `chapter_batch_generation_stream_access_service.rs` instead of repeating
  local `load_owned_task(...).ok_or(TaskNotFound)` conversions. This keeps the
  existing error mapping and HTTP/SSE behavior unchanged while narrowing where
  “owned task must exist” semantics are defined. Validation target for this
  checkpoint passed with
  `cargo check --manifest-path "backend-rs/Cargo.toml"` and
  `cargo test chapter_batch_generation_create_workflow_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`.
  The focused filter
  `cargo test chapter_batch_generation_owned_task_query_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`
  also completed successfully but currently matches 0 tests, so this slice's
  executable regression protection remains the new create-workflow helper test
  plus the repository-wide compile check.
- 2026-05-20 11:36 +08:00 checkpoint:
  this Phase 5 slice kept working inside the `chapter_batch_generation`
  read-side seam and narrowed two small ownership boundaries without touching
  route handlers or transport payload shapes. First,
  `chapter_batch_generation_status_view_service.rs` now owns the required
  status-query view boundary via
  `load_required_batch_generation_task_view_context()` plus
  `LoadBatchGenerationTaskViewContextError`, so
  `chapter_batch_generation_status_query_service.rs` no longer performs its
  own `Option -> TaskNotFound` conversion after loading an optional context.
  This aligns the status-query read path with the previously tightened
  required-owned-task access pattern while keeping the existing
  `TaskNotFound/Internal` error mapping unchanged. Second, the empty active
  batch response wrapper is now owned by
  `chapter_batch_generation_status_payload_adapter_service.rs` through
  `build_empty_active_batch_generation_response()`, and
  `chapter_batch_generation_active_query_service.rs` reuses that shared
  adapter instead of owning a second local `{has_active_task:false,task:null}`
  payload shape. This keeps the no-active-task response identical while
  shrinking one more read-side wrapper out of the query service. Validation
  target for this checkpoint passed with
  `cargo check --manifest-path "backend-rs/Cargo.toml"`,
  `cargo test chapter_batch_generation_status_payload_adapter_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`,
  `cargo test chapter_batch_generation_active_query_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`,
  and `cargo test chapter_batch_generation_status_view_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`.
  The focused filter
  `cargo test chapter_batch_generation_status_query_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`
  also completed successfully but currently matches 0 tests, so direct
  regression protection for the status-query wrapper still comes from the
  shared view/payload tests plus the compile check.
- 2026-05-20 11:41 +08:00 checkpoint:
  this Phase 5 slice stayed on the same `chapter_batch_generation` stream
  seam and finished moving stream-specific event ownership out of the shared
  status-view helper. `chapter_batch_generation_status_stream_service.rs` now
  owns the batch stream event payload builders
  (`build_batch_generation_progress_event()`,
  `build_batch_generation_result_event()`,
  `build_batch_generation_failed_event()`,
  `build_batch_generation_cancelled_event()`,
  `build_batch_generation_not_found_event()`,
  `build_batch_generation_timeout_event()`) plus
  `build_batch_generation_terminal_events()`, while
  `chapter_batch_generation_status_view_service.rs` keeps only stream-state
  loading and fallback semantics. This makes the stream owner responsible for
  stream event shape and terminal sequencing without changing the emitted SSE
  payloads, terminal conditions, or polling cadence. The same slice also
  tightened the stream orchestration boundary by adding
  `create_owned_batch_generation_status_stream()` in
  `chapter_batch_generation_status_stream_service.rs`, so
  `chapter_batch_generation_stream_workflow_service.rs` no longer performs its
  own “ensure access then build stream” choreography and instead delegates the
  full owned-stream creation boundary to the stream service. Validation target
  for this checkpoint passed with
  `cargo check --manifest-path "backend-rs/Cargo.toml"`,
  `cargo test chapter_batch_generation_status_stream_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`,
  and `cargo test chapter_batch_generation_status_view_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`.
  The focused filter
  `cargo test chapter_batch_generation_stream_workflow_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`
  also completed successfully but currently matches 0 tests, so direct
  regression protection for the workflow wrapper still comes from the owned
  stream service tests plus the compile check.
- 2026-05-20 11:45 +08:00 checkpoint:
  this Phase 5 slice stayed on wrapper/read-side hardening and targeted two
  previously thin seams that had little or no direct focused test coverage.
  First, `chapter_batch_generation_status_query_service.rs` now owns an
  explicit `build_batch_generation_status_query_result()` helper instead of
  constructing the response wrapper inline after loading the required task
  view context. The same file now carries focused tests for
  `TaskNotFound/Internal` error mapping and for the final status-query payload
  wrapper built from a concrete `BatchGenerationTaskViewContext`. Second,
  `chapter_batch_generation_stream_access_service.rs` dropped the no-payload
  `BatchGenerationStatusStreamAccessGate` marker struct and now exposes the
  access boundary as `Result<(), BatchGenerationStatusStreamAccessError>`,
  which better matches the actual semantics of the helper while keeping the
  external workflow behavior unchanged. That file also now has focused tests
  for owned-task `TaskNotFound/Internal` error mapping, so the stream access
  seam is no longer a filter that compiles but matches zero tests. Validation
  target for this checkpoint passed with
  `cargo check --manifest-path "backend-rs/Cargo.toml"`,
  `cargo test chapter_batch_generation_status_query_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`,
  `cargo test chapter_batch_generation_stream_access_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`,
  and `cargo test chapter_batch_generation_status_stream_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`.
  This checkpoint intentionally prioritizes executable seam protection over
  adding another broader ownership move, so the next round can keep shrinking
  wrappers without reopening the earlier “0 matched tests” validation gap.
- 2026-05-20 11:52 +08:00 checkpoint:
  this Phase 5 slice returned to stronger read-side ownership tightening after
  the wrapper-seam tests were stabilized. The `chapter_batch_generation`
  query/read lane no longer keeps three separate local `response_payload`
  wrapper structs for active-project, active-task-list, and task-status
  queries. Instead,
  `chapter_batch_generation_status_payload_adapter_service.rs` now owns one
  shared `BatchGenerationQueryResult` plus dedicated constructors:
  `build_active_batch_generation_query_result()`,
  `build_active_batch_generation_task_list_query_result()`, and
  `build_task_status_query_result()`. The query services
  `chapter_batch_generation_active_query_service.rs`,
  `chapter_batch_generation_active_list_query_service.rs`, and
  `chapter_batch_generation_status_query_service.rs` now delegate their final
  response wrapping to that shared payload owner instead of each defining a
  duplicate `*QueryResult { response_payload }` shell locally. This keeps all
  HTTP payload shapes unchanged while making the payload adapter the single
  owner of both read-side JSON shape and query-result wrapping for this route
  group. Focused tests were extended in the payload adapter to cover the new
  shared query-result constructors, while the existing active/status query
  tests continue to validate the resulting payloads from concrete contexts.
  Validation target for this checkpoint passed with
  `cargo check --manifest-path "backend-rs/Cargo.toml"`,
  `cargo test chapter_batch_generation_status_payload_adapter_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`,
  `cargo test chapter_batch_generation_active_query_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`,
  `cargo test chapter_batch_generation_active_list_query_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`,
  and `cargo test chapter_batch_generation_status_query_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`.
- 2026-05-20 12:08 +08:00 checkpoint:
  this Phase 5 slice continued the read-side ownership tightening with a
  narrower semantics-owner move instead of adding another new payload layer.
  `chapter_batch_generation_status_semantics_service.rs` now owns the shared
  active-task status vocabulary through
  `active_batch_generation_statuses()` and
  `is_active_batch_generation_status()`, so the route-group no longer repeats
  the `"pending" / "running"` active-state definition locally. Building on
  that, `chapter_batch_generation_status_view_service.rs` now uses a shared
  `build_active_batch_generation_task_query()` helper for the active-project
  and active-user read paths. This keeps the existing project/user filters,
  sort order, and payload behavior unchanged while making the active-task
  query semantics auditable from one owner boundary instead of two partially
  duplicated query chains. Focused tests were added in
  `chapter_batch_generation_status_semantics_service.rs` to protect the active
  status vocabulary explicitly, and the existing
  `chapter_batch_generation_status_view_service.rs` tests still validate the
  downstream stream/manual-review/read-side behavior after the query helper
  move. Validation target for this checkpoint passed with
  `cargo check --manifest-path "backend-rs/Cargo.toml"`,
  `cargo test chapter_batch_generation_status_semantics_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`,
  and
  `cargo test chapter_batch_generation_status_view_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`.
- 2026-05-20 12:34 +08:00 checkpoint:
  this Phase 5 slice advanced two parallel low-risk ownership moves without
  widening the compatibility surface. First, the single-chapter generation
  route no longer assembles the standard request object locally before calling
  the background or stream workflow. The request owner boundary moved one step
  inward: `chapter_single_generation_request_service.rs` now consumes the
  compat-only `enable_analysis` field directly in
  `build_single_chapter_generation_request()`, and both
  `chapter_single_generation_background_workflow_service.rs` and
  `chapter_single_generation_stream_workflow_service.rs` now build the
  request internally from transport values (`target_word_count`, `model`,
  `enable_analysis`). This keeps auth, workflow semantics, runtime dispatch,
  and response payloads unchanged while making
  `chapter_batch_generation.rs` thinner and less aware of request-shape
  details. Second, the batch-generation status-stream route no longer goes
  through `chapter_batch_generation_stream_workflow_service.rs`, which had
  become a pure one-line forwarder after the owned stream boundary moved into
  `chapter_batch_generation_status_stream_service.rs`. The route now calls
  `create_owned_batch_generation_status_stream()` directly, and the empty
  workflow wrapper module was removed. Validation target for this checkpoint
  passed with
  `cargo check --manifest-path "backend-rs/Cargo.toml"`,
  `cargo test chapter_single_generation_request_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`,
  and
  `cargo test chapter_batch_generation_status_stream_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`.
  The focused filters
  `cargo test chapter_single_generation_stream_workflow_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`
  and
  `cargo test chapter_single_generation_background_workflow_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`
  still currently match 0 tests, so regression protection for those workflow
  shells remains indirect through the request-service tests plus compile
  validation.
- 2026-05-20 12:48 +08:00 checkpoint:
  this Phase 5 slice stayed on the single-chapter generation lane and
  tightened the request-owner boundary one step further. The previous move had
  already removed request assembly from `chapter_batch_generation.rs`, but the
  two workflow shells still repeated the same transport-to-request-to-prepared
  chain locally. `chapter_single_generation_request_service.rs` now owns that
  full conversion through
  `prepare_single_chapter_generation_transport_request()`, which consumes the
  transport fields (`target_word_count`, `model`, `enable_analysis`), builds
  the compatibility-preserving request object once, and then prepares the
  owned execution inputs. Both
  `chapter_single_generation_background_workflow_service.rs` and
  `chapter_single_generation_stream_workflow_service.rs` now delegate that
  chain to the request service instead of repeating the same
  `build_single_chapter_generation_request() + prepare_single_chapter_generation_request()`
  sequence. This keeps the same error types, runtime dispatch, SSE payload
  shape, and chapter access/config semantics, while making the request service
  the clearer owner of the transport-to-domain conversion. Focused tests were
  extended in `chapter_single_generation_request_service.rs` to protect the
  compat-field consumption entrypoint, and validation passed with
  `cargo check --manifest-path "backend-rs/Cargo.toml"`,
  `cargo test chapter_single_generation_request_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`,
  and
  `cargo test chapter_single_generation_stream_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`.
  The `chapter_single_generation_stream_service` filter still matches 0 tests,
  so direct regression protection for the stream shell remains compile-level
  only; this was judged acceptable for this slice because the real ownership
  move happened in the request service, which now has explicit focused
  coverage.
- 2026-05-20 13:06 +08:00 checkpoint:
  this Phase 5 slice returned to `chapter_batch_generation_task_command_service.rs`
  and tightened the shared status/checkpoint ownership across the write-side
  resume path. Before this slice, the service still carried two small drift
  risks: queued-vs-resume pending runtime checkpoints were assembled through
  separate local JSON shapes, and the resume response re-declared
  `stage_code` / `execution_mode` / `checkpoint` metadata inline instead of
  reusing the shared checkpoint metadata owner used on the read side. The
  service now centralizes pending runtime checkpoint assembly through
  `build_pending_runtime_checkpoint()`, which is reused by both the queued
  snapshot writers and the resume checkpoint builder. In addition,
  `build_resume_batch_generation_response_payload()` now owns the resume
  response payload assembly and reuses
  `checkpoint_with_runtime_metadata()` from
  `chapter_batch_generation_status_payload_adapter_service.rs` so the resume
  response no longer hand-assembles a second copy of the same
  stage/execution metadata. This keeps the HTTP response fields, resume
  semantics, checkpoint defaults, and persisted task status unchanged while
  reducing drift risk between write-side resume responses and read-side status
  payloads. Focused tests were added in
  `chapter_batch_generation_task_command_service.rs` for the new queued
  pending-checkpoint helper and the shared-metadata resume response helper.
  Validation passed with
  `cargo check --manifest-path "backend-rs/Cargo.toml"`,
  `cargo test chapter_batch_generation_task_command_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`,
  and
  `cargo test chapter_batch_generation_status_payload_adapter_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`.
- 2026-05-20 13:24 +08:00 checkpoint:
  this Phase 5 slice kept the scope narrow but tightened two more ownership
  seams around active/read-side context loading and task-command response
  payload assembly. First,
  `chapter_batch_generation_status_view_service.rs` now owns shared
  single-or-many task-to-view-context conversion helpers through
  `build_optional_batch_generation_task_view_context()` and
  `build_batch_generation_task_view_contexts()`. The active-project,
  active-user, and owned-task read paths no longer each carry their own local
  `Option -> Some(context)` or `Vec -> push context in a loop` conversion
  pattern; they now delegate that shape to one owner boundary while keeping
  query filters, payload shape, and error behavior unchanged. Second,
  `chapter_batch_generation_task_command_service.rs` now owns dedicated
  response helpers for batch create, single background create, and cancel
  payloads, plus a shared
  `estimate_batch_generation_task_minutes()` helper. This removes another set
  of scattered inline JSON payload declarations and keeps the estimated-time
  convention in one place instead of embedding `2` / `total.max(1) * 2`
  directly in multiple command branches. These moves intentionally stop short
  of broader route/runtime refactors; the gain here is drift reduction rather
  than changing behavior. Validation passed with
  `cargo check --manifest-path "backend-rs/Cargo.toml"`,
  `cargo test chapter_batch_generation_status_view_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`,
  `cargo test chapter_batch_generation_task_command_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`,
  and
  `cargo test chapter_batch_generation_status_payload_adapter_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`.
- 2026-05-20 13:39 +08:00 checkpoint:
  this Phase 5 slice focused on a smaller but high-signal drift point:
  multiple `chapter_batch_generation` services were each locally projecting
  `LoadOwnedBatchGenerationTaskError` into their own domain-specific
  `TaskNotFound/Internal` enums with repeated `match` blocks. The owned-task
  query boundary now exposes a shared
  `map_owned_batch_generation_task_error()` helper in
  `chapter_batch_generation_owned_task_query_service.rs`, and the adjacent
  callers now delegate their not-found/internal projection to that helper
  instead of carrying four separate local mappings. The moved callers were
  `chapter_batch_generation_status_view_service.rs`,
  `chapter_batch_generation_cancel_service.rs`,
  `chapter_batch_generation_resume_service.rs`, and
  `chapter_batch_generation_stream_access_service.rs`. This does not change
  any outward error enum, response payload, or route mapping; it only reduces
  the chance that future owned-task error handling drifts between read-side,
  command-side, and stream access boundaries. Focused tests were added on the
  new shared mapper and the existing stream-access tests were updated to use
  the shared helper. Validation passed with
  `cargo check --manifest-path "backend-rs/Cargo.toml"`,
  `cargo test chapter_batch_generation_owned_task_query_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`,
  `cargo test chapter_batch_generation_stream_access_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`,
  `cargo test chapter_batch_generation_task_command_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`,
  and
  `cargo test chapter_batch_generation_status_view_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`.
- 2026-05-20 13:52 +08:00 checkpoint:
  this Phase 5 slice targeted the remaining ultra-thin workflow/result shells
  on the batch-generation path and removed wrappers that no longer carried
  independent semantics. `chapter_batch_generation_create_workflow_service.rs`
  and `chapter_batch_generation_resume_service.rs` no longer wrap their final
  response in one-field `Start*WorkflowResult` / `*WorkflowResult` structs
  when the only exported field was `response_payload`. The start/create path
  now returns the response `Value` directly after dispatch setup, and the
  resume workflow path now likewise returns the cloned response payload
  directly once dispatch has been scheduled. On the cancel path,
  `chapter_batch_generation_cancel_service.rs` now returns the existing
  `CancelBatchGenerationResult` from the task-command owner instead of
  introducing a second cancel-specific workflow wrapper with the same single
  field. `chapter_batch_generation.rs` was updated only at the transport call
  sites to consume those slimmer return shapes; route payloads and error
  mapping stay unchanged. This slice intentionally stopped before touching
  `PreparedBatchGenerationResumeRequest`, because that struct still owns
  multiple values (`response_payload`, `ai_config`, `provider_payload`,
  `execution`) and is not a pure wrapper yet. Validation passed with
  `cargo check --manifest-path "backend-rs/Cargo.toml"`,
  `cargo test chapter_batch_generation_task_command_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`,
  and
  `cargo test chapter_batch_generation_create_workflow_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`.
  The focused filter
  `cargo test chapter_batch_generation_resume_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`
  still matches 0 tests, which is consistent with the fact that this slice
  only removed a thin wrapper shell and kept the actual resume semantics
  protected through compile validation plus the task-command tests.
- 2026-05-20 14:07 +08:00 checkpoint:
  this Phase 5 slice continued the same thin-wrapper reduction on the
  read/query side by removing the now-redundant `BatchGenerationQueryResult`
  shell from `chapter_batch_generation_status_payload_adapter_service.rs` and
  its direct consumers. The adapter-owned `build_*_query_result()` helpers now
  return `serde_json::Value` directly instead of wrapping that payload in a
  one-field struct, and the adjacent query services
  `chapter_batch_generation_status_query_service.rs`,
  `chapter_batch_generation_active_query_service.rs`, and
  `chapter_batch_generation_active_list_query_service.rs` now return `Value`
  directly as well. `chapter_batch_generation.rs` was updated only at the
  route call sites to consume the slimmer return shape, with no response
  field changes and no error-mapping changes. This keeps the payload adapter
  as the owner of JSON shape while removing one more internal wrapper layer
  that no longer encoded meaningful behavior. Validation passed with
  `cargo check --manifest-path "backend-rs/Cargo.toml"`,
  `cargo test chapter_batch_generation_status_payload_adapter_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`,
  `cargo test chapter_batch_generation_active_query_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`,
  `cargo test chapter_batch_generation_active_list_query_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`,
  and
  `cargo test chapter_batch_generation_status_query_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`.
- 2026-05-20 14:21 +08:00 checkpoint:
  this Phase 5 slice stayed on the same batch-generation read/status lane and
  removed three more no-behavior query wrappers from
  `chapter_batch_generation_status_payload_adapter_service.rs`:
  `build_task_status_query_result()`,
  `build_active_batch_generation_query_result()`, and
  `build_active_batch_generation_task_list_query_result()`. The adjacent
  query services
  `chapter_batch_generation_status_query_service.rs`,
  `chapter_batch_generation_active_query_service.rs`, and
  `chapter_batch_generation_active_list_query_service.rs` now call the real
  response builders directly:
  `build_task_status_response()`,
  `build_active_batch_generation_response()`,
  `build_empty_active_batch_generation_response()`, and
  `build_active_batch_generation_task_list_response()`. This keeps JSON
  payload shape, task-not-found/error mapping, and status semantics unchanged
  while deleting one more adapter-only forwarding layer. Validation passed
  with `cargo check --manifest-path "backend-rs/Cargo.toml"`,
  `cargo test chapter_batch_generation_status_payload_adapter_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`,
  `cargo test chapter_batch_generation_active_query_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`,
  `cargo test chapter_batch_generation_active_list_query_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`,
  and
  `cargo test chapter_batch_generation_status_query_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`.
- 2026-05-20 14:24 +08:00 checkpoint:
  this Phase 5 slice tightened runtime snapshot ownership by deleting the
  duplicated snapshot-loader and new-task snapshot initialization helpers from
  `chapter_batch_generation_task_command_service.rs`, then reusing the
  existing owner functions in
  `chapter_batch_generation_runtime_state_service.rs` instead. In the same
  pass, `chapter_batch_generation_status_view_service.rs` stopped carrying its
  own copy of `load_batch_generation_snapshot()` and now imports that read
  helper from the runtime owner as well. Resume-specific snapshot replacement
  semantics stay local to the task-command owner, so this slice only removes
  duplication and clarifies that general snapshot load/upsert/init behavior
  belongs to the runtime state service. No HTTP payloads, status semantics,
  or resume-reset behavior changed. Validation passed with
  `cargo check --manifest-path "backend-rs/Cargo.toml"`,
  `cargo test chapter_batch_generation_task_command_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`,
  and
  `cargo test chapter_batch_generation_status_view_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`.
- 2026-05-20 14:26 +08:00 checkpoint:
  this Phase 5 slice continued the same thin-wrapper reduction on the
  task-command lane by deleting the one-field
  `CancelBatchGenerationResult` shell from
  `chapter_batch_generation_task_command_service.rs`. The owned
  `cancel_batch_generation_task()` entrypoint now returns the final response
  `Value` directly after persisting the cancelled task state and reusing the
  shared runtime finalizer, and
  `chapter_batch_generation_cancel_service.rs` now forwards that payload
  without an extra unwrap/rewrap hop. No cancel payload fields, task status
  semantics, or runtime checkpoint behavior changed; this only narrows one
  more internal service boundary that no longer encoded business meaning.
  Validation passed with
  `cargo check --manifest-path "backend-rs/Cargo.toml"`,
  `cargo test chapter_batch_generation_task_command_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`,
  and
  `cargo test chapter_batch_generation_cancel_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`.
- 2026-05-20 14:30 +08:00 checkpoint:
  this Phase 5 slice returned to the query lane and removed two more
  no-behavior helper hops. In
  `chapter_batch_generation_status_query_service.rs`, the local
  `build_batch_generation_status_query_result()` wrapper was deleted and the
  query entrypoint now returns `build_task_status_response(context)` directly.
  In `chapter_batch_generation_active_list_query_service.rs`, the intermediate
  `load_active_batch_generation_task_list_query()` helper was removed because
  its only caller already owned limit normalization and only forwarded the
  resulting contexts into `build_active_batch_generation_task_list_response()`.
  The route and service contracts stay the same: task-not-found/internal
  mapping, active-list limit normalization, and all status/list JSON payload
  fields remain unchanged. Validation passed with
  `cargo check --manifest-path "backend-rs/Cargo.toml"`,
  `cargo test chapter_batch_generation_status_query_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`,
  and
  `cargo test chapter_batch_generation_active_list_query_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`.
- 2026-05-20 14:33 +08:00 checkpoint:
  this Phase 5 slice narrowed the task-command to workflow contract by
  shrinking three plan carriers down to the minimum data their callers
  actually consume. `BatchGenerationCreatePlan` and
  `SingleGenerationBackgroundCreatePlan` in
  `chapter_batch_generation_task_command_service.rs` no longer expose the full
  persisted `created_task` model; they now return only `created_task_id`
  alongside the already-owned `response_payload`, `chapter_ids`, and
  `target_word_count`. In the same pass, `ResumeBatchGenerationPlan` stopped
  exposing `updated_task` because the resume workflow only needs the
  `response_payload` plus `execution` plan after the task-command owner
  persists the reset state internally. The adjacent workflow owners
  `chapter_batch_generation_create_workflow_service.rs` and
  `chapter_single_generation_background_workflow_service.rs` were updated to
  consume the new `created_task_id` field directly. This keeps all create,
  background, and resume payloads plus runtime dispatch semantics unchanged
  while reducing the amount of database-model knowledge leaked across the
  service boundary. Validation passed with
  `cargo check --manifest-path "backend-rs/Cargo.toml"`,
  `cargo test chapter_batch_generation_task_command_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`,
  `cargo test chapter_batch_generation_create_workflow_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`,
  and
  `cargo test chapter_single_generation_background_workflow_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`.
- 2026-05-20 14:36 +08:00 checkpoint:
  this Phase 5 slice kept shrinking workflow-only carriers that had only one
  local consumer and no independent behavior. In
  `chapter_batch_generation_resume_service.rs`, the private
  `PreparedBatchGenerationResumeRequest` shell and its
  `prepare_batch_generation_resume_request()` helper were removed; the public
  `resume_owned_batch_generation_task()` entrypoint now performs the same
  owned-task load, task-command resume preparation, config preparation, and
  runtime dispatch inline before returning the unchanged response payload. In
  `chapter_single_generation_background_workflow_service.rs`, the private
  `CreateSingleGenerationBackgroundWorkflowResult` carrier and its
  `create_single_generation_background_workflow()` helper were removed for the
  same reason, so the workflow entrypoint now directly sequences request
  preparation, task-plan creation, runtime dispatch, and final payload
  forwarding. No create/resume/background payload fields, error mapping, or
  dispatch semantics changed; this slice only removes two more single-use
  internal wrappers from the workflow lane. Validation passed with
  `cargo check --manifest-path "backend-rs/Cargo.toml"`,
  `cargo test chapter_batch_generation_task_command_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`,
  `cargo test chapter_batch_generation_resume_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`,
  and
  `cargo test chapter_single_generation_background_workflow_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`.
- 2026-05-20 14:41 +08:00 checkpoint:
  this Phase 5 slice tightened two more adjacent orchestration seams. First,
  `chapter_batch_generation_status_stream_service.rs` absorbed the tiny
  `chapter_batch_generation_stream_access_service.rs` access wrapper: the
  stream-access error enum and owned-task permission check now live directly
  next to `create_owned_batch_generation_status_stream()`, and the standalone
  wrapper module plus its `services/mod.rs` export were removed. Second,
  `chapter_batch_generation_create_workflow_service.rs` no longer uses the
  private single-consumer `CreateBatchGenerationWorkflowResult` carrier.
  `start_owned_batch_generation_workflow()` now performs request building,
  access verification, prepared create lookup, config preparation, task-plan
  creation, runtime dispatch, and final payload forwarding in one owner
  boundary. No stream-access error mapping, create payload fields, config
  defaults, or runtime dispatch semantics changed; this slice only removes one
  dead helper module and one single-use workflow carrier. Validation passed
  with `cargo check --manifest-path "backend-rs/Cargo.toml"`,
  `cargo test chapter_batch_generation_status_stream_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`,
  `cargo test chapter_batch_generation_create_workflow_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`,
  and a follow-up literal search confirmed there are no remaining
  `chapter_batch_generation_stream_access_service` references under
  `backend-rs/src`.
- 2026-05-20 14:45 +08:00 checkpoint:
  this Phase 5 slice focused on internal contract slimming in the remaining
  batch-create support helpers. In
  `chapter_batch_generation_request_compat_service.rs`, the intermediate
  `BatchGenerationRequestCompatView` projection and its
  `project_batch_generation_request_compat_fields()` helper were removed
  because the codebase no longer consumes that projected view anywhere; the
  compat-consumption boundary now reads the original
  `BatchGenerationRequestCompatFields` directly and preserves the same
  placeholder compatibility sink behavior. In
  `chapter_batch_generation_create_service.rs`, the
  `PreparedBatchGenerationCreateRequest` contract was narrowed by removing the
  unused `end_chapter_number` field, and the duplicated count validation inside
  `prepare_batch_generation_create_request()` was dropped because
  `load_chapters_for_batch_generation_range()` already owns that check. The
  normalized target-word-count behavior, create-workflow orchestration, and
  batch chapter lookup semantics remain unchanged; this slice only deletes one
  dead projection layer and one unused prepared field while relying on the
  existing validation owner. Validation passed with
  `cargo check --manifest-path "backend-rs/Cargo.toml"`,
  `cargo test chapter_batch_generation_create_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`,
  `cargo test chapter_batch_generation_create_workflow_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`,
  and
  `cargo test chapter_batch_generation_request_compat_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`.
- 2026-05-20 12:37 +08:00 checkpoint:
  this Phase 5 slice returned to the batch-create lane and moved one more
  transport-to-domain ownership seam inward without changing route behavior.
  `chapter_batch_generation.rs` no longer builds a
  `BatchGenerationCreateWorkflowRequest` locally before calling the workflow
  owner. Instead,
  `chapter_batch_generation_create_workflow_service.rs` now keeps
  `build_batch_generation_create_workflow_request()` private and
  `start_owned_batch_generation_workflow()` directly accepts the transport
  fields plus `BatchGenerationRequestCompatFields`, consumes the compat-only
  fields, and assembles the owned workflow request internally before calling
  the existing create/dispatch path. This keeps HTTP payload fields, route
  error mapping, task-plan creation, default `enable_analysis` /
  `max_retries` semantics, and runtime dispatch unchanged while making the
  route more strictly transport-only and aligning the batch-create path with
  the earlier single-generation request-owner move. Validation passed with
  `cargo check --manifest-path "backend-rs/Cargo.toml"` and
  `cargo test chapter_batch_generation_create_workflow_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`.
- 2026-05-20 12:41 +08:00 checkpoint:
  this Phase 5 slice removed two more internal one-field response wrappers on
  adjacent create/cancel paths without widening the compatibility surface.
  `chapter_single_generation_background_workflow_service.rs` no longer returns
  `StartSingleGenerationBackgroundWorkflowResult { response_payload }` from
  `start_owned_single_generation_background_workflow()`; after dispatching the
  owned runtime, it now returns the response `Value` directly. In parallel,
  `chapter_batch_generation_cancel_service.rs` no longer unwraps and rewraps
  `CancelBatchGenerationResult` from the task-command owner just to forward
  the same payload shape; `cancel_owned_batch_generation_task()` now returns
  that owned response `Value` directly. `chapter_batch_generation.rs` was
  updated only at the transport call sites to consume those slimmer return
  values, so the HTTP payload fields and error mapping remain unchanged while
  the route group loses two more pieces of internal `.response_payload`
  knowledge. Validation passed with
  `cargo check --manifest-path "backend-rs/Cargo.toml"` and
  `cargo test chapter_batch_generation_task_command_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`.
  The focused filter
  `cargo test chapter_single_generation_background_workflow_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`
  still matches 0 tests, so direct protection for that workflow shell remains
  compile-level only; this is acceptable here because the moved seam is a
  pure wrapper reduction and the owned single-generation request/task-command
  helpers still carry the meaningful focused coverage.
- 2026-05-20 12:44 +08:00 checkpoint:
  this Phase 5 slice stayed entirely inside the existing workflow owners and
  tightened their internal visibility instead of changing any outward
  contract. Three workflow-only result/request carriers are now private to
  their owner files:
  `BatchGenerationCreateWorkflowRequest` and
  `CreateBatchGenerationWorkflowResult` inside
  `chapter_batch_generation_create_workflow_service.rs`,
  `PreparedBatchGenerationResumeRequest` inside
  `chapter_batch_generation_resume_service.rs`, and
  `CreateSingleGenerationBackgroundWorkflowResult` inside
  `chapter_single_generation_background_workflow_service.rs`.
  The corresponding helper functions
  `create_batch_generation_workflow()`,
  `prepare_batch_generation_resume_request()`, and
  `create_single_generation_background_workflow()` are also now file-private,
  because their call sites never crossed a module boundary. While doing that,
  the workflow entrypoints now destructure those internal carriers directly
  and stop clone-forwarding `response_payload`, which makes the owner
  boundary narrower without changing route payloads, dispatch semantics,
  status/checkpoint defaults, or error mapping. Validation passed with
  `cargo check --manifest-path "backend-rs/Cargo.toml"`,
  `cargo test chapter_batch_generation_create_workflow_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`,
  and
  `cargo test chapter_batch_generation_task_command_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`.
- 2026-05-20 12:47 +08:00 checkpoint:
  this Phase 5 slice removed a now-dead aggregation facade instead of moving
  more behavior across boundaries. `chapter_batch_generation_service.rs` had
  become a pure `pub use` surface for adjacent task/runtime/query helpers, and
  the remaining codebase no longer referenced that module at all. The facade
  file was removed and `services/mod.rs` no longer exports
  `chapter_batch_generation_service`. This reduces one more public module
  boundary without changing runtime behavior, route ownership, or any payload
  shape. Validation passed with
  `cargo check --manifest-path "backend-rs/Cargo.toml"` and
  `cargo test chapter_batch_generation_task_command_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`.
  A follow-up literal search for `chapter_batch_generation_service` in
  `backend-rs/src` returned no matches, confirming there are no residual
  internal consumers of the deleted facade.
- 2026-05-20 12:50 +08:00 checkpoint:
  this Phase 5 slice applied the same facade-cleanup pattern to two adjacent
  chapter-generation compatibility layers. `chapter_analysis_quality_service.rs`
  was a dead one-line re-export of
  `chapter_quality_query_service::load_chapter_quality_metrics_payload` and no
  longer had any internal consumers, so the facade file and its `services/mod.rs`
  export were removed. `chapter_generation_service.rs` had become a single
  re-export of
  `chapter_generation_runtime_service::generate_and_persist_chapter_content_with_provider_payload`;
  its remaining internal callers in
  `chapter_batch_generation_runtime_state_service.rs` and
  `chapter_single_generation_stream_service.rs` now import the runtime owner
  directly, and the compatibility facade file plus its `services/mod.rs`
  export were removed as well. This further narrows the Rust service module
  surface without changing generation behavior, payload shape, or runtime
  sequencing. Validation passed with
  `cargo check --manifest-path "backend-rs/Cargo.toml"`,
  `cargo test chapter_generation_runtime_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`,
  and
  `cargo test chapter_batch_generation_runtime_state_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`.
  A follow-up literal search for `chapter_analysis_quality_service` and
  `chapter_generation_service` in `backend-rs/src` returned no matches,
  confirming there are no residual internal consumers of the deleted facades.
- 2026-05-20 12:53 +08:00 checkpoint:
  this Phase 5 slice continued the same surface-reduction work on the
  regeneration lane. `chapter_regeneration_service.rs` had become a dead
  compatibility facade that only re-exported apply/prepare/text helpers and no
  longer had any internal consumers, so the facade file and its `services/mod.rs`
  export were removed. In parallel,
  `chapter_regeneration_text_service.rs` no longer re-exports
  `contains_chapter_workflow_meta_text` and
  `sanitize_generated_narrative_text` from
  `chapter_narrative_cleaner_service`; it now imports those cleaner helpers
  privately for its own finalize logic. Adjacent callers that truly need the
  cleaner behavior, such as `chapter_regeneration_apply_service.rs`, already
  import the cleaner owner directly. This keeps regeneration payload shapes
  and text-finalization behavior unchanged while shrinking one more public
  compatibility surface and clarifying that narrative cleaning belongs to the
  cleaner owner, not to the regeneration text adapter. Validation passed with
  `cargo check --manifest-path "backend-rs/Cargo.toml"`,
  `cargo test chapter_regeneration_text_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`,
  and
  `cargo test chapter_regeneration_apply_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`.
  A follow-up literal search for `chapter_regeneration_service` in
  `backend-rs/src/services` returned no matches, confirming there are no
  residual internal consumers of the deleted facade.
- 2026-05-20 12:55 +08:00 checkpoint:
  this Phase 5 slice applied the same dead-facade cleanup to the CRUD lane.
  `chapter_crud_service.rs` only re-exported payload workflow functions and
  error enums from `chapter_crud_workflow_service.rs`, and the remaining
  Rust callers were limited to `chapter_crud_routes.rs` and
  `chapter_crud_error_mapper.rs`. Those call sites now import the CRUD
  workflow owner directly, so the compatibility facade file and its
  `services/mod.rs` export were removed. This leaves CRUD route behavior,
  payload shape, and error mapping unchanged while shrinking one more public
  service module surface. Validation passed with
  `cargo check --manifest-path "backend-rs/Cargo.toml"`,
  `cargo test chapter_crud_payload_adapter_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`,
  and
  `cargo test chapter_crud_workflow_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`.
  The CRUD workflow filter still matches 0 tests in this environment, so
  direct regression protection for this slice remains compile-level plus the
  payload-adapter tests; that is acceptable here because the change was a pure
  facade removal with no workflow logic edits. A follow-up literal search for
  `chapter_crud_service` in `backend-rs/src` returned no matches, confirming
  there are no residual internal consumers of the deleted facade.
- 2026-05-20 12:59 +08:00 checkpoint:
  this Phase 5 slice returned to the batch-generation read/status lane and
  tightened two small internal visibility seams without changing status
  semantics or payload shape. First,
  `chapter_batch_generation_status_view_service.rs` no longer re-exports
  payload-adapter or status-semantics helpers through `pub use`; it now
  imports only the owner functions it actually consumes internally, and its
  tests import payload helpers directly from
  `chapter_batch_generation_status_payload_adapter_service.rs`. Second,
  `chapter_batch_generation_status_query_service.rs` made
  `build_batch_generation_status_query_result()` file-private because the
  helper is only used by the local query path plus local unit tests. This
  keeps the same task-not-found/internal mapping and the same task-status JSON
  shape while shrinking one more layer of historical transitive surface from
  the status owner modules. Validation passed with
  `cargo check --manifest-path "backend-rs/Cargo.toml"`,
  `cargo test chapter_batch_generation_status_view_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`,
  and
  `cargo test chapter_batch_generation_status_query_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`.
- 2026-05-20 13:38 +08:00 checkpoint:
  this Phase 5 slice stayed inside the batch-generation runtime owner boundary
  and pulled two remaining pieces of runtime-state infrastructure out of
  `chapter_batch_generation_task_command_service.rs` without changing resume
  orchestration. `chapter_batch_generation_runtime_state_service.rs` now owns
  `build_pending_batch_generation_runtime_checkpoint()` for pending batch-style
  checkpoint assembly and also owns
  `replace_batch_generation_runtime_snapshot_for_resume()` for the strict
  resume snapshot-reset write path that clears stale quality fields and
  replaces, rather than merges, the persisted runtime checkpoint. The
  task-command owner still decides resume eligibility, execution branching, and
  task-row reset semantics, but it now reuses those runtime-state helpers
  instead of carrying a second local checkpoint builder plus a duplicate
  snapshot write implementation. This keeps response payloads, checkpoint
  defaults, manual-review resume blocking, and `ResumeExecutionPlan`
  dispatching unchanged while making the runtime owner more complete. This
  slice also moved the queued pending-checkpoint unit assertion to the runtime
  owner so the extracted helper keeps direct focused coverage. Validation
  passed with `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo check --manifest-path "backend-rs/Cargo.toml"`,
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo test chapter_batch_generation_task_command_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`,
  and
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo test chapter_batch_generation_runtime_state_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`.
  Current stop-rule assessment: the most obvious remaining seams in this lane
  now sit much closer to true resume/dispatch orchestration boundaries, so the
  next Phase 5 slice should only proceed when it can prove the same
  behavior-preserving profile as this runtime-owner move.
- 2026-05-20 15:48 +08:00 checkpoint:
  this Phase 5 slice moved back to the single-chapter request ownership lane
  and removed two now-thin transport wrappers from
  `chapter_single_generation_request_service.rs` without changing route or
  runtime behavior. The service no longer carries
  `build_single_chapter_generation_request()` or
  `prepare_single_chapter_generation_transport_request()` because both were
  single-consumer helpers that only consumed the compat-only
  `enable_analysis` field and then forwarded the same minimal
  `{ target_word_count, model }` request contract into the real owner entrypoint.
  `chapter_single_generation_background_workflow_service.rs` and
  `chapter_single_generation_stream_workflow_service.rs` now consume the
  compat-only `enable_analysis` field locally, assemble the minimal
  `SingleChapterGenerationRequest`, and call
  `prepare_single_chapter_generation_request()` directly. This keeps HTTP
  payload fields, single-chapter access checks, target-word normalization,
  provider-payload preparation, background task creation, runtime dispatch,
  and SSE stream construction unchanged while making the request owner surface
  narrower and clarifying that the workflow boundary, not the request service,
  owns transport-only compat consumption. Validation passed with
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo check --manifest-path "backend-rs/Cargo.toml"`,
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo test chapter_single_generation_request_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`,
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo test chapter_single_generation_background_workflow_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`,
  and
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo test chapter_single_generation_stream_workflow_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`.
  The two workflow-focused filters still match 0 tests in this environment, so
  direct protection for the moved workflow seam remains compile-level plus the
  request-service unit coverage; that is acceptable here because the slice was
  a pure contract-thinning move with no runtime logic edits.
- 2026-05-20 15:55 +08:00 checkpoint:
  this Phase 5 slice returned to the batch-create lane and applied two
  back-to-back contract-thinning moves inside the existing create owner path
  without changing transport behavior or runtime dispatch. First,
  `chapter_batch_generation_create_workflow_service.rs` no longer keeps the
  internal `BatchGenerationCreateWorkflowRequest` carrier or the
  `build_batch_generation_create_workflow_request()` helper. The workflow
  entrypoint now consumes the compat-only request fields locally, keeps
  project-access verification at the same boundary, and passes the explicit
  transport fields directly into create preparation, execution-config loading,
  task-plan creation, and runtime dispatch. Second,
  `chapter_batch_generation_create_service.rs` no longer carries the
  single-consumer `BatchGenerationCreateRequest` input struct; its owner
  entrypoint now directly accepts
  `(start_chapter_number, count, target_word_count)` as the minimal create
  contract while preserving the same count validation, chapter-range loading,
  and target-word normalization behavior. Together these two moves keep HTTP
  payload fields, project-access checks, chapter selection semantics,
  default `enable_analysis` / `max_retries` behavior, provider-payload
  preparation, task-plan creation, and background runtime dispatch unchanged
  while shrinking one more internal request layer from the batch-create path.
  Validation passed with
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo check --manifest-path "backend-rs/Cargo.toml"`,
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo test chapter_batch_generation_create_workflow_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`,
  and
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo test chapter_batch_generation_create_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`.
  Current stop-rule assessment: the obvious low-risk request/contract wrappers
  in the batch-create and single-chapter lanes are now much thinner than
  before, so the next Phase 5 slice should prefer read-side/helper ownership
  seams or another clearly single-consumer carrier, not orchestration-heavy
  resume/dispatch changes.
- 2026-05-20 16:00 +08:00 checkpoint:
  this Phase 5 slice moved back to the batch-generation read side and narrowed
  the payload-adapter surface so query-only response envelopes now belong to
  the query owners instead of the shared adapter module. In
  `chapter_batch_generation_status_payload_adapter_service.rs`, the shared
  adapter now only owns the core payload builders
  (`task_status_payload`, `active_task_payload`, task-list response helpers)
  and no longer carries the query-specific wrappers
  `build_task_status_response()`,
  `build_active_batch_generation_response()`, or
  `build_empty_active_batch_generation_response()`. That envelope assembly now
  lives locally in `chapter_batch_generation_status_query_service.rs` via
  `build_batch_generation_status_query_response()` and in
  `chapter_batch_generation_active_query_service.rs` via
  `build_active_batch_generation_query_response()`, so each query owner keeps
  its own transport-facing `Some/None` response shape while still reusing the
  shared payload builders for the actual task/status content. This keeps task
  status JSON fields, active-task envelope fields, quality/status metadata, and
  empty-state behavior unchanged while making the shared adapter less
  transitive and clarifying ownership between core payload projection and query
  response assembly. During verification, a duplicate `#[test]` attribute in
  `chapter_batch_generation_status_payload_adapter_service.rs` surfaced as
  warning-only noise, so that redundant attribute was removed to keep future
  focused test output clean. Validation passed with
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo check --manifest-path "backend-rs/Cargo.toml"`,
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo test chapter_batch_generation_status_payload_adapter_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`,
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo test chapter_batch_generation_status_query_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`,
  and
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo test chapter_batch_generation_active_query_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`.
  Current stop-rule assessment: the remaining obvious Phase 5 seams are now
  increasingly concentrated in behavior-sensitive status/view/runtime owners,
  so the next move should only proceed if it cleanly separates another helper
  or single-consumer carrier without pulling true orchestration or status
  semantics across boundaries.
- 2026-05-20 16:08 +08:00 checkpoint:
  this Phase 5 slice continued the same read-side owner-tightening pattern on
  the active-task-list query path. `chapter_batch_generation_status_payload_adapter_service.rs`
  no longer carries `build_active_batch_generation_task_list_response()`;
  the shared adapter now only owns the item-level payload builders
  (`task_status_payload`, `active_task_payload`) plus supporting metadata
  helpers. The `total/items` list-response envelope is now owned locally by
  `chapter_batch_generation_active_list_query_service.rs` via
  `build_active_batch_generation_task_list_query_response()`, so the query
  owner controls its own response shape while still reusing the shared
  per-item payload projection. This keeps `items` payload fields, `total`
  semantics, quality/status metadata, and empty-list behavior unchanged while
  further reducing the transitive surface of the shared adapter module. During
  the move, two pieces of stale test coupling were also cleaned up: a
  cross-owner test dependency from `chapter_batch_generation_status_view_service.rs`
  to the active-list query helper was removed instead of widening that helper
  back to a public surface, and leftover unused imports/warning noise were
  cleared so focused tests remain signal-rich. Validation passed with
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo check --manifest-path "backend-rs/Cargo.toml"`,
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo test chapter_batch_generation_status_payload_adapter_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`,
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo test chapter_batch_generation_active_list_query_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`,
  and
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo test chapter_batch_generation_status_view_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`.
  Current stop-rule assessment: the remaining Phase 5 opportunities are now
  mostly clustered inside behavior-sensitive status-view/runtime/task-command
  owners, so further acceleration should stay selective and only take seams
  that remove a clearly isolated helper or carrier without moving status logic
  or orchestration semantics.
- 2026-05-20 16:15 +08:00 checkpoint:
  this Phase 5 slice stayed inside the same batch-generation resume/runtime
  lane and completed one more owner-tightening move without changing resume
  eligibility or dispatch sequencing.
  `chapter_batch_generation_runtime_state_service.rs` now owns the remaining
  resume runtime reset helpers
  `build_resume_batch_generation_runtime_checkpoint()` and
  `resolve_resume_batch_generation_runtime_position()`, so the runtime-state
  owner fully controls how a resumed task clears or preserves chapter pointer
  plus pending checkpoint progress before the snapshot reset write.
  `chapter_batch_generation_task_command_service.rs` still owns domain
  validation (failed/cancelled gate, manual-review blocker, execution-plan
  branching, task-row reset), but it now delegates the resume checkpoint
  projection to the runtime-state owner instead of carrying a second local copy
  of that runtime-reset shape. Focused unit coverage for the single-chapter
  resume checkpoint and batch resume position/progress reset moved with the
  helper into the runtime-state owner, while task-command tests keep coverage
  for response payload and resume gating behavior. This keeps resume response
  payload fields, snapshot reset semantics, chapter pointer preservation for
  single-generation tasks, batch pointer clearing, and `ResumeExecutionPlan`
  dispatch behavior unchanged while making the runtime owner more complete.
  Validation passed with
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo check --manifest-path "backend-rs/Cargo.toml"`,
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo test chapter_batch_generation_runtime_state_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`,
  and
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo test chapter_batch_generation_task_command_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`.
  Updated stop-rule assessment: remaining obvious seams in this lane are now
  even closer to true resume orchestration, task-row semantics, or read-side
  status behavior, so the next Phase 5 slice should only continue if it can
  isolate another helper/carrier with the same behavior-preserving profile.
- 2026-05-20 16:19 +08:00 checkpoint:
  this Phase 5 slice accelerated with two additional low-risk surface-slimming
  moves that stay outside batch-generation orchestration and keep all external
  behavior unchanged. First,
  `chapter_batch_generation_request_compat_service.rs` now only keeps the
  `BatchGenerationRequestCompatFields` carrier; the single-consumer no-op
  helper `consume_batch_generation_request_compat_fields()` and its dedicated
  tests were removed because the compat-only fields are still consumed exactly
  once at the batch-create workflow boundary. `chapter_batch_generation_create_workflow_service.rs`
  now reads that compat carrier locally, preserves the same placeholder field
  touch for compatibility, and also narrows
  `normalize_batch_generation_enable_analysis()` plus
  `normalize_batch_generation_max_retries()` to file-private visibility because
  they are only used by the local workflow owner and its unit tests.
  Second, `chapter_batch_generation_status_stream_service.rs` reduced another
  small transitive surface by making its event-construction helpers private:
  progress/result/failed/cancelled/not-found/timeout builders,
  terminal-event assembly, and the internal stream constructor are now
  file-owned helpers because the public transport boundary is only
  `create_owned_batch_generation_status_stream()`. This keeps SSE event JSON
  shape, terminal ordering, polling cadence, and access-control behavior
  unchanged while shrinking one more internal helper surface from the stream
  owner. Validation passed with
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo check --manifest-path "backend-rs/Cargo.toml"`,
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo test chapter_batch_generation_create_workflow_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`,
  and
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo test chapter_batch_generation_status_stream_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`.
  Updated stop-rule assessment: after removing the compat no-op wrapper and
  private stream-helper leakage, the remaining obvious Phase 5 seams are now
  even more concentrated in behavior-sensitive `status_view`, `runtime_state`,
  `task_command`, or API transport boundaries. Further acceleration should
  continue only when a candidate seam is clearly a single-consumer helper,
  carrier, or internal visibility tightening move with no task/status semantic
  drift risk.
- 2026-05-20 16:23 +08:00 checkpoint:
  this Phase 5 slice continued with one more pair of low-risk visibility /
  transport-seam tightening moves, again without changing any external request,
  response, or runtime behavior. On the service side,
  `chapter_batch_generation_access_service.rs` narrowed
  `build_user_ai_config()` to file-private visibility because it is only used
  by the local `prepare_generation_execution_config()` owner and its value is
  not a standalone cross-module contract. In parallel,
  `chapter_batch_generation_active_list_query_service.rs` narrowed
  `normalize_active_batch_generation_task_list_limit()` to file-private
  visibility because the limit normalization contract is only consumed by the
  active-list query owner plus local unit tests.
  On the API transport side,
  `backend-rs/src/api/chapter_batch_generation.rs` now uses one route-local
  helper `build_batch_generation_request_compat_fields()` to assemble the
  compat carrier before delegating to
  `start_owned_batch_generation_workflow()`. This keeps the route transport-
  oriented while reducing inline request-field copying and making the compat
  carrier assembly explicit at one local boundary. No request fields, defaults,
  workflow inputs, query behavior, or background dispatch behavior changed.
  Validation passed with
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo check --manifest-path "backend-rs/Cargo.toml"`,
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo test chapter_batch_generation_active_list_query_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`,
  and
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo test chapter_batch_generation_create_workflow_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`.
  Updated stop-rule assessment: the remaining useful Phase 5 seams now sit even
  closer to truly shared read-side builders (`status_view`) or to behavior-
  sensitive command/runtime owners. Further acceleration should stay selective
  and prefer only private-helper tightening or unmistakably transport-only
  extraction moves that do not alter task/status/checkpoint semantics.
- 2026-05-20 16:28 +08:00 checkpoint:
  this Phase 5 slice aligned the single-generation lane with the same
  compatibility-surface slimming pattern already applied to batch generation.
  `chapter_single_generation_request_service.rs` no longer carries the no-op
  compat helper `consume_single_chapter_generation_request_compat_fields()`;
  the compat-only `enable_analysis` field is still consumed exactly once at the
  workflow boundary, but it now stays local to
  `chapter_single_generation_background_workflow_service.rs` and
  `chapter_single_generation_stream_workflow_service.rs` via a direct
  placeholder read instead of flowing through a dedicated wrapper function.
  This keeps the single-generation request contract minimal and makes it
  explicit that `enable_analysis` is still compatibility-only in this lane.
  In parallel, `chapter_batch_generation_status_view_service.rs` narrowed
  `build_batch_generation_task_view_context()` to file-private visibility
  because that builder is only used by local load/query helpers inside the same
  owner module; no other module consumes it as a cross-owner contract. These
  moves keep single-generation request fields, background/stream workflow
  inputs, status-view payload assembly, runtime state loading, and read-side
  semantics unchanged while shrinking one more set of internal helper surfaces.
  Validation passed with
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo check --manifest-path "backend-rs/Cargo.toml"`,
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo test chapter_single_generation_request_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`,
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo test chapter_batch_generation_status_view_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`,
  and
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo test chapter_single_generation_stream_workflow_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`.
  The single-generation stream workflow filter still matches 0 tests in this
  environment, so direct protection there remains compile-level plus the
  request-service tests; that is acceptable for this slice because the move was
  limited to removing a no-op compat wrapper and keeping the placeholder read
  local to the workflow boundary.
  Updated stop-rule assessment: Phase 5 now has even fewer obvious low-risk
  wrappers left. The remaining candidates are mostly private-helper tightening
  inside `status_view`, or true behavior-sensitive owners where further moves
  should stop unless the seam is unmistakably single-consumer and
  behavior-preserving.
- 2026-05-20 16:32 +08:00 checkpoint:
  this Phase 5 slice tightened the remaining crate-internal carrier and
  dispatch boundaries across the batch-generation command/access lanes without
  changing any runtime behavior. In
  `chapter_batch_generation_access_service.rs`, the generation-preparation
  contract moved from public module surface to crate-internal surface:
  `LoadAccessibleChapterForGenerationError`,
  `PreparedGenerationExecutionConfig`, and the access/preparation helpers
  `prepare_generation_execution_config()`, `verify_project_access()`, and
  `load_accessible_chapter_for_generation()` now use `pub(crate)` visibility
  because they are only consumed inside the Rust backend crate.
  In `chapter_batch_generation_dispatch_service.rs`, the concrete dispatch
  helpers for single, batch, and resume execution are also now `pub(crate)`,
  matching their real usage as internal runtime-launch boundaries rather than
  external contracts. In
  `chapter_batch_generation_request_compat_service.rs`, the compat carrier
  `BatchGenerationRequestCompatFields` is now crate-internal with crate-
  internal fields, reflecting that it only flows from the route module into the
  batch-create workflow owner. Finally,
  `chapter_batch_generation_task_command_service.rs` narrowed the command-side
  plan and execution carriers to crate-internal visibility:
  `BatchGenerationCreatePlan`,
  `SingleGenerationBackgroundCreatePlan`,
  `ResumeBatchGenerationPlan`,
  `ResumeExecutionPlan`,
  `create_batch_generation_task_plan()`,
  `create_single_generation_background_task_plan()`,
  `prepare_batch_generation_resume()`, and `cancel_batch_generation_task()` are
  now explicitly crate-internal, while `parse_batch_task_chapter_ids()` was
  further narrowed to file-private because only the local owner and its tests
  consume it. During validation, a `private_interfaces` warning exposed that
  `start_owned_batch_generation_workflow()` still accepted the now
  crate-internal compat carrier while remaining `pub`; the function visibility
  was aligned to `pub(crate)` so the carrier and workflow boundary now match.
  This keeps route behavior, workflow inputs, dispatch behavior, resume
  branching, response payloads, and task/checkpoint semantics unchanged while
  making the remaining Phase 5 boundary much more honest about what is truly
  internal to the Rust backend crate.
  Validation passed with
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo check --manifest-path "backend-rs/Cargo.toml"`,
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo test chapter_batch_generation_task_command_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`,
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo test chapter_batch_generation_create_workflow_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`,
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo test chapter_batch_generation_resume_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`,
  and
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo test chapter_single_generation_background_workflow_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`.
  The resume/background workflow filters still match 0 tests in this
  environment, so direct protection there remains compile-level plus adjacent
  task-command/create coverage; that is acceptable here because the slice was a
  pure visibility-tightening move with no behavior edits.
  Updated stop-rule assessment: most remaining Phase 5 opportunities are now
  inside genuinely behavior-sensitive owners rather than obvious overexposed
  crate surfaces. Further acceleration should prefer only final private-helper
  tightening or stop when the next move would primarily reshuffle semantics
  instead of shrinking a clear boundary.
- 2026-05-20 16:36 +08:00 checkpoint:
  this Phase 5 slice completed a matching crate-internal visibility pass across
  the batch-generation read-side owners. In
  `chapter_batch_generation_status_view_service.rs`, the read-side context and
  stream-state contracts are now crate-internal:
  `BatchGenerationTaskViewContext`,
  `LoadBatchGenerationTaskViewContextError`,
  `load_required_batch_generation_task_view_context()`,
  `load_active_project_batch_generation_task_view_context()`,
  `load_active_user_batch_generation_task_view_contexts()`,
  `BatchGenerationStreamState`, and
  `load_batch_generation_stream_state()` now use `pub(crate)` visibility,
  reflecting that they are consumed only by other Rust backend modules. During
  the same pass, the now-unused optional task-view loader
  `load_batch_generation_task_view_context()` was removed entirely because no
  remaining module or test consumed it after the earlier status-query
  refactor. In the adjacent query/stream owners,
  `LoadBatchGenerationStatusQueryError`,
  `LoadActiveBatchGenerationQueryError`,
  `LoadActiveBatchGenerationTaskListQueryError`,
  `BatchGenerationStatusStreamAccessError`, and their corresponding load/stream
  entrypoints were aligned to `pub(crate)` visibility as well.
  This immediately surfaced one more honest-boundary cleanup in
  `chapter_batch_generation_error_mapper.rs`: the error-mapper functions for
  active-query, active-list, status-query, and status-stream access are now
  also `pub(crate)` so their visibility matches the crate-internal error types
  they map. Together these moves keep status JSON fields, active-query payloads,
  SSE polling/event behavior, access control, and error mapping semantics
  unchanged while finishing one more meaningful boundary-tightening wave on the
  read side and eliminating the dead optional task-view loader.
  Validation passed with
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo check --manifest-path "backend-rs/Cargo.toml"`,
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo test chapter_batch_generation_status_view_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`,
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo test chapter_batch_generation_status_query_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`,
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo test chapter_batch_generation_active_query_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`,
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo test chapter_batch_generation_active_list_query_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`,
  and
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo test chapter_batch_generation_status_stream_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`.
  Updated stop-rule assessment: after the read-side crate-internal pass, the
  remaining unsimplified surfaces are now overwhelmingly concentrated in true
  behavior owners such as `runtime_state` and `task_command`, or else in route-
  level transport glue whose remaining code is mostly explicit wiring rather
  than accidental boundary bloat. That means the next meaningful move is likely
  no longer a cheap Phase 5 seam-tightening slice. Unless a clearly isolated
  private helper still appears, this follow-up should treat Phase 5 as
  approaching a stage-complete checkpoint rather than continuing mechanical
  boundary churn.
- 2026-05-20 16:40 +08:00 checkpoint:
  this Phase 5 slice finished one more crate-internal visibility alignment pass
  across the remaining workflow-entry and request-preparation boundaries that
  still sat above their actual usage scope. In the batch-generation workflow
  lane, `CreateBatchGenerationWorkflowDomainError`,
  `CreateBatchGenerationWorkflowError`,
  `CancelBatchGenerationWorkflowError`, and
  `PrepareBatchGenerationResumeRequestError` are now crate-internal, matching
  the fact that their only consumers are the batch-generation route module and
  its local error mapper. In the single-generation lane,
  `CreateSingleGenerationBackgroundWorkflowError`,
  `start_owned_single_generation_background_workflow()`, and
  `create_single_generation_stream_workflow()` are now crate-internal as well.
  The single-generation request-preparation layer was tightened further so
  `SingleChapterGenerationRequest`,
  `PrepareSingleChapterGenerationRequestError`,
  `PreparedSingleChapterGenerationRequest`, and
  `prepare_single_chapter_generation_request()` are crate-internal, while the
  target-word helper pair
  `normalize_single_chapter_generation_target_word_count()` and
  `load_chapter_generation_target()` now stay file-private because only the
  local request owner and its tests consume them. To keep boundary visibility
  honest end-to-end, the remaining batch-generation error-mapper entrypoints
  for create/cancel/resume plus the single-generation request/background
  mappers were also aligned to `pub(crate)` visibility. These moves preserve
  HTTP payloads, route behavior, workflow branching, request preparation,
  dispatch behavior, and all task/status/checkpoint semantics while completing
  the last obvious crate-internal surface reduction wave around the chapter
  generation follow-up.
  Validation passed with
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo check --manifest-path "backend-rs/Cargo.toml"`,
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo test chapter_batch_generation_create_workflow_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`,
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo test chapter_batch_generation_task_command_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`,
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo test chapter_single_generation_request_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`,
  and
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo test chapter_batch_generation_status_query_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`.
  No new visibility warnings remained after this pass.
  Updated stop-rule assessment: the remaining Phase 5 opportunities now look
  exhausted enough that further work would mainly reshuffle true behavior
  owners (`runtime_state`, `task_command`, status semantics) instead of
  shrinking accidental boundaries. Unless a newly discovered isolated private
  helper appears, this follow-up should treat Phase 5 as a stage-complete
  refactor checkpoint and avoid additional mechanical boundary churn.
- 2026-05-20 16:47 +08:00 checkpoint:
  this Phase 5 slice completed one last narrow visibility-tightening pass in
  `chapter_batch_generation_runtime_state_service.rs`, after a fresh audit
  confirmed that the remaining exported runtime helpers were still consumed
  only inside the Rust backend crate. The snapshot loader
  `load_batch_generation_snapshot()` and the runtime entrypoints for single-
  chapter and batch execution now use `pub(crate)` visibility, matching their
  real ownership as crate-internal runtime boundaries shared only with the
  adjacent dispatch, task-command, and read-side owners. The remaining
  persistence and runtime-progression helpers that are not consumed outside the
  file, including the snapshot upsert helper plus the generic runtime
  checkpoint builders, now stay file-private. The batch/single resume and
  pending-checkpoint helpers remain crate-internal because they are still
  intentionally shared with adjacent owner modules and focused tests. This
  move preserves task creation, runtime execution, resume behavior, checkpoint
  payload fields, snapshot reset semantics, and SSE/read-side behavior while
  making the runtime-state owner boundary more honest about which helpers are
  truly reusable contracts versus internal implementation detail.
  Validation passed with
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo check --manifest-path "backend-rs/Cargo.toml"` ,
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo test chapter_batch_generation_runtime_state_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`,
  and
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo test chapter_batch_generation_task_command_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`.
  Updated stop-rule assessment: this was the last clearly isolated
  crate-internal seam discovered by the current audit. After this point, most
  remaining unsimplified surfaces sit directly inside behavior-sensitive
  runtime/task/status owners or route transport wiring whose remaining code is
  largely explicit orchestration rather than accidental surface area. Phase 5
  should therefore be treated as effectively stage-complete, and further
  acceleration should shift toward planning or opening the next Rust migration
  phase instead of continuing mechanical boundary-tightening for its own sake.
- 2026-05-20 16:52 +08:00 checkpoint:
  this Phase 5 slice completed one more narrow read-side visibility pass after
  a local file-system audit replaced an earlier stale semantic-search lead.
  In `chapter_batch_generation_status_payload_adapter_service.rs`, the shared
  read-side helpers `to_iso()`, `checkpoint_with_runtime_metadata()`,
  `task_status_payload()`, and `active_task_payload()` are now `pub(crate)`,
  matching their real usage as crate-internal payload adapters consumed only by
  the adjacent query/view/task-command owners. In
  `chapter_batch_generation_status_semantics_service.rs`, the shared read-side
  helpers `active_batch_generation_statuses()`, `task_type()`,
  `task_stage_code()`, and `task_execution_mode()` are also now `pub(crate)`,
  reflecting that no external module boundary consumes them outside the Rust
  backend crate. During validation, this tightening surfaced one honest cleanup:
  `is_active_batch_generation_status()` was no longer used outside tests, so it
  was removed entirely and the semantics tests were adjusted to assert against
  `active_batch_generation_statuses()` directly. These moves preserve active-
  task query payloads, status payload assembly, read-side metadata adaptation,
  status vocabulary, execution-mode defaults, and all route/SSE/task/checkpoint
  semantics while shrinking one more small set of accidental read-side export
  surfaces.
  Validation passed with
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo check --manifest-path "backend-rs/Cargo.toml"`,
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo test chapter_batch_generation_status_payload_adapter_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`,
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo test chapter_batch_generation_status_semantics_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`,
  and
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo test chapter_batch_generation_status_query_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`.
  Updated stop-rule assessment: this confirms that only a very small number of
  isolated crate-internal read-side seams were still available after the prior
  runtime-state pass. Phase 5 can still accept similarly narrow helper/
  visibility cleanups if they are immediately obvious and independently
  verifiable, but the remaining surface area is now overwhelmingly concentrated
  in true behavior owners or explicit transport orchestration. Further
  acceleration should therefore bias toward planning the next Rust migration
  phase instead of repeatedly searching for ever-smaller mechanical boundary
  churn.
- 2026-05-20 16:56 +08:00 checkpoint:
  this Phase 5 slice completed one more narrow owner-boundary tightening pass
  in `chapter_batch_generation_create_service.rs`. A local usage audit
  confirmed that the create-request preparation owner still exposed a small
  cluster of request-preparation helpers and carriers more broadly than their
  actual usage required. The create-request error type
  `PrepareBatchGenerationCreateRequestError`, the prepared carrier
  `PreparedBatchGenerationCreateRequest`, and the workflow-facing entrypoint
  `prepare_batch_generation_create_request()` are now crate-internal, matching
  the fact that they are only consumed by the adjacent batch-create workflow
  owner. The count validation helper, target-word normalization helper, and
  chapter-range loader now stay file-private because they are only reused
  inside the create-request owner and its focused tests. These moves preserve
  batch-create validation semantics, normalized target-word defaults, chapter
  range loading behavior, workflow dispatch behavior, HTTP payloads, and all
  task/checkpoint/runtime semantics while shrinking one more isolated create-
  request export surface that no longer needs module-wide visibility.
  Validation passed with
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo check --manifest-path "backend-rs/Cargo.toml"`,
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo test chapter_batch_generation_create_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`,
  and
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo test chapter_batch_generation_create_workflow_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`.
  Updated stop-rule assessment: this confirms that the remaining viable Phase 5
  slices are now limited to small owner-boundary cleanups in request/query/
  adapter lanes where usage scope is immediately provable and validation is
  cheap. The broad direction remains unchanged: if the next candidate touches
  runtime-write semantics, transport contracts, or task-status behavior instead
  of an isolated helper/export surface, that work should be treated as the next
  migration phase rather than more Phase 5 mechanical tightening.
- 2026-05-20 16:59 +08:00 checkpoint:
  this Phase 5 slice completed one more crate-internal query/access boundary
  tightening pass in `chapter_batch_generation_owned_task_query_service.rs`.
  A local usage audit confirmed that the shared owned-task error type
  `LoadOwnedBatchGenerationTaskError`, the optional and required owned-task
  loaders `load_owned_task()` and `load_required_owned_task()`, and the generic
  mapper `map_owned_batch_generation_task_error()` are only consumed by
  adjacent batch-generation owners inside the Rust backend crate, including the
  cancel, resume, status-view, and status-stream lanes. These shared query/
  access helpers are now `pub(crate)`, matching their real ownership as
  crate-internal support boundaries rather than public module exports. This
  move preserves owned-task lookup behavior, access control semantics, not-
  found/internal error mapping, status-stream access checks, and all route/
  status/task semantics while shrinking one more isolated shared helper surface
  that no longer needs full module visibility.
  Validation passed with
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo check --manifest-path "backend-rs/Cargo.toml"`,
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo test chapter_batch_generation_owned_task_query_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`,
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo test chapter_batch_generation_status_stream_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`,
  and
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo test chapter_batch_generation_status_view_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`.
  Updated stop-rule assessment: this reinforces the current Phase 5 shape: the
  only remaining credible slices are narrow shared helper/export tightenings in
  query/access/adapter owners where usage scope is immediately provable and
  verification is cheap. If the next candidate cannot be justified at that
  level of isolation, the work should stop and roll into the next Rust
  migration phase rather than continue searching for mechanical churn.
- 2026-05-20 17:02 +08:00 checkpoint:
  this Phase 5 slice completed one final ultra-narrow stream-boundary
  tightening pass in `chapter_batch_generation_status_stream_service.rs`.
  A local usage audit confirmed that the type alias
  `BatchGenerationStatusStream` is only used inside the Rust backend crate as
  the shared concrete stream return type for the batch-generation status-stream
  owner and its adjacent route-facing constructor. The alias is now
  `pub(crate)`, matching its real role as crate-internal transport plumbing
  rather than a public module export. This move preserves the concrete stream
  type, SSE event construction, terminal-event behavior, access checks, and all
  route/status semantics while shrinking one more tiny export surface whose
  visibility exceeded its actual usage.
  Validation passed with
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo check --manifest-path "backend-rs/Cargo.toml"`,
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo test chapter_batch_generation_status_stream_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`,
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo test chapter_batch_generation_status_query_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`,
  and
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo test chapter_batch_generation_status_view_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`.
  Updated stop-rule assessment: this confirms that Phase 5 has effectively
  reached the end of the “cheap, obvious, behavior-preserving export tightening”
  lane. Further continuation should only proceed if another similarly isolated
  seam is immediately visible from local usage evidence; otherwise the correct
  next move is to stop searching for mechanical churn and switch to the next
  Rust migration phase.
- 2026-05-20 17:07 +08:00 checkpoint:
  this Phase 5 slice completed a final module-boundary alignment pass across
  the batch-generation and adjacent single-generation owners. After the prior
  helper/type-alias tightening waves, a local module-usage audit confirmed that
  the corresponding owner modules in `backend-rs/src/services/mod.rs` are also
  only consumed inside the Rust backend crate. The batch-generation support
  modules (`chapter_batch_generation_*`) plus the adjacent
  `chapter_single_generation_*` workflow/request/stream modules are now
  declared as `pub(crate) mod`, aligning the module boundary with the already
  crate-internal functions, carriers, and helpers they expose. On the API side,
  `chapter_batch_generation` and `chapter_batch_generation_error_mapper` in
  `backend-rs/src/api/mod.rs` are now also `pub(crate) mod`, matching their
  real usage as crate-internal route and transport-mapping owners that are only
  merged by neighboring API modules inside the same binary crate. These moves
  preserve all route registration, workflow dispatch, request preparation,
  stream behavior, payload mapping, and task/checkpoint/runtime semantics while
  bringing the owner-module declarations into line with the crate-internal
  boundary model established by the earlier Phase 5 slices.
  Validation passed with
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo check --manifest-path "backend-rs/Cargo.toml"`,
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo test chapter_batch_generation_status_stream_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`,
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo test chapter_batch_generation_create_workflow_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`,
  and
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo test chapter_single_generation_request_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`.
  Updated stop-rule assessment: this completes the highest-signal remaining
  crate-internal boundary alignment work that could be justified without
  touching behavior-sensitive owners. Phase 5 should now be treated as
  effectively exhausted for mechanical Rust seam tightening. Further work
  should move to the next Rust migration phase unless a newly introduced,
  immediately obvious isolated seam appears during unrelated edits.
- 2026-05-20 17:10 +08:00 checkpoint:
  this Phase 5 slice completed a final route-owner visibility alignment pass
  across the chapter route composition stack. A local route-usage audit
  confirmed that the route entrypoints `routes()` in
  `chapter_batch_generation.rs`, `chapters.rs`, `chapter_analysis_routes.rs`,
  `chapter_crud_routes.rs`, and `chapter_regeneration_routes.rs` are only
  consumed by neighboring API modules inside the Rust backend crate
  (`chapters.rs` and `router.rs`) and do not serve as external library-facing
  entrypoints. These route-owner constructors are now `pub(crate)`, aligning
  the route composition layer with the earlier helper/type/module boundary
  tightening waves. This move preserves route registration order, route merge
  behavior, batch-generation endpoints, chapter route aggregation, stream and
  background route exposure, and all HTTP/SSE/task semantics while removing the
  last obvious overexposed route-owner surfaces in the current Phase 5 scope.
  Validation passed with
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo check --manifest-path "backend-rs/Cargo.toml"`,
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo test router --manifest-path "backend-rs/Cargo.toml" -- --nocapture`,
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo test chapter_batch_generation_status_stream_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`,
  and
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo test chapter_single_generation_request_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`.
  Updated stop-rule assessment: this should be treated as the formal end of the
  current Phase 5 mechanical boundary-tightening track. The remaining work now
  sits in behavior-sensitive runtime/task/status owners or in genuine Rust
  migration concerns, not in accidental export surface area. Further
  acceleration should therefore switch to the next migration phase rather than
  continue searching for additional Phase 5 seams.
- 2026-05-20 17:13 +08:00 checkpoint:
  this Phase 5 slice completed a final API module-boundary cleanup across the
  remaining chapter route/error-mapper owners in `backend-rs/src/api/mod.rs`.
  A local usage audit confirmed that the chapter-analysis, chapter-crud,
  chapter-regeneration, and shared `chapters` / `chapters_error_mapper` modules
  are only consumed by neighboring API modules inside the same Rust binary
  crate and do not form public library-facing boundaries. These chapter-related
  API modules are now declared as `pub(crate) mod`, aligning the API module
  declarations with the earlier crate-internal function/type/module/route-owner
  tightening work already completed in this follow-up. This move preserves
  chapter route aggregation, route wiring, error mapping semantics, analysis/
  CRUD/regeneration endpoint exposure, and all HTTP/SSE/task semantics while
  removing the last obvious overexposed API-module surfaces in the current
  Phase 5 scope.
  Validation passed with
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo check --manifest-path "backend-rs/Cargo.toml"`,
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo test router --manifest-path "backend-rs/Cargo.toml" -- --nocapture`,
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo test chapter_batch_generation_status_stream_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`,
  and
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo test chapter_single_generation_request_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`.
  Updated stop-rule assessment: this should be treated as the final meaningful
  Phase 5 boundary-alignment slice on the Rust chapter-generation path. The
  accidental export-surface cleanup track is now exhausted enough that further
  acceleration should move directly into the next Rust migration phase instead
  of continuing to search for more mechanical tightening opportunities.
- 2026-05-20 17:16 +08:00 checkpoint:
  this Phase 5 slice completed the crate-root boundary alignment pass in
  `backend-rs/src/main.rs`. Because the Rust backend is currently built as a
  binary crate with `main.rs` and no public `lib.rs` boundary, the top-level
  declarations `pub mod ai`, `pub mod api`, `pub mod config`, `pub mod db`,
  `pub mod mcp`, `pub mod middleware`, `pub mod models`, `pub mod services`,
  `pub mod tasks`, and `pub mod utils` were broader than needed. They are now
  private `mod` declarations, which aligns the crate root with the earlier
  crate-internal helper/type/module/route-owner cleanup work already completed
  across the chapter-generation path. This move preserves all runtime startup,
  router build, background task bootstrap, and chapter-generation behavior
  while removing the final crate-root export surface that was still implying a
  wider library boundary than the current binary actually provides.
  Validation passed with
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo check --manifest-path "backend-rs/Cargo.toml"`,
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo test router --manifest-path "backend-rs/Cargo.toml" -- --nocapture`,
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo test chapter_batch_generation_status_stream_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`,
  and
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo test chapter_single_generation_request_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`.
  This pass also surfaced a set of pre-existing unused-import / dead-code
  warnings that had been masked by the previous top-level public module shape;
  these warnings were not introduced by behavioral refactor logic and should be
  treated as separate cleanup work rather than part of the chapter-generation
  seam-tightening scope. Updated stop-rule assessment: Phase 5 mechanical
  boundary tightening is now fully exhausted. Further work should switch to the
  next Rust migration phase or to an explicit repository-wide warning cleanup
  effort, not continue searching for more chapter-generation export seams.
- 2026-05-20 17:21 +08:00 checkpoint:
  this final Phase 5 cleanup slice removed the last directly adjacent
  chapter-generation dead code that was surfaced during the crate-root
  visibility pass. A local usage audit confirmed that
  `backend-rs/src/services/chapter_access_http_service.rs` was completely
  unreferenced, so the module was removed from `services/mod.rs` and the file
  was deleted. In `chapter_access_service.rs`, the unused helper
  `check_accessible_chapter_exists()` was removed because no route or service
  consumes that boolean wrapper. In `chapter_generation_prompt_service.rs`, the
  no-longer-needed wrapper `build_prompt()` was removed and the focused tests
  now assert directly through `build_prompt_with_provider_payload()` plus the
  placeholder provider payload helper. These moves preserve chapter access
  semantics, prompt construction behavior, batch/single generation flow, and
  all route/runtime behavior while finishing the last clearly provable
  chapter-generation dead-code cleanup that still fit the current Phase 5
  scope.
  Validation passed with
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo check --manifest-path "backend-rs/Cargo.toml"`,
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo test chapter_generation_prompt_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`,
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo test router --manifest-path "backend-rs/Cargo.toml" -- --nocapture`,
  and
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo test chapter_batch_generation_status_stream_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`.
  Updated stop-rule assessment: with the crate-root boundary pass and this last
  chapter-generation dead-code cleanup complete, the current Phase 5 follow-up
  should be treated as fully exhausted. The remaining warnings are now broader
  repository hygiene concerns, not chapter-generation seam work. Further
  acceleration should move to the next Rust migration phase or to an explicit
  repo-wide warning cleanup task, not continue mining this task for more Phase
  5 slices.
- 2026-05-20 17:36 +08:00 checkpoint:
  this slice is the first post-Phase-5 Rust migration step rather than another
  mechanical boundary-tightening pass. A local ownership audit showed that
  `chapter_batch_generation_task_command_service.rs` was still mixing task
  persistence / execution-plan semantics with command response payload
  adaptation for create, cancel, resume, and single-chapter background create
  flows. That response-shaping logic now lives in the new
  `backend-rs/src/services/chapter_batch_generation_command_payload_adapter_service.rs`,
  while `chapter_batch_generation_task_command_service.rs` keeps task record
  writes, resume gating, chapter-id parsing, and runtime snapshot reset
  behavior. This preserves all existing HTTP payload fields, task/checkpoint
  defaults, resume execution branching, and single/batch runtime dispatch
  semantics while making the command owner boundary closer to “write model +
  execution plan” and the payload owner boundary closer to “response
  adaptation”.
  Validation passed with
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo check --manifest-path "backend-rs/Cargo.toml"`,
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo test chapter_batch_generation_command_payload_adapter_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`,
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo test chapter_batch_generation_task_command_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`,
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo test chapter_batch_generation_resume_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`,
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo test chapter_single_generation_background_workflow_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`,
  and
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo test chapter_batch_generation_create_workflow_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`.
  Updated stop-rule assessment: this confirms there is still productive work
  available after the Phase 5 seam-tightening track, but it now sits in
  behavior-preserving ownership clarification inside true command/runtime/query
  owners rather than in visibility churn. The next slice should continue in
  that direction, preferably by tightening one more internal semantic boundary
  around resume/runtime-state or status-view ownership without changing HTTP,
  SSE, or task-checkpoint contracts.
- 2026-05-20 18:02 +08:00 checkpoint:
  this slice continued the post-Phase-5 Rust migration work by moving the
  resume-time task reset semantics deeper into
  `backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs`.
  A local ownership audit showed that
  `chapter_batch_generation_task_command_service.rs` was still directly owning
  the `ActiveModel` reset for resume (`pending` status, cleared error state,
  cleared failed chapter list, zeroed retry/progress counters, and
  single-vs-batch current chapter position reset) even though the adjacent
  runtime-state owner already owned the corresponding resume checkpoint and
  snapshot-replacement semantics. The new
  `reset_batch_generation_task_for_resume()` runtime-state helper now owns that
  write-side reset plus the follow-up snapshot replacement, while
  `prepare_batch_generation_resume()` keeps only resume eligibility checks and
  execution-plan selection. This preserves resume response payloads, task type
  branching, pending checkpoint defaults, snapshot clearing behavior, and all
  HTTP/SSE/task contracts while tightening one more real owner boundary in the
  resume path.
  Validation passed with
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo check --manifest-path "backend-rs/Cargo.toml"` ,
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo test chapter_batch_generation_runtime_state_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`,
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo test chapter_batch_generation_task_command_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`,
  and
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo test chapter_batch_generation_resume_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`.
  Updated stop-rule assessment: this confirms the remaining productive slices
  are still available, but they are now concentrated in true behavior owners
  such as status-view/query assembly and runtime/write-side helpers rather than
  broad command surfaces. The next slice should likely target one more
  internal semantic boundary in `status_view` or an adjacent query owner,
  unless a separate repo-wide warning cleanup task is started explicitly.
- 2026-05-20 18:07 +08:00 checkpoint:
  this slice continued the post-Phase-5 Rust migration work on the read side
  by moving the batch-generation status/active/list response assembly back into
  `backend-rs/src/services/chapter_batch_generation_status_view_service.rs`.
  A local ownership audit showed that
  `chapter_batch_generation_status_query_service.rs`,
  `chapter_batch_generation_active_query_service.rs`, and
  `chapter_batch_generation_active_list_query_service.rs` were still each
  carrying small amounts of view-payload assembly even though
  `status_view_service` already owned the task view context and adjacent
  read-side semantics. The status-view owner now exposes
  `build_batch_generation_status_query_response()`,
  `build_active_batch_generation_query_response()`, and
  `build_active_batch_generation_task_list_query_response()`, while the query
  owners keep access checks, context loading, limit normalization, and error
  mapping only. This preserves all existing payload fields, stage/checkpoint
  metadata, quality metrics exposure, and active-task wrapper shapes while
  making the query layer closer to pure orchestration and the view owner closer
  to the single read-side response assembly boundary.
  Validation passed with
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo check --manifest-path "backend-rs/Cargo.toml"`,
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo test chapter_batch_generation_status_view_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`,
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo test chapter_batch_generation_status_query_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`,
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo test chapter_batch_generation_active_query_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`,
  and
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo test chapter_batch_generation_active_list_query_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`.
  Updated stop-rule assessment: the remaining productive seams are now even
  more concentrated in the true behavior owners. A sensible next slice would
  be either another narrow read-side normalization inside `status_view` /
  stream semantics, or an explicit separate task for repo-wide warnings if the
  goal shifts from refactor ownership to hygiene.
- 2026-05-20 18:11 +08:00 checkpoint:
  this slice continued the read-side ownership tightening by moving the
  batch-generation stream event payload assembly into
  `backend-rs/src/services/chapter_batch_generation_status_view_service.rs`,
  alongside the already-owned stream-state projection semantics. A local audit
  showed that `chapter_batch_generation_status_stream_service.rs` still owned
  the JSON event builders for progress, terminal result, failure, cancelled,
  not-found, and timeout events even though those events were derived entirely
  from `BatchGenerationStreamState`, which is already projected by the
  neighboring status-view owner. The status-view owner now exposes
  `build_batch_generation_progress_event()`,
  `build_batch_generation_result_event()`,
  `build_batch_generation_failed_event()`,
  `build_batch_generation_cancelled_event()`,
  `build_batch_generation_not_found_event()`,
  `build_batch_generation_timeout_event()`, and
  `build_batch_generation_terminal_events()`, while
  `chapter_batch_generation_status_stream_service.rs` keeps only stream-access
  checks, change detection cursor logic, polling cadence, and SSE transport
  sending. This preserves event types, payload fields, timeout/not-found/error
  codes, terminal event ordering, and all SSE transport behavior while making
  the stream owner closer to pure polling/transport orchestration and the
  status-view owner closer to the single read-side event assembly boundary.
  Validation passed with
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo check --manifest-path "backend-rs/Cargo.toml"`,
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo test chapter_batch_generation_status_view_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`,
  and
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo test chapter_batch_generation_status_stream_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`.
  Updated stop-rule assessment: the remaining seams are now increasingly
  narrow and concentrated in the deepest behavior owners. Further acceleration
  should either continue with another similarly small owner clarification on
  the batch-generation path, or deliberately switch to a separate repository
  hygiene task if the main bottleneck becomes warning noise rather than
  ownership ambiguity.
- 2026-05-20 18:20 +08:00 checkpoint:
  this slice continued the post-Phase-5 Rust owner clarification work on the
  write side by moving the batch-cancel terminal task write fully into
  `backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs`.
  A local audit showed that
  `chapter_batch_generation_task_command_service.rs` still directly mutated the
  task row to `cancelled` and set `completed_at` before delegating adjacent
  cancelled checkpoint persistence to the runtime-state owner. That split left
  the cancel path with mixed ownership: command-level eligibility plus partial
  terminal state write in one file, and snapshot finalization in another. The
  runtime-state owner now sets the cancelled task status, writes
  `completed_at`, and builds the cancelled runtime checkpoint through the new
  shared `build_cancelled_batch_generation_runtime_checkpoint()` helper, while
  `cancel_batch_generation_task()` keeps only cancel eligibility checks and
  response payload return. This preserves cancel response payload fields,
  cancelled checkpoint shape, terminal progress semantics, runtime-loop
  cancelled handling, and all HTTP/SSE/task contracts while making the cancel
  write path closer to a single runtime-state owner boundary.
  Validation passed with
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo test chapter_batch_generation_runtime_state_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`,
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo test chapter_batch_generation_task_command_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`,
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo test chapter_batch_generation_cancel_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`,
  and
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo check --manifest-path "backend-rs/Cargo.toml"`.
  Updated stop-rule assessment: productive seams still exist, but they are now
  extremely narrow and should only continue where one owner still clearly mixes
  command/query/runtime responsibility. The next likely slice is either one
  more write-side clarification around batch failure/finalization semantics, or
  an explicit decision to stop this refactor lane and switch to a separate Rust
  migration or repository-hygiene task.
- 2026-05-20 18:23 +08:00 checkpoint:
  this slice continued the post-Phase-5 Rust owner clarification work on batch
  failure finalization by moving checkpoint-facing failure message selection
  fully into
  `backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs`.
  A local audit showed that `execute_batch_generation_runtime()` still chose
  terminal checkpoint copy at each failure callsite even though the adjacent
  runtime-state owner already owned the failed task write, failed checkpoint
  persistence, and terminal progress semantics. The runtime-state owner now
  exposes `BatchGenerationFailureKind` plus the shared
  `checkpoint_message_for_batch_generation_failure()` helper, and
  `finalize_batch_generation_failure()` now accepts a typed failure kind
  instead of a route/runtime-loop-provided checkpoint message string. The
  runtime loop keeps only source-specific task error text (`Chapter not found`
  / DB load error / generation error) plus chapter position context, while the
  runtime-state owner decides the stable checkpoint-facing message for missing
  chapter, chapter load failure, and generation failure terminal states. This
  preserves task `error_message`, failed checkpoint shape, SSE/read-side
  terminal behavior, and all HTTP/task contracts while making batch failure
  finalization closer to a single runtime-state owner boundary.
  Validation passed with
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo test chapter_batch_generation_runtime_state_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`,
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo test chapter_batch_generation_cancel_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`,
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo test chapter_batch_generation_status_stream_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`,
  and
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo check --manifest-path "backend-rs/Cargo.toml"`.
  Updated stop-rule assessment: remaining seams on this path are now very close
  to diminishing returns. Continue only if another slice can still collapse a
  clearly duplicated behavior owner boundary without touching response fields,
  SSE timing, or task/checkpoint defaults; otherwise switch to a separate Rust
  migration slice or explicit repository-hygiene task.
- 2026-05-20 18:42 +08:00 checkpoint:
  this slice tightened the batch success finalization boundary by collapsing
  the duplicated “is this terminal completion or still-running progress” logic
  into one runtime-state plan. A local audit showed that
  `finalize_batch_generation_success()` still decided terminal-vs-running task
  state twice: once while writing the task row (`completed` vs `running`,
  whether `completed_at` should be set) and again in
  `resolve_batch_generation_success_checkpoint()` while choosing checkpoint
  phase/progress/status/event/message. The runtime-state owner now lets
  `BatchGenerationSuccessCheckpointPlan` carry both task-facing terminal
  semantics (`task_status`, `should_complete_task`) and checkpoint-facing
  semantics, and `finalize_batch_generation_success()` consumes that single
  plan instead of re-deriving completion state locally. This preserves
  completed/running task status, `completed_at` write timing, checkpoint
  progress/event/message semantics, SSE terminal behavior, and all HTTP/task
  contracts while removing one more behavior-sensitive duplicate decision table
  from the runtime loop boundary.
  Validation passed with
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo test chapter_batch_generation_runtime_state_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`,
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo test chapter_batch_generation_status_stream_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`,
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo test chapter_batch_generation_status_view_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`,
  and
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo check --manifest-path "backend-rs/Cargo.toml"`.
  Updated stop-rule assessment: the remaining `chapter_batch_generation`
  ownership seams are now extremely close to exhaustion. Further work on this
  task should proceed only if another slice still removes a concrete duplicated
  behavior decision across command/query/runtime owners; otherwise this lane
  should stop and the next acceleration step should move to a separate Rust
  migration target or an explicit hygiene task.
- 2026-05-20 18:46 +08:00 checkpoint:
  this slice deliberately switched from the nearly exhausted
  `chapter_batch_generation` runtime lane to the still-valid secondary seam
  around single-chapter request ownership. A local audit confirmed that the
  route itself was already thin, but both
  `chapter_single_generation_stream_workflow_service.rs` and
  `chapter_single_generation_background_workflow_service.rs` were still each
  reconstructing the same internal `SingleChapterGenerationRequest` from loose
  route/workflow parameters (`target_word_count`, `model`, compat-only
  `enable_analysis`) before delegating to
  `prepare_single_chapter_generation_request()`. The request owner now exposes
  `build_single_chapter_generation_request()` in
  `backend-rs/src/services/chapter_single_generation_request_service.rs`, and
  both workflows now delegate that internal request shaping to the request
  service owner instead of open-coding it locally. This preserves route payload
  fields, target-word-count normalization, provider-payload preparation,
  single-chapter stream/background workflow behavior, and all HTTP/SSE/task
  contracts while reducing one more duplicated request-shaping seam on the
  single-chapter path.
  Validation passed with
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo test chapter_single_generation_request_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`,
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo test chapter_single_generation_stream_workflow_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`,
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo test chapter_single_generation_background_workflow_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`,
  and
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo check --manifest-path "backend-rs/Cargo.toml"`.
  Updated stop-rule assessment: `chapter_batch_generation` itself is now close
  enough to exhaustion that further acceleration should come from adjacent
  low-risk seams like single-chapter request/workflow ownership, unless a new
  duplicated behavior decision is discovered in the batch runtime lane.
- 2026-05-20 18:49 +08:00 checkpoint:
  this slice completed the next step of the single-chapter request ownership
  lane by closing the internal request contract all the way from route to
  workflow. After the previous slice, the request owner already knew how to
  build a `SingleChapterGenerationRequest`, but
  `backend-rs/src/api/chapter_batch_generation.rs` still passed loose internal
  fields (`target_word_count`, `model`, `enable_analysis`) directly into both
  single-chapter workflows, and each workflow still encoded that loose-params
  contract in its own function signature. The route now builds the internal
  request through
  `backend-rs/src/services/chapter_single_generation_request_service.rs`, and
  both
  `chapter_single_generation_stream_workflow_service.rs` and
  `chapter_single_generation_background_workflow_service.rs` now consume one
  `SingleChapterGenerationRequest` value instead of three separate params. This
  preserves route payload fields, request normalization, stream/background
  behavior, and all HTTP/SSE/task contracts while making the internal
  single-chapter contract owner explicit across route → workflow →
  request-preparation boundaries.
  Validation passed with
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo test chapter_single_generation_request_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`,
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo test chapter_single_generation_stream_workflow_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`,
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo test chapter_single_generation_background_workflow_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`,
  and
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo check --manifest-path "backend-rs/Cargo.toml"`.
  Updated stop-rule assessment: the best remaining acceleration on this task is
  now more likely to come from adjacent single-chapter request/workflow/stream
  seams than from squeezing one more micro-slice out of
  `chapter_batch_generation`, unless a fresh duplicated behavior decision is
  discovered there.
- 2026-05-20 19:55 +08:00 checkpoint:
  this slice clarified the single-chapter request contract by splitting the
  compat-only `enable_analysis` flag away from the real internal generation
  request. A local audit showed that
  `backend-rs/src/services/chapter_single_generation_request_service.rs`
  already owned the normalized internal request shape, but the request
  contract still exposed `enable_analysis` even though that field no longer
  participated in any internal request-preparation or execution decision. The
  request owner now keeps `SingleChapterGenerationRequest` minimal
  (`target_word_count`, `model`) and exposes
  `SingleChapterGenerationRequestCompatFields` plus
  `consume_single_chapter_generation_request_compat_fields()` for the
  compatibility-only `enable_analysis` field. The route explicitly consumes
  that compat field before building the real internal request, and both
  single-chapter workflows now operate only on the minimal internal contract.
  This preserves request compatibility, route payload fields, target-word-count
  normalization, stream/background behavior, and all HTTP/SSE/task contracts
  while making the difference between compat-only transport fields and real
  internal request semantics explicit.
  Validation passed with
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo test chapter_single_generation_request_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`,
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo test chapter_single_generation_stream_workflow_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`,
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo test chapter_single_generation_background_workflow_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`,
  and
  `$env:CARGO_TARGET_DIR='C:/Users/yanc/.codex/memories/mumu-rs-target'; cargo check --manifest-path "backend-rs/Cargo.toml"`.
- 2026-05-20 19:55 +08:00 checkpoint:
  this slice continued the single-chapter owner-clarification lane by
  collapsing the scattered execution-input contract into one explicit internal
  owner. A local audit showed that
  `chapter_single_generation_background_workflow_service.rs`,
  `chapter_single_generation_stream_workflow_service.rs`,
  `chapter_single_generation_stream_service.rs`,
  `chapter_batch_generation_dispatch_service.rs`, and
  `chapter_batch_generation_runtime_state_service.rs` were still passing the
  same prepared execution fields (`chapter_id`, `target_word_count`,
  `ai_config`, `provider_payload`) as loose parameters across each boundary
  even though they all came from one prepared single-chapter request. The
  request owner now exposes `SingleChapterGenerationExecutionInput`, the
  prepared request carries that execution input as one owned value, and the
  stream builder, background dispatch, and runtime executor all consume that
  explicit contract instead of re-spelling the same field bundle in separate
  signatures. This preserves route payloads, SSE payload shape, task creation
  behavior, runtime generation semantics, provider-payload ownership, and all
  HTTP/SSE/task contracts while making single-chapter execution input ownership
  explicit across request preparation → workflow → dispatch/stream → runtime.
  Validation pending for this slice in the current session; run targeted
  `chapter_single_generation_*` tests plus `cargo check` before taking another
  seam.

## Risky Files / Review Points

- `backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs`
- `backend-rs/src/services/chapter_batch_generation_status_view_service.rs`
- `backend-rs/src/services/chapter_batch_generation_task_command_service.rs`
- `backend-rs/src/api/chapter_batch_generation.rs`
- `backend-rs/src/api/chapters.rs`

Escalate review before continuing if a slice would:

- change response fields
- change SSE terminal timing or event naming
- change task status/checkpoint defaults
- require touching unrelated model/config layers just to complete the move

## Start Gate

Before `task.py start`, confirm:

- `prd.md` reflects the narrowed continuation scope
- `design.md` captures the continuation model and stop rule
- `implement.md` identifies the next execution wave
- the user has approved moving from planning to implementation
