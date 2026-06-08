# Implementation Plan

## Execution Rule

Do not start implementation until the planning artifacts in this task are
reviewed and approved.

## 2026-06-07 Rust-First Re-Plan

The next execution rounds must not start by moving more Python compatibility
helpers. Recent Python cleanup is useful fallback shrink evidence, but it is
not the primary migration lane. The primary lane is now Rust owner completion
in `backend-rs`, followed by Python fallback shrink only after the Rust owner
has focused validation.

New package order:

1. `chapter_single_generation` Rust owner package:
   prepare, write, stream, runtime, snapshot, task-model, and quality-status.
2. `chapter_generation` shared owner package:
   shared prompt/runtime/candidate/quality semantics currently surfaced through
   the remaining generation compatibility shell.
3. `chapter_batch_generation` Rust owner package:
   read, write, resume, cancel, status, stream, runtime, and task-view.
4. `chapters` compatibility shell shrink:
   only after the matching Rust owner, route parity, smoke evidence, and
   rollback boundary are explicit.
5. `schema / migration owner`:
   promote table/field assumptions when a route package exposes ownership
   pressure.

Package start gate:

- identify the Rust route/service owner files first
- identify the Python fallback shell second
- define preserved HTTP/SSE/task/checkpoint/provider/error contracts
- define focused Rust tests and `cargo check` command before editing
- define Python fallback shrink as the final step, not the lead change

For the current state, the surviving
`backend/app/services/compat/chapter_generation_route_compat_service.py` is a
source map, not the next Python edit target. The next implementation package
should start from `backend-rs` single-generation or shared generation owners
and only then repoint or remove that Python shell.

## Ordered Checklist

1. Inspect the current `backend-rs` and Python backend state, then select one
   Rust-first whole-file, whole-function-group, or whole-module migration
   package for this round.
2. Load the relevant backend spec indexes before editing code.
3. Record the package map before implementation:
   Rust target files, Python source/fallback files, behavior contract,
   validation commands, and rollback/cutover evidence.
4. Migrate or tighten the Rust owner first as a coherent unit. Whole files and
   whole function groups should move together when they belong to the same
   behavior owner.
5. Use micro-slices only as internal review checkpoints inside the selected
   package; do not report them as standalone migration completion.
6. Remove, freeze, or repoint Python legacy wrappers only after the Rust owner
   and fallback behavior are explicit and validated.
7. Add or update focused tests for changed service behavior, payload shape,
   task lifecycle, checkpoint, SSE, provider default, or error shell semantics.
8. Run validation with `cargo check`, targeted Rust tests, and route-group
   smoke/manifest checks when transport ownership or fallback behavior changes.
9. Leave a package checkpoint that states completed owner scope, remaining
   Python shell, rollback boundary, and the next package entrypoint.

## Latest Checkpoint

- 2026-06-08 chapter_draft/history analysis-read-context owner checkpoint:
  this round continued the same Rust-first draft/history package and finished
  the next route-facing read owner tightening step. The goal was to stop
  `chapter_analysis_read_context_service.rs` from keeping one more local
  `candidate_attempt + generation_history` query seam after the shared
  `GenerationHistory` owner had already been established.

  package map for this checkpoint:

  - Python source map:
    - `backend/app/services/chapter_generation/history_service.py`
    - `backend/app/services/chapter_draft_query_service.py`
    - `backend/app/services/chapter_analysis_response_service.py`
  - Rust target map:
    - updated `backend-rs/src/services/chapter_analysis_read_context_service.rs`
    - updated `backend-rs/src/services/chapter_draft_history_service.rs`
    - reused existing owner
      `backend-rs/src/services/chapter_draft_source_service.rs`
    - validated neighboring consumers:
      - `backend-rs/src/services/chapter_analysis_view_query_service.rs`
      - `backend-rs/src/services/chapter_quality_metrics_query_service.rs`
      - `backend-rs/src/services/chapter_single_generation_runtime_restore_service.rs`
  - behavior contract preserved:
    - analysis read-context still returns the latest candidate draft attempt
      plus the latest 30 chapter histories
    - history ordering remains descending `created_at`
    - chapter analysis payloads, quality metrics payloads, and restored
      single-generation runtime quality fragments remain shape-compatible
    - this is still an internal Rust owner consolidation; no HTTP/SSE route
      behavior or fallback knob changed in this round
  - rollback / remaining Python boundary:
    - Python draft/history route shells still remain source maps and fallback
      references; this round only shrinks a remaining Rust local query seam
    - if the shared Rust read-context owner regresses, rollback is limited to
      restoring the local chapter-analysis read queries; no deploy-time route
      cutover is involved yet

  focused validation for this checkpoint:

  - `cargo fmt --manifest-path "backend-rs/Cargo.toml"`
  - `cargo test chapter_draft_history_service --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-draft-history-owner" -- --nocapture`
  - `cargo test chapter_analysis_view_query_service --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-draft-history-owner" -- --nocapture`
  - `cargo test chapter_quality_metrics_query_service --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-draft-history-owner" -- --nocapture`
  - `cargo test chapter_single_generation_runtime_restore_service --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-draft-history-owner" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-draft-history-owner"`

  next package entrypoint:

  - keep the same draft/history package and continue toward route-facing
    draft detail / apply / history-write consolidation instead of reopening
    local history read seams
  - after this read-context owner stays stable, the next whole-file package
    should prefer:
    - draft/apply/history-write owner consolidation, or
    - chapter analysis / draft route package repoint with explicit rollback
      evidence

- 2026-06-08 chapter_draft/history GenerationHistory Rust owner checkpoint:
  this round continued the Rust-first `chapter_single_generation` /
  shared `chapter_generation` package by consolidating the
  `backend/app/services/chapter_generation/history_service.py` generation-history
  owner into a dedicated Rust history-backed service. The goal was not to add
  another staged helper, but to stop scattering checker/reviser/latest-history
  semantics across `source/view/query` owners in `backend-rs`.

  package map for this checkpoint:

  - Python source map:
    - `backend/app/services/chapter_generation/history_service.py`
    - `backend/app/services/chapter_draft_query_service.py`
    - `backend/app/services/chapter_draft_workflow_service.py`
    - `backend/app/services/chapter_analysis_response_service.py`
    - compatibility re-export source map remains
      `backend/app/services/chapter_generation_history_service.py`
  - Rust target map:
    - added `backend-rs/src/services/chapter_draft_history_service.rs`
    - updated `backend-rs/src/services/chapter_draft_source_service.rs`
    - updated `backend-rs/src/services/chapter_draft_view_payload_service.rs`
    - updated `backend-rs/src/services/chapter_draft_apply_service.rs`
    - updated `backend-rs/src/services/chapter_analysis_draft_service.rs`
    - updated `backend-rs/src/services/chapter_analysis_view_query_service.rs`
    - updated `backend-rs/src/services/mod.rs`
  - behavior contract preserved:
    - latest reviser lookup still prefers explicit `history_id`, otherwise scans
      latest chapter histories with the same 60-item cap
    - checker history parsing still only accepts
      `log_type = chapter_text_checker_v1`
    - reviser history parsing still only accepts
      `log_type = chapter_text_reviser_v1`
    - chapter analysis payload fields, draft detail payload fields, and draft
      apply response/error shells remain unchanged
    - this is an internal Rust owner consolidation; no HTTP/SSE contract or
      rollback knob changed in this round
  - rollback / remaining Python boundary:
    - Python history/draft source files remain source maps and fallback
      references; this checkpoint does not retire their FastAPI ownership
    - if the new Rust owner regresses, rollback is limited to repointing
      `chapter_analysis_*` / `chapter_draft_*` consumers back to the previous
      `source/view/query` split; no deploy-time route cutover is involved yet
    - do not count `history_service.py` as retired until the matching draft /
      analysis route package and Python compatibility references are shrunk or
      frozen behind explicit Rust route ownership

  focused validation for this checkpoint:

  - `cargo fmt --manifest-path "backend-rs/Cargo.toml"`
  - `cargo test chapter_draft_history_service --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-draft-history-owner" -- --nocapture`
  - `cargo test chapter_analysis_view_query_service --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-draft-history-owner" -- --nocapture`
  - `cargo test chapter_analysis_draft_service --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-draft-history-owner" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-draft-history-owner"`

  next package entrypoint:

  - continue the same draft/history package by shrinking the remaining local
    history-read assembly in `chapter_analysis_read_context_service.rs` and the
    route-facing draft detail/apply package, instead of returning to
    Python-side helper cleanup
  - after the Rust draft/history owner stays stable, choose the next whole-file
    package as either:
    - draft/apply/history-write owner consolidation, or
    - chapter analysis / draft route package repoint with explicit rollback
      evidence

- 2026-06-08 chapter_single_generation candidate-executor default-on cutover
  checkpoint:
  this round converted the staged Rust candidate executor into the default
  active single-generation executor path for `backend-rs`. The single-chapter
  route/runtime chain was already consuming the Rust candidate route gateway;
  this checkpoint changes the production default from Python fallback first to
  Rust executor first, while keeping the explicit rollback boundary and
  fallback-on-error behavior.

  package map for this checkpoint:

  - Python source map:
    - `backend/app/services/chapter_candidate_executor_service.py`
    - `backend/app/services/chapter_candidate_executor_wiring_service.py`
    - `backend/app/services/compat/chapter_generation_route_compat_service.py`
    - `backend/app/api/chapters.py`
  - Rust target map:
    - `backend-rs/src/config.rs`
    - `backend-rs/src/services/chapter_candidate_route_gateway_service.rs`
    - `backend-rs/src/services/chapter_candidate_executor_production_adapter_service.rs`
    - `backend-rs/src/services/chapter_candidate_executor_runtime_adapter_service.rs`
    - `backend-rs/src/services/chapter_generation_runtime_service.rs`
    - `backend-rs/src/api/chapter_generation_routes.rs`
  - behavior contract preserved:
    - single-generation HTTP route paths and SSE payload shapes are unchanged
    - active single-generation runtime still goes through the same candidate
      route gateway and converts gateway output back into the existing
      `GeneratedChapterResult` persistence/history path
    - rollback boundary remains `python_candidate_executor_fallback`
    - Rust executor errors still auto-fallback when
      `CHAPTER_CANDIDATE_RUST_EXECUTOR_FALLBACK_ON_ERROR=true`
    - Python source files remain frozen fallback/source-map owners; this is
      default-path cutover, not Python shell retirement
  - rollback / remaining Python boundary:
    - operators can still disable the Rust candidate executor explicitly with
      `CHAPTER_CANDIDATE_RUST_EXECUTOR_ENABLED=false`
    - if Rust candidate execution regresses at runtime, the production adapter
      still falls back to the Python-compatible branch when fallback-on-error
      stays enabled
    - Python FastAPI route shell and Python candidate executor source map are
      still the next shrink/repoint targets, not part of this cutover

  focused validation for this checkpoint:

  - `cargo test chapter_candidate_route_gateway_service --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-candidate-route-gateway-smoke-owner" -- --nocapture`
  - `cargo test chapter_generation_runtime_service --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-candidate-route-gateway-smoke-owner" -- --nocapture`
  - `cargo test chapter_single_generation_active_gateway_smoke_service --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-candidate-route-gateway-smoke-owner" -- --nocapture`
  - `cargo test health --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-candidate-route-gateway-smoke-owner" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-candidate-route-gateway-smoke-owner"`

  next package entrypoint:

  - shrink or repoint the still-frozen Python candidate executor fallback
    shell after this default-on path stays stable under smoke and business
    verification
  - continue whole-file / whole-module migration on remaining
    `chapter_single_generation` and shared `chapter_generation` owners instead
    of returning to Python helper-only cleanup

- 2026-06-08 chapter-candidate-targeted-final-repair Rust staged owner checkpoint:
  this round continued the Rust-first shared `chapter_generation` candidate
  owner package and ported Python
  `chapter_candidate_targeted_final_repair_service.py` as a whole targeted
  repair workflow owner. This remains staged because the production candidate
  executor still runs through the Python executor/wiring path.

  package map for this checkpoint:

  - Python source map:
    - `backend/app/services/chapter_candidate_targeted_final_repair_service.py`
    - formula source map remains
      `backend/app/services/chapter_candidate_rerank_service.py` for targeted
      suffix, temperature, max-token, char-limit, keep/adopt/prefer, and
      follow-up formulas
    - production candidate execution still reaches targeted repair through
      `backend/app/services/chapter_candidate_executor_service.py`
  - Rust target map:
    - added
      `backend-rs/src/services/chapter_candidate_targeted_final_repair_service.rs`
    - registered the module in `backend-rs/src/services/mod.rs`
    - composes directly with
      `backend-rs/src/services/chapter_candidate_runtime_state_service.rs`
      for targeted repair runtime-state sync
  - behavior contract preserved:
    - targeted repair path / attempt kind remain `targeted_quality_repair`
    - repair prompt joins base prompt, targeted suffix, and previous draft
      block with the Python-compatible `Previous draft to rewrite` wrapper
    - repair kwargs override prompt, temperature, and max_tokens through
      injected formula callbacks
    - repair output collection receives the resolved max-output char limit
    - repair candidate record construction is delegated to the record-builder
      callback
    - repair seed metadata is attached into
      `quality_metrics.candidate_selection`
    - failed repair collection falls back to the original selected candidate
      and candidate list
    - kept/adopted/preferred targeted repair candidates can replace the winner
    - kept but not adopted candidates can become deferred follow-up repair
      seeds only when `allow_followup_seed_defer` is true
  - rollback / remaining Python boundary:
    - if this staged owner regresses, remove its module registration and keep
      the Python active path unchanged
    - do not count `chapter_candidate_targeted_final_repair_service.py` as
      active-path retired until a Rust candidate executor consumes
      `execute_targeted_final_repair_pass_workflow(...)`
    - rerank-heavy formulas remain injectable until the rerank/executor
      package cuts over

  focused validation passed with:

  - `cargo fmt --manifest-path "backend-rs/Cargo.toml"`
  - `cargo test chapter_candidate_targeted_final_repair_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/mumunovel-candidate-targeted-final-repair-owner" -- --nocapture`
  - `cargo test chapter_candidate_word_budget_repair_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/mumunovel-candidate-targeted-final-repair-owner" -- --nocapture`
  - `cargo test chapter_candidate_finalize_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/mumunovel-candidate-targeted-final-repair-owner" -- --nocapture`
  - `cargo test chapter_candidate_generation_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/mumunovel-candidate-targeted-final-repair-owner" -- --nocapture`
  - `cargo test chapter_candidate_record_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/mumunovel-candidate-targeted-final-repair-owner" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/mumunovel-candidate-targeted-final-repair-owner"`
  - `git diff --check -- "backend-rs/src/services/chapter_candidate_targeted_final_repair_service.rs" "backend-rs/src/services/mod.rs"`

  next package entrypoint:

  - move into a staged Rust candidate executor / wiring function group that
    composes output / generation / record / word-budget repair / targeted
    final repair / finalize / runtime-state owners.
  - keep Python fallback shrink as the follow-up step, not the lead change.

- 2026-06-08 chapter-candidate-word-budget-repair Rust staged owner checkpoint:
  this round continued the Rust-first shared `chapter_generation` candidate
  owner package and ported Python
  `chapter_candidate_word_budget_repair_service.py` as a whole repair workflow
  owner. This is intentionally staged because the production candidate
  executor still runs through the Python executor/wiring path.

  package map for this checkpoint:

  - Python source map:
    - `backend/app/services/chapter_candidate_word_budget_repair_service.py`
    - formula source map remains
      `backend/app/services/chapter_candidate_rerank_service.py` for repair
      apply/keep/prefer, suffix, temperature, max-token, and char-limit
      formulas
    - production candidate execution still reaches repair through
      `backend/app/services/chapter_candidate_executor_service.py`
  - Rust target map:
    - added
      `backend-rs/src/services/chapter_candidate_word_budget_repair_service.rs`
    - registered the module in `backend-rs/src/services/mod.rs`
    - composes directly with
      `backend-rs/src/services/chapter_candidate_runtime_state_service.rs`
      for word-budget repair labels and runtime-state sync
  - behavior contract preserved:
    - skip path returns the original selected candidate and candidate list
    - repair prompt joins base prompt, repair suffix, and previous draft block
      with the Python-compatible `Previous draft to rewrite` wrapper
    - repair kwargs override prompt, temperature, and max_tokens through
      injected formula callbacks
    - repair output collection receives the resolved max-output char limit
    - repair candidate record construction is delegated to the record-builder
      callback
    - repair seed metadata is attached into
      `quality_metrics.candidate_selection`
    - failed repair collection falls back to the original selected candidate
      and does not mark repair as used
    - kept repair candidates are appended and may replace the selected winner
      through injected select/prefer callbacks
  - rollback / remaining Python boundary:
    - if this staged owner regresses, remove its module registration and keep
      the Python active path unchanged
    - do not count `chapter_candidate_word_budget_repair_service.py` as
      active-path retired until a Rust candidate executor consumes
      `maybe_apply_word_budget_repair_workflow(...)`
    - rerank-heavy formulas remain injectable until the rerank/executor
      package cuts over

  focused validation passed with:

  - `cargo fmt --manifest-path "backend-rs/Cargo.toml"`
  - `cargo test chapter_candidate_word_budget_repair_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/mumunovel-candidate-word-budget-owner" -- --nocapture`
  - `cargo test chapter_candidate_generation_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/mumunovel-candidate-word-budget-owner" -- --nocapture`
  - `cargo test chapter_candidate_record_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/mumunovel-candidate-word-budget-owner" -- --nocapture`
  - `cargo test chapter_candidate_finalize_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/mumunovel-candidate-word-budget-owner" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/mumunovel-candidate-word-budget-owner"`
  - `git diff --check -- "backend-rs/src/services/chapter_candidate_word_budget_repair_service.rs" "backend-rs/src/services/mod.rs"`

  next package entrypoint:

  - migrate `chapter_candidate_targeted_final_repair_service.py` as the next
    whole repair owner, or move directly into a staged Rust candidate executor
    that composes output / generation / record / word-budget repair / finalize
    / runtime-state owners.
  - keep Python fallback shrink as the follow-up step, not the lead change.

- 2026-06-08 chapter-candidate-finalize Rust staged owner checkpoint:
  this round continued the Rust-first shared `chapter_generation` candidate
  owner package and ported Python `chapter_candidate_finalize_service.py` as a
  whole finalization owner. This is real Rust owner preparation for the
  candidate executor cutover, but it remains staged because the production
  candidate executor still runs through the Python executor/wiring path.

  package map for this checkpoint:

  - Python source map:
    - `backend/app/services/chapter_candidate_finalize_service.py`
    - formula source map remains
      `backend/app/services/chapter_candidate_rerank_service.py` for
      candidate-selection metadata, pool summary, best-candidate selection,
      quality-gate normalization, and word-budget repair preference formulas
    - production candidate execution still reaches finalization through
      `backend/app/services/chapter_candidate_executor_service.py`
  - Rust target map:
    - added `backend-rs/src/services/chapter_candidate_finalize_service.rs`
    - registered the module in `backend-rs/src/services/mod.rs`
    - composes directly with
      `backend-rs/src/services/chapter_candidate_runtime_state_service.rs`
      for final runtime-state sync
  - behavior contract preserved:
    - selected-candidate labels fall back to Rust candidate runtime-state
      semantics when the record does not carry explicit labels
    - final generation path remains `single_pass`, `rerank_retry`, or
      `word_budget_repair`
    - final quality-gate plan is rebuilt, normalized, copied into
      `quality_metrics.quality_gate`, then enriched with final
      candidate-selection metadata
    - candidate-selection metadata is attached into
      `quality_metrics.candidate_selection` and flattened onto the selected
      candidate record
    - non-saveable final candidates may promote a preferred word-budget repair
      candidate through the injected preference callback
    - final runtime-state sync input carries candidate index, total,
      character count, chunk count, generation path, attempt kind, rerank flag,
      word-budget repair flag, and winner index
  - rollback / remaining Python boundary:
    - if this staged owner regresses, remove its module registration and keep
      the Python active path unchanged
    - do not count `chapter_candidate_finalize_service.py` as active-path
      retired until a Rust candidate executor consumes
      `finalize_selected_candidate_result(...)`
    - rerank-heavy formulas remain injectable until the rerank/executor
      package cuts over

  focused validation passed with:

  - `cargo fmt --manifest-path "backend-rs/Cargo.toml"`
  - `cargo test chapter_candidate_finalize_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/mumunovel-candidate-finalize-owner" -- --nocapture`
  - `cargo test chapter_candidate_generation_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/mumunovel-candidate-finalize-owner" -- --nocapture`
  - `cargo test chapter_candidate_record_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/mumunovel-candidate-finalize-owner" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/mumunovel-candidate-finalize-owner"`
  - `git diff --check -- "backend-rs/src/services/chapter_candidate_finalize_service.rs" "backend-rs/src/services/mod.rs"`

  next package entrypoint:

  - migrate a Rust candidate executor / wiring function group that composes:
    - `chapter_candidate_output_service.rs`
    - `chapter_candidate_generation_service.rs`
    - `chapter_candidate_record_service.rs`
    - `chapter_candidate_finalize_service.rs`
    - `chapter_candidate_runtime_state_service.rs`
  - if a full executor cutover is still too large, migrate one whole remaining
    dependency owner next:
    `chapter_candidate_word_budget_repair_service.py` or
    `chapter_candidate_targeted_final_repair_service.py`.
  - keep Python fallback shrink as the follow-up step, not the lead change.

- 2026-06-08 chapter-candidate-record Rust staged owner checkpoint:
  this round continued the Rust-first shared `chapter_generation` candidate
  owner package and ported Python `chapter_candidate_record_service.py` as a
  whole record-construction owner. This keeps migration progress in Rust code
  instead of doing another Python compatibility cleanup, but it is still
  recorded as staged because production candidate execution has not cut over to
  the Rust candidate executor.

  package map for this checkpoint:

  - Python source map:
    - `backend/app/services/chapter_candidate_record_service.py`
    - dependency source map remains
      `backend/app/services/chapter_candidate_rerank_service.py` for the
      quality-gate and candidate-selection metadata formulas
    - production candidate execution still reaches this behavior through the
      Python candidate executor/wiring path
  - Rust target map:
    - added `backend-rs/src/services/chapter_candidate_record_service.rs`
    - registered the module in `backend-rs/src/services/mod.rs`
    - updated `backend-rs/src/services/chapter_candidate_generation_service.rs`
      with a composition test that uses the Rust record owner as the
      generation workflow record-builder dependency
  - behavior contract preserved:
    - generated text is sanitized by the Rust narrative cleaner before quality
      evaluation
    - meta-only sanitized output returns the same empty-narrative error shape
      and emits the optional warning callback
    - quality-gate plan builder runs before and after candidate-selection
      metadata enrichment
    - empty enriched plan falls back to the initial normalized plan
    - candidate selection metadata is attached into `quality_metrics` and
      flattened onto the candidate record
    - word-budget pressure can normalize `allow_save` to `auto_repair`, matching
      the Python rerank owner semantics
  - rollback / remaining Python boundary:
    - if this staged owner regresses, remove the module registration and the
      generation composition test; the Python active path remains unchanged
    - do not count `chapter_candidate_record_service.py` as active-path
      retired until Rust candidate executor/wiring consumes this owner in the
      production candidate path

  focused validation passed with:

  - `cargo fmt --manifest-path "backend-rs/Cargo.toml"`
  - `cargo test chapter_candidate_record_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/mumunovel-candidate-record-owner" -- --nocapture`
  - `cargo test chapter_candidate_generation_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/mumunovel-candidate-record-owner" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/mumunovel-candidate-record-owner"`
  - `git diff --check -- backend-rs/src/services/chapter_candidate_record_service.rs backend-rs/src/services/chapter_candidate_generation_service.rs backend-rs/src/services/mod.rs`

  next package entrypoint:

  - migrate the Rust candidate executor / wiring function group so production
    candidate execution composes the Rust owners already available:
    - `chapter_candidate_output_service.rs`
    - `chapter_candidate_generation_service.rs`
    - `chapter_candidate_record_service.rs`
    - `chapter_candidate_runtime_state_service.rs`
  - keep Python fallback shrink as the follow-up step, not the lead change.

- 2026-06-08 chapter-candidate-generation workflow Rust staged owner checkpoint:
  this round stayed on the Rust-first shared `chapter_generation` candidate
  owner package and ported the whole Python candidate-pool workflow function
  group from `chapter_candidate_generation_service.py` into a tested Rust
  owner. This is intentionally recorded as a staged owner, not as production
  cutover, because the Rust candidate executor has not yet consumed it.

  package map for this checkpoint:

  - Python source map:
    - `backend/app/services/chapter_candidate_generation_service.py`
    - adjacent production consumers remain in
      `backend/app/services/chapter_candidate_executor_service.py`
      and `backend/app/services/chapter_candidate_executor_wiring_service.py`
  - Rust target map:
    - added `backend-rs/src/services/chapter_candidate_generation_service.rs`
    - registered the module in `backend-rs/src/services/mod.rs`
    - composes with existing Rust candidate runtime-state and candidate-output
      contracts
  - behavior contract preserved:
    - `max_candidates` normalizes to at least one
    - first candidate uses base prompt/temperature
    - retry candidates append prompt and strategy suffixes with a blank line
    - retry temperature overrides generation kwargs only when representable as
      a JSON number
    - candidate runtime-state sync keeps `single_pass` / `rerank_retry` and
      `initial_candidate` / `rerank_candidate` labels
    - best-candidate selector falls back to the last produced candidate
  - rollback / remaining Python boundary:
    - production candidate execution still uses Python executor/generation
      wiring
    - if the staged Rust owner regresses, remove its module registration and
      keep the Python active path unchanged
    - do not count this as Python active-path retirement until a Rust
      candidate executor consumes `generate_candidate_pool_workflow(...)`

  implementation notes:

  - callback dependencies use owned `Value` / `Vec<Value>` inputs for
    quality-gate, quality-metrics, retry, and selection payloads. This avoids
    higher-ranked lifetime problems with async tests and future executor
    adapters while keeping the JSON candidate boundary explicit.
  - the file currently has `#![allow(dead_code)]` because it is staged for the
    next executor cutover package.

  focused validation passed with:

  - `cargo fmt --manifest-path "backend-rs/Cargo.toml"`
  - `cargo test chapter_candidate_generation_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/mumunovel-candidate-generation-owner" -- --nocapture`
  - `cargo test chapter_candidate_output_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/mumunovel-candidate-generation-owner" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/mumunovel-candidate-generation-owner"`

  next package entrypoint:

  - migrate the Rust candidate executor / wiring function group so production
    candidate execution consumes:
    - `collect_generation_candidate_output(...)`
    - `generate_candidate_pool_workflow(...)`
  - only after that cutover, freeze or remove the Python
    `chapter_candidate_generation_service.py` active path.

- 2026-06-08 chapter-candidate-output concrete Rust request owner checkpoint:
  this round stayed inside the shared `chapter_generation` candidate/text owner
  package and completed the concrete Rust request entry for Python
  `chapter_candidate_output_service.py`. The previous checkpoint had a real
  production consumer for the lower-level stream collector; this checkpoint
  moves the consumer up to `AIService + prompt/tools + runtime_state` request
  ownership so production code no longer hand-builds the provider stream before
  entering the candidate-output owner.

  package map for this checkpoint:
  - Python source map:
    - `backend/app/services/chapter_candidate_output_service.py`
    - `backend/app/services/chapter_candidate_runtime_state_service.py`
    - `backend/app/services/compat/chapter_generation_route_compat_service.py`
  - Rust target map:
    - `backend-rs/src/services/chapter_candidate_output_service.rs`
    - `backend-rs/src/services/chapter_regeneration_stream_launch_service.rs`

  behavior contract kept stable:
  - `ChapterCandidateOutputRequest` now carries `AIService`, prompt,
    optional system prompt, optional tools, candidate index, optional
    max-output chars, and optional runtime-state reference
  - production callers with an `AIService` consume
    `collect_generation_candidate_output(...)`
  - `collect_generation_candidate_output_from_stream(...)` remains the
    lower-level stream reuse/test hook and continues to own chunk aggregation,
    runtime-state sync, stream error propagation, and sentence-boundary
    truncation
  - regeneration stream still owns SSE event construction and error event
    shape; it now delegates only AI stream creation/output collection to the
    candidate-output owner
  - no HTTP payload, SSE payload shape, task lifecycle, provider defaults, or
    Python fallback shell changed in this checkpoint

  implementation boundary for this checkpoint:
  - added `ChapterCandidateOutputRequest` and
    `collect_generation_candidate_output(...)` to
    `chapter_candidate_output_service.rs`
  - changed `chapter_regeneration_stream_launch_service.rs` to call the
    concrete candidate-output owner instead of directly calling
    `AIService::generate_text_stream(...)` and then the lower-level stream
    collector
  - kept the lower-level stream collector as the tested implementation core;
    no unconsumed facade was introduced

  validation passed with:
  - `cargo fmt --manifest-path "backend-rs/Cargo.toml"`
  - `cargo test chapter_candidate_output_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/mumunovel-candidate-output-owner" -- --nocapture`
  - `cargo test chapter_regeneration_stream_launch_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/mumunovel-candidate-output-owner" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/mumunovel-candidate-output-owner"`
  - `git diff --check -- <touched files>` passed with only the existing
    CRLF/LF notice for
    `docs/architecture/rust-strangler-refactor-plan-2026-05-17.zh-CN.md`

  rollback boundary after this change:
  - restore `chapter_regeneration_stream_launch_service.rs` to call
    `AIService::generate_text_stream(...)` directly and pass the stream into
    `collect_generation_candidate_output_from_stream(...)`
  - keep the Rust candidate-output service file if other callers have been
    added; otherwise the concrete request wrapper can be removed without
    touching Python fallback

  next package entrypoint:
  - continue toward a real Rust candidate executor/generation function group
    that can consume `collect_generation_candidate_output(...)` directly
  - defer `chapter_candidate_record_service.py` until the Rust quality-gate /
    candidate-selection metadata owner is explicit

- 2026-06-08 chapter-candidate-output Rust stream owner checkpoint:
  this round continued the shared `chapter_generation` candidate/text owner
  package and moved the Python candidate output collection contract into a
  Rust stream-output owner with a real production consumer. The package did
  not start from Python fallback cleanup; Python files remain source map only.

  package map for this checkpoint:
  - Python source map:
    - `backend/app/services/chapter_candidate_output_service.py`
    - `backend/app/services/chapter_candidate_runtime_state_service.py`
    - `backend/app/services/chapter_generated_text_service.py`
    - `backend/app/services/compat/chapter_generation_route_compat_service.py`
  - Rust target map:
    - `backend-rs/src/services/chapter_candidate_output_service.rs`
    - `backend-rs/src/services/chapter_candidate_runtime_state_service.rs`
    - `backend-rs/src/services/chapter_narrative_cleaner_service.rs`
    - `backend-rs/src/services/chapter_regeneration_stream_launch_service.rs`
    - `backend-rs/src/services/mod.rs`

  behavior contract kept stable or made more Python-compatible:
  - stream output aggregation now has a Rust owner for full-content assembly,
    chunk list preservation, stream error propagation, optional max-output
    truncation, and candidate runtime-state progress sync
  - candidate indexes are clamped to at least `1`, matching Python candidate
    output behavior
  - runtime-state sync updates `candidate_index`, `candidate_total`,
    `candidate_count`, `current_chars`, `word_count`, and `chunk_count`
    through the existing Rust candidate runtime-state owner
  - max-output truncation uses the Rust sentence-boundary cleaner with Unicode
    character counting, not UTF-8 byte length
  - regeneration stream now consumes the Rust stream-output owner for chunk
    collection while preserving its existing SSE event construction, finalize
    behavior, and error payload shape
  - no HTTP payload, route wiring, task lifecycle, provider defaults, or
    Python fallback shell changed in this checkpoint

  implementation boundary for this checkpoint:
  - added `backend-rs/src/services/chapter_candidate_output_service.rs` as the
    Rust owner for Python `chapter_candidate_output_service.py` stream-output
    semantics
  - wired `chapter_regeneration_stream_launch_service.rs` to consume
    `collect_generation_candidate_output_from_stream(...)` as a real
    production path, so the owner is not an unconsumed helper
  - removed the local `allow(dead_code)` staging from
    `trim_text_to_sentence_boundary(...)` and
    `trim_text_to_sentence_boundary_with_lookback(...)` because the trim owner
    is now consumed by Rust stream-output collection
  - intentionally did not add an unconsumed concrete `AIService` facade for
    candidate generation; the next real candidate-executor migration should
    add that wrapper only when it is wired to a production caller

  validation passed with:
  - `cargo fmt --manifest-path "backend-rs/Cargo.toml"`
  - `cargo test chapter_candidate_output_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/mumunovel-candidate-output-owner" -- --nocapture`
  - `cargo test chapter_narrative_cleaner_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/mumunovel-candidate-output-owner" -- --nocapture`
  - `cargo test chapter_regeneration_stream_launch_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/mumunovel-candidate-output-owner" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/mumunovel-candidate-output-owner"`
  - `git diff --check -- <touched files>` passed with only the existing
    CRLF/LF notice for
    `docs/architecture/rust-strangler-refactor-plan-2026-05-17.zh-CN.md`

  rollback boundary after this change:
  - restore `chapter_regeneration_stream_launch_service.rs` to its previous
    local `while let Some(chunk)` aggregation loop
  - remove the `chapter_candidate_output_service` module registration if no
    other Rust caller has been added
  - keep `chapter_narrative_cleaner_service.rs` sanitizer owners intact; only
    the stream-output trim consumption would be rolled back
  - Python fallback requires no rollback because Python source files were not
    changed

  next package entrypoint:
  - continue the shared `chapter_generation` candidate/text owner package by
    migrating the next whole function group from
    `chapter_candidate_record_service.py` only after Rust quality-selection /
    quality-gate helper ownership is identified
  - alternatively migrate a Rust candidate-executor function group that can
    consume `collect_generation_candidate_output_from_stream(...)` directly,
    making the Python candidate-output fallback freeze/repoint step explicit

- 2026-06-08 chapter-generated-text Rust narrative-cleaner owner checkpoint:
  this round selected a shared `chapter_generation` text-cleaning owner package
  instead of another Python wrapper move. The Python source map was
  `backend/app/services/chapter_generated_text_service.py`; the Rust owner was
  `backend-rs/src/services/chapter_narrative_cleaner_service.rs`, which was
  already consumed by draft/regeneration paths and is now also consumed by the
  single-chapter generation runtime.

  package map for this checkpoint:
  - Python source map:
    - `backend/app/services/chapter_generated_text_service.py`
    - `backend/app/services/chapter_candidate_output_service.py`
    - `backend/app/services/chapter_candidate_record_service.py`
    - `backend/app/services/compat/chapter_generation_route_compat_service.py`
  - Rust target map:
    - `backend-rs/src/services/chapter_narrative_cleaner_service.rs`
    - `backend-rs/src/services/chapter_generation_runtime_service.rs`

  behavior contract kept stable or made more Python-compatible:
  - Rust narrative cleaner still removes workflow/meta lines before generated
    narrative text is persisted or applied
  - Rust now owns the Python sentence-boundary hard-limit helper as a staged
    owner for the next candidate-output migration entrypoint
  - single-chapter Rust runtime no longer only trims the AI response; it now
    sanitizes generated narrative text through the Rust cleaner before building
    `GeneratedChapterResult`
  - meta-only output is rejected as an empty narrative after sanitization,
    matching the Python candidate-record guardrail instead of persisting
    process text into chapter content
  - no HTTP payload, SSE payload, route wiring, task lifecycle, provider
    defaults, or Python fallback shell changed in this checkpoint

  implementation boundary for this checkpoint:
  - added Rust `trim_text_to_sentence_boundary(...)` and
    `trim_text_to_sentence_boundary_with_lookback(...)` with Python-style
    Unicode character counting and sentence-boundary fallback behavior
  - kept the trim helpers behind local `allow(dead_code)` because the current
    production consumer is the sanitizer; the trim helper is staged for the
    next candidate-output Rust owner and must not be counted alone as cutover
  - changed `ChapterGenerationRuntimeContext::build_generated_result(...)` to
    return `Result<GeneratedChapterResult, String>` so cleaner failures can
    stop the runtime before persistence
  - added focused tests for normal generated text, meta-prefix cleanup,
    meta-only rejection, Unicode hard-limit trimming, boundary selection, and
    fallback punctuation insertion

  validation passed with:
  - `cargo fmt --manifest-path "backend-rs/Cargo.toml"`
  - `cargo test chapter_narrative_cleaner_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/mumunovel-narrative-cleaner-owner" -- --nocapture`
  - `cargo test chapter_generation_runtime_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/mumunovel-narrative-cleaner-owner" -- --nocapture`
  - `cargo test single_generation_stream --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/mumunovel-narrative-cleaner-owner" -- --nocapture`
  - `cargo test single_generation_background_workflow --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/mumunovel-narrative-cleaner-owner" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/mumunovel-narrative-cleaner-owner"`

  rollback boundary after this change:
  - remove the `chapter_generation_runtime_service.rs` sanitizer call and
    restore `build_generated_result(...) -> GeneratedChapterResult` with the
    previous trim-only behavior
  - leave `chapter_narrative_cleaner_service.rs` existing sanitizer consumers
    intact, because draft/regeneration already depended on that owner before
    this checkpoint
  - no Python fallback rollback is needed because Python source files were not
    changed

  next package entrypoint:
  - continue the shared `chapter_generation` candidate/text owner package by
    wiring the staged sentence-boundary trim owner into a Rust candidate-output
    implementation only when a real Rust candidate output consumer exists
  - do not report the staged trim helper as complete migration until it is
    consumed by a Rust production path

- 2026-06-07 single-generation route request owner-collapse checkpoint:
  after the candidate runtime-state Rust owner landed, this round stayed in
  `backend-rs` and collapsed the remaining public helper seam for
  `route payload -> SingleChapterGenerationRequest`. The route DTO now owns
  conversion through `SingleChapterGenerationRouteRequest::into_generation_request()`,
  and the stream/background owners consume that directly.

  package map for this checkpoint:
  - Python source map:
    - `backend/app/services/compat/chapter_generation_route_compat_service.py`
    - `backend/app/api/chapter_generation_routes.py`
  - Rust target map:
    - `backend-rs/src/api/chapter_generation_routes.rs`
    - `backend-rs/src/services/chapter_single_generation_prepare_service.rs`
    - `backend-rs/src/services/chapter_single_generation_stream_entry_service.rs`
    - `backend-rs/src/services/chapter_single_generation_write_workflow_service.rs`

  behavior contract kept stable:
  - HTTP route payload shape and strict unknown-field behavior stay unchanged
  - Python-compatible flag defaults still resolve in the internal request owner
  - stream entry still owns `route payload -> runtime launch input -> SSE stream`
  - background write workflow still owns
    `route payload -> existing task payload or launch -> persist/dispatch`
  - no Python fallback route wiring changed in this checkpoint

  implementation boundary for this checkpoint:
  - removed the standalone
    `build_single_chapter_generation_request_from_route_payload(...)` helper
    seam from the prepare service public surface
  - added `SingleChapterGenerationRouteRequest::into_generation_request()`
    beside the route DTO so the compatibility normalization stays with the
    transport payload owner
  - updated stream/background owners and focused tests to consume the route DTO
    conversion directly
  - route tests now assert route payload shape only, instead of crossing into
    internal request construction

  validation passed with:
  - `rg -n "build_single_chapter_generation_request_from_route_payload" "backend-rs/src"`
    -> no matches
  - `cargo fmt --manifest-path "backend-rs/Cargo.toml"`
  - `cargo test single_chapter_generation_route --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/mumunovel-candidate-runtime" -- --nocapture`
  - `cargo test single_generation_stream --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/mumunovel-candidate-runtime" -- --nocapture`
  - `cargo test single_generation_background_workflow --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/mumunovel-candidate-runtime" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/mumunovel-candidate-runtime"`

  rollback boundary after this change:
  - restore the standalone helper and repoint stream/background owners back to
    it if an external Rust caller unexpectedly depended on the old helper
    surface
  - no Python fallback change is needed for rollback because the Python source
    map was untouched

  next package entrypoint:
  - continue the single-generation Rust owner package by looking for function
    groups that still expose neighboring wrapper seams around startup snapshot,
    existing-background read state, or stream success projection
  - do not shrink `chapter_generation_route_compat_service.py` until the
    matching Rust owner has production consumption and focused validation

- 2026-06-07 Rust candidate runtime-state owner checkpoint:
  this round returned to the Rust-first lane after the re-plan. Instead of
  moving another Python compatibility wrapper, it ported the pure candidate
  runtime-state semantics from Python into a Rust owner and wired one existing
  Rust payload projection path to consume that owner.

  package map for this checkpoint:
  - Python source map:
    - `backend/app/services/chapter_candidate_runtime_state_service.py`
    - `backend/app/services/chapter_candidate_generation_service.py`
    - `backend/app/services/compat/chapter_generation_route_compat_service.py`
  - Rust target map:
    - `backend-rs/src/services/chapter_candidate_runtime_state_service.rs`
    - `backend-rs/src/services/chapter_batch_generation_task_payload_base_service.rs`
    - `backend-rs/src/services/mod.rs`

  behavior contract kept stable:
  - candidate attempt labels remain:
    - `1 -> ("single_pass", "initial_candidate")`
    - `>1 -> ("rerank_retry", "rerank_candidate")`
    - word-budget repair -> `("word_budget_repair", "word_budget_repair")`
  - candidate runtime-state defaults remain compatible with Python:
    `candidate_total`, `candidate_count`, `candidate_index`, `current_chars`,
    `word_count`, `chunk_count`, `generation_path`, `attempt_kind`,
    `rerank_used`, `word_budget_repair_used`, and `winner_candidate_index`
  - batch task checkpoint payloads still expose the Python-query diagnostic
    candidate fields, preserving `null` for missing raw fields and `null` for
    non-bool boolean fields
  - no HTTP payload, SSE payload, task lifecycle, AI call behavior, or Python
    fallback route wiring changed in this checkpoint

  implementation boundary for this checkpoint:
  - `chapter_candidate_runtime_state_service.rs` now owns the Rust equivalent
    of Python candidate runtime-state helpers:
    - attempt-label resolution
    - default runtime-state construction
    - runtime-state snapshot normalization
    - runtime-state sync mutation
    - Python-query checkpoint candidate field insertion
  - `chapter_batch_generation_task_payload_base_service.rs` now delegates
    candidate diagnostic field insertion to that Rust owner instead of carrying
    the candidate field list locally
  - a local `allow(dead_code)` is intentionally scoped to the new staged owner
    because only the checkpoint inserter is consumed immediately; the remaining
    pure helpers are the next candidate-executor migration target

  validation passed with:
  - `cargo fmt --manifest-path "backend-rs/Cargo.toml"`
  - `cargo test candidate_runtime_state --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/mumunovel-candidate-runtime" -- --nocapture`
  - `cargo test python_query_snapshot --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/mumunovel-candidate-runtime" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/mumunovel-candidate-runtime"`

  rollback boundary after this change:
  - remove the `chapter_candidate_runtime_state_service` module registration
    and inline the previous candidate diagnostic field insertion constants back
    into `chapter_batch_generation_task_payload_base_service.rs`
  - Python source files were not changed in this checkpoint, so the existing
    Python candidate runtime-state fallback remains intact

  next package entrypoint:
  - continue the shared `chapter_generation` candidate owner package by moving
    the next pure candidate function group from Python to Rust:
    `resolve_generation_attempt_labels -> sync runtime state -> candidate
    record metadata`
  - only after those Rust owners are consumed by stream/background or batch
    candidate flow should the surviving Python wrappers in
    `chapter_generation_route_compat_service.py` be shrunk

- 2026-06-07 Rust-first re-plan checkpoint:
  this checkpoint corrects the execution strategy after the latest route compat
  cleanup. The Python route compat retirements were valid fallback shrink work,
  but the next implementation round must return to real Rust owner migration.

  revised current state:
  - `backend/app/services/compat/` has only one substantive remaining compat
    owner:
    `backend/app/services/compat/chapter_generation_route_compat_service.py`
  - that file should be treated as the Python source map for the next Rust
    package, not as a Python-only relocation target
  - the next package starts in `backend-rs`, with Python changes reserved for
    fallback shrink after Rust validation

  revised next package:
  - primary:
    `chapter_single_generation` Rust owner package
  - secondary:
    shared `chapter_generation` owner package
  - follow-up:
    `chapters` compatibility shell shrink, including the surviving generation
    compat shell, only after the matching Rust owner is validated

  required validation shape:
  - focused Rust tests for the owner file or module being changed
  - `cargo check --manifest-path "backend-rs/Cargo.toml"` with an explicit
    target dir when needed
  - targeted Python API tests only when Python fallback or route wiring is
    repointed after the Rust owner is ready

  progress reporting rule:
  - report Rust owner completion separately from Python fallback shrink
  - do not count a Python-only wrapper relocation as primary migration progress
    unless it directly follows validated Rust ownership and removes an active
    fallback dependency

- 2026-06-07 partial-regeneration and regeneration route compat owner retirement checkpoint:
  this round stayed on the Python shell-compression path and retired two more
  surviving route compat owner files after both routes had become their only
  real production consumers. Unlike the earlier top-level shim cleanup, this
  package removed active compat owners from the production path and moved the
  default wiring plus monkeypatch surface back to the route owners directly.

  package map for this checkpoint:
  - Python source map:
    - `backend/app/api/chapter_partial_regeneration_routes.py`
    - `backend/app/api/chapter_regeneration_routes.py`
    - `backend/tests/test_api/chapters_test_support.py`
    - `backend/tests/test_api/test_chapters.py`
    - `backend/tests/test_api/test_chapters_stream_routes.py`
  - Rust target map:
    - `backend-rs/src/api/chapter_regeneration_routes.rs`
    - `backend-rs/src/services/chapter_regeneration_prepare_service.rs`
    - `backend-rs/src/services/chapter_regeneration_apply_service.rs`
    - `backend-rs/src/services/chapter_single_generation_stream_workflow_service.rs`
    - `backend-rs/src/services/chapter_single_generation_write_workflow_service.rs`

  behavior contract kept stable:
  - the same regeneration routes remain reachable:
    - `POST /chapters/{chapter_id}/partial-regenerate-stream`
    - `POST /chapters/{chapter_id}/apply-partial-regenerate`
    - `POST /chapters/{chapter_id}/regenerate-stream`
    - `GET /chapters/{chapter_id}/regeneration/tasks`
  - partial-regeneration SSE flow still keeps the same progress/result shell,
    generated-text sanitation, workflow-meta rejection, and chapter-content
    apply semantics
  - regeneration SSE flow still keeps the same stream payload shape, context
    preparation, generated-text sanitation, and `REGENERATOR_FACTORY` patch
    semantics used by focused API tests
  - no HTTP payload shell, SSE event ordering, task lifecycle, or error
    semantics were intentionally changed in this slice

  implementation boundary for this checkpoint:
  - `backend/app/api/chapter_partial_regeneration_routes.py` now owns:
    - `normalize_partial_regeneration_output(...)`
    - `partial_regenerate_stream_with_default_route_wiring(...)`
    - `apply_partial_regenerate_with_default_route_wiring(...)`
  - `backend/app/api/chapter_regeneration_routes.py` now owns:
    - `REGENERATOR_FACTORY`
    - `regenerate_chapter_stream_with_default_route_wiring(...)`
  - shared API test support and focused API tests now patch the route modules
    directly instead of deleted compat owners:
    - `backend/tests/test_api/chapters_test_support.py`
    - `backend/tests/test_api/test_chapters.py`
    - `backend/tests/test_api/test_chapters_stream_routes.py`
  - deleted Python files:
    - `backend/app/services/compat/chapter_partial_regeneration_route_compat_service.py`
    - `backend/app/services/compat/chapter_regeneration_route_compat_service.py`

  this is counted as real migration progress because two more Python compat
  owner files fully left the active code path:
  - no production import path remains on either deleted compat owner
  - no repo test import path remains on either deleted compat owner
  - the regeneration boundary is now:
    `route owner -> real regeneration/context/apply owners`
    instead of
    `route owner -> route compat owner -> real owners`

  validation passed with:
  - `rg -n --glob "*.py" "chapter_regeneration_route_compat_service|chapter_partial_regeneration_route_compat_service" backend/app backend/tests`
    -> no active code matches
  - `python -c "import sys; sys.path.insert(0, 'backend'); from app.api import chapter_partial_regeneration_routes, chapter_regeneration_routes; print('ok')"`
  - `python -m pytest backend/tests/test_api/test_chapters_stream_routes.py -q -k "partial_regenerate or regenerate"`
  - `python -m pytest backend/tests/test_api/test_chapters.py -q -k "regenerate"`

  rollback boundary after this change:
  - rollback is file-group only:
    - restore the two deleted compat owner files if any external import surface
      outside this repo still depends on those historical module paths
  - no route payload shell, SSE ordering, or regeneration workflow semantics
    changed in this slice
  - after the Rust-first re-plan, the surviving
    `backend/app/services/compat/chapter_generation_route_compat_service.py`
    owner shell is treated as the Python source map for the next Rust package,
    not as another Python-only relocation target

- 2026-06-07 analysis-task, analysis, annotation, and expansion-plan route compat owner retirement checkpoint:
  this round stayed on the Python shell-compression path and retired four more
  surviving route compat owner files once each route module had become the
  only real production consumer. The default wiring and monkeypatch surfaces
  were moved back to the route owners directly instead of preserving one more
  helper hop.

  package map for this checkpoint:
  - Python source map:
    - `backend/app/api/chapter_analysis_task_routes.py`
    - `backend/app/api/chapter_analysis_routes.py`
    - `backend/app/api/chapter_annotation_routes.py`
    - `backend/app/api/chapter_expansion_plan_routes.py`
    - `backend/tests/test_api/chapters_test_support.py`
    - `backend/tests/test_api/test_chapters.py`
    - `backend/tests/test_api/test_chapters_analysis.py`
    - `backend/tests/test_api/test_chapters_quality_views.py`
    - `backend/tests/test_api/test_chapters_stream_routes.py`
  - Rust target map:
    - `backend-rs/src/api/chapter_crud_routes.rs`
    - `backend-rs/src/api/chapter_generation_routes.rs`
    - `backend-rs/src/api/chapter_regeneration_routes.rs`
    - `backend-rs/src/services/chapter_single_generation_write_workflow_service.rs`
    - `backend-rs/src/services/chapter_single_generation_stream_workflow_service.rs`

  behavior contract kept stable:
  - the same analysis-task routes remain reachable:
    - `GET /chapters/{chapter_id}/analysis/status`
    - `POST /chapters/analysis/status/batch`
    - `GET /chapters/{chapter_id}/can-generate`
    - `POST /chapters/{chapter_id}/analyze`
  - the same analysis / annotation / expansion-plan routes remain reachable:
    - `GET /chapters/{chapter_id}/analysis`
    - `GET /chapters/{chapter_id}/annotations`
    - `PUT /chapters/{chapter_id}/expansion-plan`
  - route-local monkeypatch surfaces remain reachable on the surviving route
    owners, especially `execute_chapter_analysis_background`
  - no HTTP payload shell, task lifecycle, or error semantics were
    intentionally changed in this slice

  implementation boundary for this checkpoint:
  - `backend/app/api/chapter_analysis_task_routes.py` now owns:
    - `execute_chapter_analysis_background`
    - `get_analysis_task_status_with_default_route_wiring(...)`
    - `get_batch_analysis_task_status_with_default_route_wiring(...)`
    - `check_can_generate_with_default_route_wiring(...)`
    - `trigger_chapter_analysis_with_default_route_wiring(...)`
  - `backend/app/api/chapter_analysis_routes.py` now owns:
    - `get_chapter_analysis_with_default_route_wiring(...)`
  - `backend/app/api/chapter_annotation_routes.py` now owns:
    - `get_chapter_annotations_with_default_route_wiring(...)`
  - `backend/app/api/chapter_expansion_plan_routes.py` now owns:
    - `update_chapter_expansion_plan_with_default_route_wiring(...)`
  - focused API tests now patch the route modules directly instead of deleted
    compat owners
  - deleted Python files:
    - `backend/app/services/compat/chapter_analysis_task_route_compat_service.py`
    - `backend/app/services/compat/chapter_analysis_route_compat_service.py`
    - `backend/app/services/compat/chapter_annotation_route_compat_service.py`
    - `backend/app/services/compat/chapter_expansion_plan_route_compat_service.py`

  this is counted as real migration progress because four more Python compat
  owner files fully left the active code path:
  - no production import path remains on any deleted compat owner
  - no repo test import path remains on any deleted compat owner
  - the route boundary is now:
    `route owner -> real analysis/annotation/expansion-plan owners`
    instead of
    `route owner -> route compat owner -> real owners`

  validation passed with:
  - `rg -n --glob "*.py" "chapter_analysis_task_route_compat_service|chapter_analysis_route_compat_service|chapter_annotation_route_compat_service|chapter_expansion_plan_route_compat_service" backend/app backend/tests`
    -> no active code matches
  - `python -c "import sys; sys.path.insert(0, 'backend'); from app.api import chapters; from app.api import chapter_analysis_task_routes, chapter_analysis_routes, chapter_annotation_routes, chapter_expansion_plan_routes; print('ok')"`
  - `python -m pytest backend/tests/test_api/test_chapters_analysis.py -q`
  - `python -m pytest backend/tests/test_api/test_chapters.py -q -k "delegate_analysis_route or delegate_annotation_route or delegate_expansion_plan_route or update_chapter_expansion_plan or return_chapter_annotations_with_analysis_metadata or return_analysis_checker_and_auto_revision_payloads"`
  - `python -m pytest backend/tests/test_api/test_chapters_quality_views.py -q -k "annotations"`

  rollback boundary after this change:
  - rollback is file-group only:
    - restore the four deleted compat owner files if any external import
      surface outside this repo still depends on those historical module paths
  - no route payload shell, analysis workflow semantics, annotation payload
    semantics, or expansion-plan update semantics changed in this slice

- 2026-06-07 batch-run and task-workflow runtime compat owner retirement checkpoint:
  this round stayed on the Python shell-compression path and retired two more
  runtime-helper compat owner files around the `chapters.py` batch generation
  path. Unlike pure forwarding helpers, this package moved the remaining small
  runtime behavior back to real service owners before deleting the compat
  files.

  package map for this checkpoint:
  - Python source map:
    - `backend/app/api/chapters.py`
    - `backend/app/services/batch_generation_run_service.py`
    - `backend/app/services/task_workflow_runtime_service.py`
    - `backend/app/services/task_quality_snapshot_service.py`
    - `backend/tests/test_services/test_batch_generation_run_service.py`
    - `backend/tests/test_services/test_task_workflow_runtime_service.py`
    - `backend/tests/test_services/test_task_state_store.py`
    - `backend/tests/test_api/test_chapters.py`
  - Rust target map:
    - `backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs`
    - `backend-rs/src/services/chapter_batch_generation_write_workflow_service.rs`
    - `backend-rs/src/services/chapter_batch_generation_read_context_service.rs`
    - `backend-rs/src/services/chapter_generation_task_semantics_service.rs`

  behavior contract kept stable:
  - cancelable batch-generation waiting still shields the generation task,
    polls task status on timeout, and raises `asyncio.CancelledError` when the
    persisted task is marked `cancelled`
  - batch write-lock lookup still uses the chapter-analysis write-lock owner
  - task runtime cache clearing still clears both quality-metrics cache and
    workflow-runtime cache for the same task id
  - persisted task workflow snapshot helpers, `SNAPSHOT_UNSET`, and task
    existence checks keep the same public behavior
  - no batch lifecycle status, snapshot payload, runtime checkpoint, or API
    response shell was intentionally changed in this slice

  implementation boundary for this checkpoint:
  - `await_cancelable_batch_generation_result(...)` and `get_db_write_lock(...)`
    moved to the real owner
    `backend/app/services/batch_generation_run_service.py`
  - `clear_task_runtime_caches(...)` moved to the real owner
    `backend/app/services/task_workflow_runtime_service.py`
  - `clear_task_runtime_caches(...)` uses a local lazy import for
    `task_quality_snapshot_service` so the existing
    `task_quality_snapshot_service -> task_workflow_runtime_service` snapshot
    dependency does not become a circular import
  - `backend/app/api/chapters.py` now imports the batch-run and task-workflow
    runtime helpers directly from the real owners
  - focused compat-only coverage moved to real owner tests:
    - `backend/tests/test_services/test_batch_generation_run_service.py`
    - `backend/tests/test_services/test_task_workflow_runtime_service.py`
  - deleted Python files:
    - `backend/app/services/compat/batch_generation_run_compat_service.py`
    - `backend/app/services/compat/task_workflow_runtime_compat_service.py`
    - `backend/tests/test_services/test_batch_generation_run_compat_service.py`
    - `backend/tests/test_services/test_task_workflow_runtime_compat_service.py`

  this is counted as real migration progress because two more Python compat
  owner files fully left the active code path:
  - no production import path remains on either deleted compat owner
  - no repo test import path remains on either deleted compat owner
  - the batch runtime helper boundary is now:
    `api / batch workflow -> real batch-run / task-workflow owners`
    instead of
    `api / batch workflow -> runtime compat owners -> real owners`

  validation passed with:
  - `rg -n "batch_generation_run_compat_service|task_workflow_runtime_compat_service" backend/app backend/tests backend/app/services/CLAUDE.md`
    -> no active code matches
  - `python -c "import sys; sys.path.insert(0, 'backend'); from app.api import chapters; from app.services import batch_generation_run_service, task_workflow_runtime_service; print('ok')"`
  - `python -m pytest backend/tests/test_services/test_batch_generation_run_service.py backend/tests/test_services/test_task_workflow_runtime_service.py backend/tests/test_services/test_task_state_store.py -q`
  - `python -m pytest backend/tests/test_api/test_chapters.py -q -k "snapshot or runtime or cancelable or batch"`

  rollback boundary after this change:
  - rollback is file-group only:
    - restore the two deleted compat owner files if any external import surface
      outside this repo still depends on those historical module paths
  - no batch task lifecycle, task stream snapshot, or cancellation behavior
    changed in this slice
  - next Python fallback target in this neighborhood should move to a larger
    route/workflow owner rather than another helper facade

- 2026-06-07 generated-text and prompt-quality compat owner retirement checkpoint:
  this round stayed on the Python shell-compression path and retired two more
  surviving helper compat owner files around the `chapters.py` generation
  path. It deliberately chose a grouped whole-file package where real service
  owners already existed and the remaining compat files only replayed helper
  calls.

  package map for this checkpoint:
  - Python source map:
    - `backend/app/api/chapters.py`
    - `backend/app/services/chapter_generated_text_service.py`
    - `backend/app/services/chapter_generation/runtime/prompt_service.py`
    - `backend/app/services/story_quality_feedback_service.py`
    - `backend/app/services/compat/chapter_generation_route_compat_service.py`
    - `backend/tests/test_services/test_chapter_generated_text_service.py`
    - `backend/tests/test_services/test_chapter_generation_runtime_prompt_service.py`
    - `backend/tests/test_services/test_story_quality_feedback_service.py`
    - `backend/tests/test_api/test_chapters_stream_routes.py`
    - `backend/tests/test_api/test_chapters_batch_generation.py`
    - `backend/tests/test_api/test_chapters_candidate_rerank.py`
  - Rust target map:
    - `backend-rs/src/services/chapter_single_generation_write_workflow_service.rs`
    - `backend-rs/src/services/chapter_single_generation_stream_workflow_service.rs`
    - `backend-rs/src/services/chapter_generation_execution_contract_service.rs`
    - `backend-rs/src/services/chapter_generation_quality_runtime_context_service.rs`

  behavior contract kept stable:
  - generated text sanitation keeps the same meta-text filtering, sentence
    trimming, and light template-polish behavior
  - runtime prompt construction keeps the same style-profile, temperature,
    organization-ledger, web-research grounding, and word-budget guardrails
  - story-quality metric computation still resolves through the existing real
    `story_quality_feedback_service` owner
  - `chapters.py` and the surviving route compat owner keep the same patchable
    helper names used by focused API tests
  - no HTTP payload shell, SSE payload shape, prompt contract, or quality
    metric response shape was intentionally changed in this slice

  implementation boundary for this checkpoint:
  - `backend/app/api/chapters.py` now imports generated-text helpers directly
    from `backend/app/services/chapter_generated_text_service.py`
  - `backend/app/api/chapters.py` and
    `backend/app/services/compat/chapter_generation_route_compat_service.py`
    now import runtime prompt helpers directly from
    `backend/app/services/chapter_generation/runtime/prompt_service.py`
  - both modules now import story quality computation directly from
    `backend/app/services/story_quality_feedback_service.py`
  - focused compat-only coverage moved to real owner tests:
    - generated-text meta detection and sanitation moved into
      `backend/tests/test_services/test_chapter_generated_text_service.py`
    - prompt style-profile, temperature, and runtime prompt construction moved
      into
      `backend/tests/test_services/test_chapter_generation_runtime_prompt_service.py`
  - deleted Python files:
    - `backend/app/services/compat/chapter_generated_text_compat_service.py`
    - `backend/app/services/compat/chapter_prompt_quality_compat_service.py`
    - `backend/tests/test_services/test_chapter_generated_text_compat_service.py`
    - `backend/tests/test_services/test_chapter_prompt_quality_compat_service.py`

  this is counted as real migration progress because two more Python compat
  owner files fully left the active code path:
  - no production import path remains on either deleted compat owner
  - no repo test import path remains on either deleted compat owner
  - the generated-text and prompt-quality helper boundary is now:
    `api / route compat -> real text / prompt / quality owners`
    instead of
    `api / route compat -> helper compat owners -> real owners`

  validation passed with:
  - `rg -n "chapter_generated_text_compat_service|chapter_prompt_quality_compat_service" backend/app backend/tests`
    -> no active code matches
  - `python -c "import sys; sys.path.insert(0, 'backend'); from app.api import chapters, chapter_generation_routes; from app.services.compat import chapter_generation_route_compat_service; from app.services import chapter_generated_text_service; from app.services.chapter_generation.runtime import prompt_service; print('ok')"`
  - `python -m pytest backend/tests/test_services/test_chapter_generated_text_service.py backend/tests/test_services/test_chapter_generation_runtime_prompt_service.py backend/tests/test_services/test_story_quality_feedback_service.py -q`
  - `python -m pytest backend/tests/test_api/test_chapters_stream_routes.py backend/tests/test_api/test_chapters_batch_generation.py backend/tests/test_api/test_chapters_candidate_rerank.py -q`

  rollback boundary after this change:
  - rollback is file-group only:
    - restore the two deleted compat owner files if any external import surface
      outside this repo still depends on those historical module paths
  - no stream payload shell, prompt construction semantics, or story-quality
    metric semantics changed in this slice
  - next Python fallback target in this neighborhood should remain a larger
    active owner, especially `chapter_generation_route_compat_service.py`, or
    the next whole-file `chapter_single_generation` package

- 2026-06-07 chapter-candidate executor compat owner retirement checkpoint:
  this round stayed on the Python shell-compression path and retired one more
  surviving compat owner file around the `chapters.py` candidate-rerank /
  stream path instead of reopening a standalone Rust seam.

  package map for this checkpoint:
  - Python source map:
    - `backend/app/api/chapters.py`
    - `backend/app/services/chapter_candidate_generation_service.py`
    - `backend/app/services/chapter_candidate_output_service.py`
    - `backend/app/services/chapter_candidate_record_service.py`
    - `backend/app/services/chapter_candidate_runtime_state_service.py`
    - `backend/app/services/chapter_candidate_executor_service.py`
    - `backend/app/services/chapter_candidate_executor_wiring_service.py`
    - `backend/app/services/compat/chapter_generation_route_compat_service.py`
    - `backend/tests/test_services/test_chapter_candidate_generation_service.py`
    - `backend/tests/test_services/test_chapter_candidate_output_service.py`
    - `backend/tests/test_services/test_chapter_candidate_record_service.py`
    - `backend/tests/test_services/test_chapter_candidate_runtime_state_service.py`
    - `backend/tests/test_services/test_chapter_candidate_executor_service.py`
    - `backend/tests/test_services/test_chapter_candidate_executor_wiring_service.py`
    - `backend/tests/test_api/test_chapters_candidate_rerank.py`
    - `backend/tests/test_api/test_chapters_stream_routes.py`
  - Rust target map:
    - `backend-rs/src/services/chapter_single_generation_write_workflow_service.rs`
    - `backend-rs/src/services/chapter_single_generation_stream_workflow_service.rs`
    - `backend-rs/src/services/chapter_generation_execution_contract_service.rs`
    - `backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs`

  behavior contract kept stable:
  - the same candidate-rerank workflow entrypoints remain reachable:
    - `chapters.py::_get_chapter_candidate_executor_dependencies()`
    - `chapters.py::_generate_best_ranked_candidate(...)`
    - `chapter_generation_route_compat_service.generate_best_ranked_candidate(...)`
  - the same stream route behavior remains:
    - `POST /chapters/{chapter_id}/generate-stream`
  - the same candidate selection / rerank / targeted-repair semantics remain
  - the same route-local and shared `chapters.py` monkeypatch surfaces remain
    reachable for stream-route tests
  - no SSE payload shell, candidate-selection metadata shape, or quality-gate
    routing semantics were intentionally changed in this slice

  implementation boundary for this checkpoint:
  - `resolve_generation_attempt_labels(...)` moved to the real owner
    `backend/app/services/chapter_candidate_generation_service.py`
  - output collection, record building, and runtime-state sync now resolve
    directly against the real owners:
    - `backend/app/services/chapter_candidate_output_service.py`
    - `backend/app/services/chapter_candidate_record_service.py`
    - `backend/app/services/chapter_candidate_runtime_state_service.py`
  - `backend/app/api/chapters.py` keeps its local wrapper names for patch
    stability, but those wrappers no longer route through a compat owner
  - `backend/app/services/compat/chapter_generation_route_compat_service.py`
    now keeps a route-local patch surface while delegating those helpers
    directly to the same real owners
  - focused coverage for the retired compat-owner-only behaviors moved to real
    owner tests:
    - `backend/tests/test_services/test_chapter_candidate_generation_service.py`
    - `backend/tests/test_services/test_chapter_candidate_executor_wiring_service.py`
  - deleted Python files:
    - `backend/app/services/compat/chapter_candidate_executor_compat_service.py`
    - `backend/tests/test_services/test_chapter_candidate_executor_compat_service.py`

  this is counted as real migration progress because one more surviving Python
  compat owner fully left the active code path instead of only moving imports
  between wrappers:
  - no production import path remains on the deleted compat owner
  - no repo test import path remains on the deleted compat owner
  - the candidate-rerank helper boundary is now:
    `api / route compat -> real helper owners -> executor / wiring / workflow owners`
    instead of
    `api / route compat -> executor compat owner -> real helper owners`

  validation passed with:
  - `rg -n "chapter_candidate_executor_compat_service" backend/app backend/tests`
    -> no matches under active code paths
  - `python -c "import sys; sys.path.insert(0, 'backend'); from app.api import chapters; from app.services import chapter_candidate_executor_service, chapter_candidate_generation_service, chapter_candidate_output_service, chapter_candidate_record_service, chapter_candidate_runtime_state_service; from app.services.compat import chapter_generation_route_compat_service; print('ok')"`
  - `python -m pytest backend/tests/test_services/test_chapter_candidate_generation_service.py backend/tests/test_services/test_chapter_candidate_output_service.py backend/tests/test_services/test_chapter_candidate_record_service.py backend/tests/test_services/test_chapter_candidate_runtime_state_service.py backend/tests/test_services/test_chapter_candidate_executor_service.py backend/tests/test_services/test_chapter_candidate_executor_wiring_service.py -q`
  - `python -m pytest backend/tests/test_api/test_chapters_candidate_rerank.py backend/tests/test_api/test_chapters_stream_routes.py -q`

  rollback boundary after this change:
  - rollback is file-group only:
    - restore `backend/app/services/compat/chapter_candidate_executor_compat_service.py`
      if any external import surface outside this repo still depends on that
      historical module path
  - no stream payload shell, candidate selection semantics, or quality-gate
    branch semantics changed in this slice
  - next Python fallback target in this neighborhood should be the surviving
    `chapter_generation_route_compat_service.py` helper shell or the next
    whole-file `chapter_single_generation` owner package

- 2026-06-07 chapter-candidate entry compat owner retirement checkpoint:
  this round stayed on the Python shell-compression path, but it moved past
  top-level shim cleanup and retired one surviving compat owner file around
  the `chapters.py` candidate-rerank / stream path instead of reopening a
  standalone Rust seam.

  package map for this checkpoint:
  - Python source map:
    - `backend/app/api/chapters.py`
    - `backend/app/services/chapter_candidate_executor_service.py`
    - `backend/app/services/chapter_candidate_executor_wiring_service.py`
    - `backend/app/services/compat/chapter_candidate_executor_compat_service.py`
    - `backend/app/services/compat/chapter_generation_route_compat_service.py`
    - `backend/tests/test_services/test_chapter_candidate_executor_service.py`
    - `backend/tests/test_services/test_chapter_candidate_executor_compat_service.py`
    - `backend/tests/test_api/test_chapters_candidate_rerank.py`
    - `backend/tests/test_api/test_chapters_stream_routes.py`
  - Rust target map:
    - `backend-rs/src/services/chapter_single_generation_write_workflow_service.rs`
    - `backend-rs/src/services/chapter_single_generation_stream_workflow_service.rs`
    - `backend-rs/src/services/chapter_generation_execution_contract_service.rs`
    - `backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs`

  behavior contract kept stable:
  - the same candidate-rerank workflow entrypoints remain reachable:
    - `chapters.py::_get_chapter_candidate_executor_dependencies()`
    - `chapters.py::_generate_best_ranked_candidate(...)`
    - `chapter_generation_route_compat_service.generate_best_ranked_candidate(...)`
  - the same stream route behavior remains:
    - `POST /chapters/{chapter_id}/generate-stream`
  - the same candidate selection / rerank / targeted-repair semantics remain
  - the same stream-route testing monkeypatch surface remains reachable
    through the surviving route compat owner and the shared `chapters.py`
    module object
  - no SSE payload shell, candidate-selection metadata shape, or quality-gate
    routing semantics were intentionally changed in this slice

  implementation boundary for this checkpoint:
  - `generate_best_ranked_candidate(...)` moved to the real owner
    `backend/app/services/chapter_candidate_executor_service.py`
  - cached dependency assembly now also lives on the same real owner boundary:
    `get_chapter_candidate_executor_dependencies(...)`
    with a lazy import into
    `backend/app/services/chapter_candidate_executor_wiring_service.py`
    so the compat owner no longer needs to exist
  - production imports now point directly at the surviving real owners:
    - `backend/app/api/chapters.py`
      -> `app.services.chapter_candidate_executor_service`
    - `backend/app/services/compat/chapter_generation_route_compat_service.py`
      -> `app.services.chapter_candidate_executor_service`
  - the surviving route compat owner now prefers:
    - local monkeypatch overrides when tests patch
      `chapter_generation_route_compat_service.*`
    - otherwise the shared `chapters.py` patch surface for
      `OneToOneContextBuilder`, `OneToManyContextBuilder`,
      `PromptService.get_template`, `PromptService.format_prompt`,
      `compute_story_quality_metrics`, and
      `_resolve_quality_gate_execution_plan`
  - focused service coverage moved from the deleted compat owner test into
    `backend/tests/test_services/test_chapter_candidate_executor_service.py`
  - deleted Python files:
    - `backend/app/services/compat/chapter_candidate_entry_compat_service.py`
    - `backend/tests/test_services/test_chapter_candidate_entry_compat_service.py`

  this is counted as real migration progress because one more surviving Python
  compat owner fully left the active code path instead of only moving imports
  between wrappers:
  - no production import path remains on the deleted compat owner
  - no repo test import path remains on the deleted compat owner
  - the candidate-rerank entry boundary is now
    `api / route compat -> real executor owner -> wiring / workflow owners`
    instead of
    `api / route compat -> entry compat owner -> real executor owner`

  validation passed with:
  - `rg -n "chapter_candidate_entry_compat_service|_generate_best_ranked_candidate_compat_service|_get_chapter_candidate_executor_dependencies_compat_service|_generate_best_ranked_candidate_entry" backend/app backend/tests`
    -> no matches
  - `python -c "import sys; sys.path.insert(0, 'backend'); from app.api import chapters; from app.services import chapter_candidate_executor_service; from app.services.compat import chapter_candidate_executor_compat_service, chapter_generation_route_compat_service; print('ok')"`
  - `python -m pytest backend/tests/test_services/test_chapter_candidate_executor_service.py backend/tests/test_services/test_chapter_candidate_executor_compat_service.py -q`
  - `python -m pytest backend/tests/test_api/test_chapters_candidate_rerank.py -q`
  - `python -m pytest backend/tests/test_api/test_chapters_stream_routes.py -q`

  rollback boundary after this change:
  - rollback is file-group only:
    - restore `backend/app/services/compat/chapter_candidate_entry_compat_service.py`
      if any external import surface outside this repo still depends on that
      historical module path
  - no stream payload shell, candidate selection semantics, or quality-gate
    branch semantics changed in this slice
  - next Python fallback target in this neighborhood should be the surviving
    `backend/app/services/compat/chapter_candidate_executor_compat_service.py`
    owner or the next whole-file `chapter_single_generation` owner package

- 2026-06-07 batch-entry and project-quality top-level compat shim group retirement checkpoint:
  this round stayed on the Python shell-compression path and retired another
  grouped pair of top-level compat shim files around `chapters.py`,
  `chapter_quality_routes.py`, and the surviving batch-generation / project
  quality compat owners instead of reopening a standalone Rust seam.

  package map for this checkpoint:
  - Python source map:
    - `backend/app/api/chapters.py`
    - `backend/app/api/chapter_quality_routes.py`
    - `backend/app/services/compat/batch_generation_entry_compat_service.py`
    - `backend/app/services/compat/project_quality_trend_compat_service.py`
    - `backend/app/services/compat/batch_generation_route_compat_service.py`
    - `backend/app/services/compat/chapter_generation_route_compat_service.py`
    - `backend/tests/test_api/chapters_test_support.py`
    - `backend/tests/test_api/test_chapters_batch_status_resume.py`
    - `backend/tests/test_api/test_chapters_quality_views.py`
    - `backend/tests/test_services/test_batch_generation_entry_compat_service.py`
    - `backend/tests/test_services/test_project_quality_trend_compat_service.py`
  - Rust target map:
    - `backend-rs/src/services/chapter_batch_generation_write_workflow_service.rs`
    - `backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs`
    - `backend-rs/src/services/chapter_single_generation_write_workflow_service.rs`
    - `backend-rs/src/services/chapter_generation_execution_contract_service.rs`

  behavior contract kept stable:
  - same batch-generation entry helpers remain reachable on the surviving
    compat owner:
    - `execute_batch_generation_in_order`
    - `generate_single_chapter_for_batch`
  - same project-quality trend snapshot helpers remain reachable on the
    surviving compat owner:
    - `get_project_quality_trend_snapshot`
    - `get_project_quality_trend_snapshot_with_default_wiring`
  - same batch resume/status and project quality route behavior remains:
    - `POST /chapters/project/{project_id}/batch-generate`
    - `POST /chapters/batch-generate/{batch_id}/resume`
    - `GET /chapters/project/{project_id}/quality-trend`
  - no HTTP payload shell, snapshot summary semantics, or batch lifecycle
    semantics were intentionally changed in this slice

  implementation boundary for this checkpoint:
  - production imports now point directly at surviving compat owners:
    - `backend/app/api/chapters.py`
      -> `app.services.compat.batch_generation_entry_compat_service`
      -> `app.services.compat.project_quality_trend_compat_service`
    - `backend/app/api/chapter_quality_routes.py`
      -> `app.services.compat.project_quality_trend_compat_service`
  - surviving compat route owners now also import the same compat owners
    directly:
    - `backend/app/services/compat/batch_generation_route_compat_service.py`
    - `backend/app/services/compat/chapter_generation_route_compat_service.py`
  - focused API tests, shared API test support, and focused service tests now
    import the same surviving compat owners directly instead of historical
    top-level shims
  - deleted top-level Python shim files:
    - `backend/app/services/batch_generation_entry_compat_service.py`
    - `backend/app/services/project_quality_trend_compat_service.py`

  this is counted as real migration progress because two more Python files
  fully left the active codebase and two more `chapters.py`-adjacent historical
  module names no longer participate in the runtime path:
  - no production import path remains on the deleted shim pair
  - no repo test import path remains on the deleted shim pair
  - the remaining batch-entry and project-quality fallback surface is now
    anchored on one explicit compat layer instead of
    `api -> top-level shim -> compat owner`

  validation passed with:
  - `python -c "import sys; sys.path.insert(0, 'backend'); from app.api import chapters, chapter_quality_routes; from app.services.compat import batch_generation_entry_compat_service, project_quality_trend_compat_service; print('ok')"`
  - `python -m pytest backend/tests/test_services/test_batch_generation_entry_compat_service.py -q`
  - `python -m pytest backend/tests/test_services/test_project_quality_trend_compat_service.py -q`
  - `python -m pytest backend/tests/test_api/test_chapters_batch_status_resume.py -q`
  - `python -m pytest backend/tests/test_api/test_chapters_quality_views.py -q`

  rollback boundary after this change:
  - rollback is file-group only:
    - restore the deleted top-level compat shim files if any external import
      surface outside this repo still depends on the historical module paths
  - no route payload shell, project quality summary semantics, or batch task
    lifecycle semantics changed in this slice

- 2026-06-07 analysis-task and regeneration top-level route compat shim group retirement checkpoint:
  this round stayed on the Python shell-compression path and retired the
  second grouped set of top-level route compat shim files around
  `chapter_analysis_task_routes.py` and `chapter_regeneration_routes.py`
  instead of reopening another standalone Rust seam.

  package map for this checkpoint:
  - Python source map:
    - `backend/app/api/chapter_analysis_task_routes.py`
    - `backend/app/api/chapter_regeneration_routes.py`
    - `backend/app/services/compat/chapter_analysis_task_route_compat_service.py`
    - `backend/app/services/compat/chapter_regeneration_route_compat_service.py`
    - `backend/tests/test_api/chapters_test_support.py`
    - `backend/tests/test_api/test_chapters_analysis.py`
    - `backend/tests/test_api/test_chapters.py`
    - `backend/tests/test_api/test_chapters_stream_routes.py`
  - Rust target map:
    - `backend-rs/src/api/chapter_generation_routes.rs`
    - `backend-rs/src/api/chapter_regeneration_routes.rs`
    - `backend-rs/src/services/chapter_single_generation_stream_workflow_service.rs`
    - `backend-rs/src/services/chapter_single_generation_write_workflow_service.rs`

  behavior contract kept stable:
  - same analysis-task routes remain:
    - `GET /chapters/{chapter_id}/analysis/status`
    - `POST /chapters/analysis/status/batch`
    - `GET /chapters/{chapter_id}/can-generate`
    - `POST /chapters/{chapter_id}/analyze`
  - same regeneration route remains:
    - `POST /chapters/{chapter_id}/regenerate-stream`
  - the same test monkeypatch surfaces remain reachable on the surviving
    compat owners:
    - `execute_chapter_analysis_background`
    - `get_db`
    - `REGENERATOR_FACTORY`
  - no HTTP payload shell, SSE shape, task lifecycle, or error message
    semantics were intentionally changed in this slice

  implementation boundary for this checkpoint:
  - production imports now point directly at surviving compat owners:
    - `backend/app/api/chapter_analysis_task_routes.py`
      -> `app.services.compat.chapter_analysis_task_route_compat_service`
    - `backend/app/api/chapter_regeneration_routes.py`
      -> `app.services.compat.chapter_regeneration_route_compat_service`
  - focused API tests and shared API test support now import the same
    surviving compat owners directly instead of historical top-level shims
  - surviving compat owners no longer dynamically re-import the deleted
    top-level shim modules to reach patchable symbols; they now use their
    local patch surfaces directly
  - deleted top-level Python shim files:
    - `backend/app/services/chapter_analysis_task_route_compat_service.py`
    - `backend/app/services/chapter_regeneration_route_compat_service.py`

  this is counted as real migration progress because two more Python files
  fully left the active codebase and the remaining analysis/regeneration route
  owners are now anchored on one explicit compat layer instead of
  `api -> top-level shim -> compat owner`:
  - no production import path remains on the deleted shim pair
  - no repo test import path remains on the deleted shim pair
  - the surviving compat owners now carry the patchable surface directly,
    which makes the next fallback-shell audit smaller and easier to retire

  validation passed with:
  - `python -c "import sys; sys.path.insert(0, 'backend'); from app.api import chapter_analysis_task_routes, chapter_regeneration_routes; from app.services.compat import chapter_analysis_task_route_compat_service, chapter_regeneration_route_compat_service; print('ok')"`
  - `python -m pytest backend/tests/test_api/test_chapters_analysis.py -q`
  - `python -m pytest backend/tests/test_api/test_chapters_stream_routes.py -q -k "regenerate or partial_regenerate or analysis"`
  - `python -m pytest backend/tests/test_api/test_chapters.py -q -k "regenerate or analysis or can_generate"`

  rollback boundary after this change:
  - rollback is file-group only:
    - restore the deleted top-level route compat shim files if any external
      import surface outside this repo still depends on the historical module
      paths
  - no route payload shell, SSE shape, or analysis/regeneration workflow
    semantics changed in this slice

- 2026-06-07 chapters-neighbor compat helper group retirement checkpoint:
  this round stayed on the Python shell-compression path and retired another
  grouped set of top-level compat helper files around `backend/app/api/chapters.py`
  instead of reopening a new standalone seam.

  package map for this checkpoint:
  - Python source map:
    - `backend/app/api/chapters.py`
    - `backend/app/services/compat/chapter_generation_route_compat_service.py`
    - `backend/app/services/compat/chapter_candidate_entry_compat_service.py`
    - `backend/tests/test_services/test_batch_generation_run_compat_service.py`
    - `backend/tests/test_services/test_chapter_generated_text_compat_service.py`
    - `backend/tests/test_services/test_chapter_prompt_quality_compat_service.py`
    - `backend/tests/test_services/test_chapter_candidate_entry_compat_service.py`
    - `backend/tests/test_services/test_chapter_candidate_executor_compat_service.py`
    - `backend/tests/test_services/test_task_workflow_runtime_compat_service.py`
    - `backend/tests/test_api/test_chapters.py`
  - Rust target map:
    - `backend-rs/src/services/chapter_single_generation_write_workflow_service.rs`
    - `backend-rs/src/services/chapter_single_generation_stream_workflow_service.rs`
    - `backend-rs/src/services/chapter_generation_execution_contract_service.rs`
    - `backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs`

  behavior contract kept stable:
  - `chapters.py` keeps the same helper call graph and external route behavior
  - the same compat helper functions remain patchable/importable on the
    surviving `app.services.compat.*` owners
  - candidate generation, generated-text sanitation, prompt/quality helper,
    cancelable batch wait, and task-workflow snapshot semantics were not
    intentionally changed in this slice

  implementation boundary for this checkpoint:
  - `backend/app/api/chapters.py` now imports these compat helpers directly
    from `app.services.compat.*`:
    - `chapter_candidate_entry_compat_service`
    - `chapter_candidate_executor_compat_service`
    - `chapter_generated_text_compat_service`
    - `chapter_prompt_quality_compat_service`
    - `batch_generation_run_compat_service`
    - `task_workflow_runtime_compat_service`
  - `backend/app/services/compat/chapter_generation_route_compat_service.py`
    now also imports candidate and prompt/quality helpers from surviving
    compat owners directly instead of the deleted top-level mirror files
  - focused service tests now import the surviving compat owners directly
  - deleted top-level Python helper files:
    - `backend/app/services/batch_generation_run_compat_service.py`
    - `backend/app/services/chapter_generated_text_compat_service.py`
    - `backend/app/services/chapter_prompt_quality_compat_service.py`
    - `backend/app/services/task_workflow_runtime_compat_service.py`
    - `backend/app/services/chapter_candidate_entry_compat_service.py`
    - `backend/app/services/chapter_candidate_executor_compat_service.py`

  this is counted as real migration progress because six more Python files
  fully left the active codebase and the large `chapters.py` route owner no
  longer depends on their historical top-level names:
  - no production import path remains on the deleted helper group
  - no repo test import path remains on the deleted helper group
  - the remaining Python helper surface around `chapters.py` is narrower and
    more explicitly anchored on the surviving compat owners

  validation passed with:
  - `python -c "import sys; sys.path.insert(0, 'backend'); from app.api import chapters; from app.services.compat import chapter_candidate_entry_compat_service, chapter_candidate_executor_compat_service, chapter_generated_text_compat_service, chapter_prompt_quality_compat_service, batch_generation_run_compat_service, task_workflow_runtime_compat_service; print('ok')"`
  - `python -m pytest backend/tests/test_services/test_chapter_candidate_executor_compat_service.py -q`
  - `python -m pytest backend/tests/test_services/test_chapter_candidate_entry_compat_service.py -q`
  - `python -m pytest backend/tests/test_services/test_chapter_generated_text_compat_service.py -q`
  - `python -m pytest backend/tests/test_services/test_chapter_prompt_quality_compat_service.py -q`
  - `python -m pytest backend/tests/test_services/test_batch_generation_run_compat_service.py -q`
  - `python -m pytest backend/tests/test_services/test_task_workflow_runtime_compat_service.py -q`
  - `python -m pytest backend/tests/test_api/test_chapters.py -q -k "reuse_active_background_task_for_same_chapter or quality_gate or partial_regenerate" -vv`

  rollback boundary after this change:
  - rollback is file-group only:
    - restore the deleted top-level helper files if any external import
      surface outside this repo still depends on the historical module paths
  - no HTTP payload shell, SSE shape, task lifecycle, or fallback message
    semantics changed in this slice

- 2026-06-07 chapter-generation top-level stream/runtime shim group retirement checkpoint:
  this round switched to Package A `chapter_generation` at the Python fallback
  shell layer and retired one whole group of top-level compatibility shim
  files instead of continuing another low-signal Rust-only seam.

  package map for this checkpoint:
  - Python source map:
    - `backend/app/api/chapters.py`
    - `backend/app/api/chapter_batch_generation_routes.py`
    - `backend/app/services/batch_generation_retry_service.py`
    - `backend/app/services/batch_generation_single_chapter_wiring_service.py`
    - `backend/app/services/chapter_regeneration_context_service.py`
    - `backend/app/services/compat/batch_generation_route_compat_service.py`
    - `backend/app/services/compat/chapter_analysis_task_route_compat_service.py`
    - `backend/app/services/compat/chapter_generation_route_compat_service.py`
    - `backend/app/services/compat/chapter_prompt_quality_compat_service.py`
    - `backend/tests/test_api/test_chapters_stream_routes.py`
  - Rust target map:
    - `backend-rs/src/api/chapter_generation_routes.rs`
    - `backend-rs/src/services/chapter_single_generation_stream_workflow_service.rs`
    - `backend-rs/src/services/chapter_single_generation_write_workflow_service.rs`
    - `backend-rs/src/services/chapter_generation_execution_contract_service.rs`

  behavior contract kept stable:
  - same single-chapter stream route remains:
    `POST /chapters/{chapter_id}/generate-stream`
  - same single-chapter background route remains:
    `POST /chapters/{chapter_id}/generate-background`
  - same batch generation stream route remains:
    `GET /chapters/batch-generate/{batch_id}/stream`
  - same stream/batch/analysis test monkeypatch surfaces remain reachable on
    the surviving compat owners
  - same prerequisite, runtime prompt, request-policy, and stream dependency
    semantics remain unchanged; only import ownership moved to the real
    owner files

  implementation boundary for this checkpoint:
  - production imports were repointed from top-level shims to the real
    `chapter_generation` owners:
    - `app.services.chapter_generation.runtime.service`
    - `app.services.chapter_generation.runtime.prompt_service`
    - `app.services.chapter_generation.prerequisite_service`
    - `app.services.chapter_generation.stream.entry_service`
    - `app.services.chapter_generation.stream.request_policy_service`
  - batch stream route ownership was also tightened:
    - `backend/app/api/chapter_batch_generation_routes.py`
      now imports from
      `app.services.compat.batch_generation_route_compat_service`
    - focused stream-route tests now patch the same surviving compat owner
  - deleted top-level Python shim files:
    - `backend/app/services/chapter_generation_runtime_service.py`
    - `backend/app/services/chapter_generation_runtime_prompt_service.py`
    - `backend/app/services/chapter_generation_prerequisite_service.py`
    - `backend/app/services/chapter_generation_stream_entry_service.py`
    - `backend/app/services/chapter_generation_stream_request_policy_service.py`
    - `backend/app/services/chapter_generation_stream_service.py`
    - `backend/app/services/chapter_generation_stream_wiring_service.py`
    - `backend/app/services/chapter_generation_stream_candidate_service.py`
    - `backend/app/services/chapter_generation_stream_execution_service.py`
    - `backend/app/services/chapter_generation_stream_finalize_service.py`
    - `backend/app/services/chapter_generation_stream_models.py`
    - `backend/app/services/batch_generation_route_compat_service.py`

  this is counted as real migration progress because twelve Python files fully
  left the active codebase in one grouped checkpoint instead of lingering as
  long-lived import bridges:
  - no production import path remains on the deleted file group
  - no repo test import path remains on the deleted file group
  - the `chapter_generation` Python shell is now materially thinner and closer
    to “real owner files only” rather than “real owner + top-level mirror
    shims”

  validation passed with:
  - `python -c "import sys; sys.path.insert(0, 'backend'); from app.api import chapters; from app.api import chapter_batch_generation_routes; from app.services.compat import chapter_generation_route_compat_service, batch_generation_route_compat_service; print('ok')"`
  - `python -m pytest backend/tests/test_api/test_chapters_stream_routes.py -q`
  - `python -m pytest backend/tests/test_api/test_chapters_batch_generation.py -q -k "generation_route_compat or quality_gate"`
  - `python -m pytest backend/tests/test_api/test_chapters_analysis.py -q`
  - `python -m pytest backend/tests/test_services/test_chapter_generation_background_entry_service.py -q`

  rollback boundary after this change:
  - rollback is file-group only:
    - restore the deleted top-level shim files if any external import surface
      outside this repo still depends on the historical module paths
  - no route payload shell, SSE payload shape, task lifecycle, or provider
    default semantics changed in this slice

- 2026-06-07 single-generation route compat shim retirement checkpoint:
  this round stayed on Package B `chapter_single_generation` and continued the
  Python fallback-shell compression path by retiring one more whole-file route
  compat shim instead of adding another Rust-only micro seam.

  package map for this checkpoint:
  - Python source map:
    - `backend/app/api/chapter_generation_routes.py`
    - `backend/app/api/chapters.py`
    - `backend/app/services/compat/chapter_generation_route_compat_service.py`
    - `backend/app/services/chapter_generation/stream/entry_service.py`
    - `backend/app/services/chapter_generation/background_entry_service.py`
    - `backend/app/services/batch_generation_orchestration_service.py`
    - `backend/tests/test_api/chapters_test_support.py`
    - `backend/tests/test_api/test_chapters_stream_routes.py`
    - `backend/tests/test_api/test_chapters_batch_generation.py`
  - Rust target map:
    - `backend-rs/src/api/chapter_generation_routes.rs`
    - `backend-rs/src/services/chapter_single_generation_write_workflow_service.rs`
    - `backend-rs/src/services/chapter_single_generation_existing_background_query_service.rs`

  behavior contract kept stable:
  - same single-chapter stream route remains:
    `POST /chapters/{chapter_id}/generate-stream`
  - same single-chapter background route remains:
    `POST /chapters/{chapter_id}/generate-background`
  - same existing-background short-circuit shell remains:
    `task_id/chapter_id/status/message/estimated_time_minutes`
  - same testing monkeypatch surface remains available on the surviving compat
    owner:
    - `get_db`
    - `get_template`
    - `format_prompt`
    - `OneToOneContextBuilder`
    - `OneToManyContextBuilder`
    - `build_chapter_runtime_system_prompt`
    - `compute_story_quality_metrics`
    - `resolve_quality_gate_execution_plan`
    - `execute_chapter_analysis_background`

  implementation boundary for this checkpoint:
  - deleted file:
    - `backend/app/services/chapter_generation_route_compat_service.py`
  - production imports now point directly at the surviving compat owner:
    - `backend/app/api/chapter_generation_routes.py`
      -> `app.services.compat.chapter_generation_route_compat_service`
    - `backend/app/api/chapters.py`
      -> `app.services.compat.chapter_generation_route_compat_service`
    - `backend/app/services/chapter_generation/stream/entry_service.py`
      -> `app.services.compat.chapter_generation_route_compat_service`
  - the surviving compat owner
    `backend/app/services/compat/chapter_generation_route_compat_service.py`
    no longer resolves its own dependencies through the deleted top-level shim;
    it now uses its local owner symbols directly
  - focused API tests that previously imported the deleted shim now import the
    surviving compat owner directly:
    - `backend/tests/test_api/chapters_test_support.py`
    - `backend/tests/test_api/test_chapters_stream_routes.py`
    - `backend/tests/test_api/test_chapters_batch_generation.py`
  - `backend/app/services/__init__.py` stays clean again after a temporary
    lazy-alias experiment was discarded to avoid package-level circular import
    side effects

  this is counted as real migration progress because one more Python file
  fully left the active codebase instead of remaining as a permanent route
  bridge:
  - no production import path remains on the deleted shim file
  - no test import path remains on the deleted shim file
  - the single-generation background/stream Python fallback chain is now
    tighter:
    `route/api -> compat owner -> real entry/background/orchestration owners`

  validation passed with:
  - `python -c "import sys; sys.path.insert(0, 'backend'); from app.api import chapter_generation_routes; from app.api import chapters; from app.services.compat import chapter_generation_route_compat_service; print('ok')"`
  - `python -m pytest backend/tests/test_services/test_single_chapter_background_generation_service.py -q`
  - `python -m pytest backend/tests/test_api/test_chapters.py -q -k reuse_active_background_task_for_same_chapter`
  - `python -m pytest backend/tests/test_api/test_chapters_stream_routes.py -q -k "generate_stream or expected_builder"`
  - `python -m pytest backend/tests/test_api/test_chapters_batch_generation.py -q -k "generation_route_compat or quality_gate"`

  rollback boundary after this change:
  - rollback is file-level only:
    - restore
      `backend/app/services/chapter_generation_route_compat_service.py`
      if any legacy external import surface outside repo tests still requires
      the deleted shim
  - no gateway route ownership, transport, payload shell, or fallback message
    semantics changed in this slice

- 2026-06-07 single-generation Python background entry shim retirement checkpoint:
  this round stayed on Package B `chapter_single_generation` and continued the
  fallback-shell compression path by removing one more Python background
  routing shim file from the active code path.

  package map for this checkpoint:
  - Python source map:
    - `backend/app/api/chapter_generation_routes.py`
    - `backend/app/services/compat/chapter_generation_route_compat_service.py`
    - `backend/app/services/chapter_generation/background_entry_service.py`
    - `backend/app/services/batch_generation_orchestration_service.py`
  - Rust target map:
    - `backend-rs/src/api/chapter_generation_routes.rs`
    - `backend-rs/src/services/chapter_single_generation_write_workflow_service.rs`
    - `backend-rs/src/services/chapter_single_generation_existing_background_query_service.rs`

  behavior contract kept stable:
  - same single-chapter background route remains:
    `POST /chapters/{chapter_id}/generate-background`
  - same existing-background short-circuit shell remains:
    `task_id/chapter_id/status/message/estimated_time_minutes`
  - same existing-background message remains unchanged:
    `已有后台生成任务正在执行`
  - no background-entry transport, payload, or fallback semantics were
    intentionally changed in this slice

  implementation boundary for this checkpoint:
  - deleted shim file:
    - `backend/app/services/chapter_generation_background_entry_service.py`
  - surviving compat owner
    `backend/app/services/compat/chapter_generation_route_compat_service.py`
    now imports the real background entry owner directly from:
    - `backend/app/services/chapter_generation/background_entry_service.py`
  - surviving Python fallback chain is now tighter:
    `route compat -> background entry owner -> batch_generation_orchestration`

  this is counted as real migration progress because one more Python
  file-level compatibility hop fully left the active code path instead of
  staying behind as a long-lived import bridge:
  - no remaining import path points at the deleted shim file
  - the single-generation background Python fallback chain now has one less
    file to audit before future retirement

  validation passed with:
  - `python -m pytest backend/tests/test_services/test_single_chapter_background_generation_service.py -q`
  - `python -m pytest backend/tests/test_api/test_chapters.py -q -k reuse_active_background_task_for_same_chapter`

  rollback boundary after this change:
  - rollback is file-level only:
    - restore
      `backend/app/services/chapter_generation_background_entry_service.py`
      if any legacy import surface unexpectedly still requires the deleted shim
  - no gateway route ownership, transport, or schema ownership changed in this
    slice

- 2026-06-07 single-generation Python background fallback shim retirement checkpoint:
  this round stayed on Package B `chapter_single_generation` and continued the
  previous Python fallback owner collapse one step further: the compatibility
  shim file itself is now gone.

  package map for this checkpoint:
  - Python source map:
    - `backend/app/api/chapter_generation_routes.py`
    - `backend/app/services/compat/chapter_generation_route_compat_service.py`
    - `backend/app/services/chapter_generation/background_entry_service.py`
    - `backend/app/services/batch_generation_orchestration_service.py`
    - `backend/tests/test_services/test_single_chapter_background_generation_service.py`
  - Rust target map:
    - `backend-rs/src/api/chapter_generation_routes.rs`
    - `backend-rs/src/services/chapter_single_generation_write_workflow_service.rs`
    - `backend-rs/src/services/chapter_single_generation_existing_background_query_service.rs`

  behavior contract kept stable:
  - same single-chapter background route remains:
    `POST /chapters/{chapter_id}/generate-background`
  - same existing-background short-circuit shell remains:
    `task_id/chapter_id/status/message/estimated_time_minutes`
  - same existing-background message remains unchanged:
    `已有后台生成任务正在执行`
  - same stale-recovery thresholds remain unchanged:
    - pending timeout: 3 minutes
    - running timeout: 15 minutes
  - same chapter-id compatibility remains unchanged:
    - `["chapter-id"]`
    - `[{"id": "chapter-id"}]`

  implementation boundary for this checkpoint:
  - deleted file:
    - `backend/app/services/single_chapter_background_generation_service.py`
  - focused service test
    `backend/tests/test_services/test_single_chapter_background_generation_service.py`
    now imports the surviving owner functions directly from:
    - `backend/app/services/batch_generation_orchestration_service.py`
  - surviving Python fallback owner chain is now:
    `route compat -> background entry -> batch_generation_orchestration`

  this is counted as real migration progress because one Python fallback file
  fully left the active codebase instead of remaining as a long-lived re-export
  shell:
  - no production import path remains on the deleted file
  - the remaining Python single-generation fallback surface is materially
    smaller than the previous checkpoint

  validation passed with:
  - `python -m pytest backend/tests/test_services/test_single_chapter_background_generation_service.py -q`
  - `python -m pytest backend/tests/test_api/test_chapters.py -q -k reuse_active_background_task_for_same_chapter`

  rollback boundary after this change:
  - rollback is file-level only:
    - restore
      `backend/app/services/single_chapter_background_generation_service.py`
      if any unexpected import surface is still needed
  - no gateway route ownership, transport, or schema ownership changed in this
    slice

- 2026-06-07 single-generation Python background fallback owner file-collapse checkpoint:
  this round stayed on Package B `chapter_single_generation`, but unlike the
  previous Rust-only owner lifts it moved one real Python fallback owner file
  back into the surviving Python orchestration owner so the remaining fallback
  shell is thinner and easier to retire.

  package map for this checkpoint:
  - Python source map:
    - `backend/app/api/chapter_generation_routes.py`
    - `backend/app/services/compat/chapter_generation_route_compat_service.py`
    - `backend/app/services/chapter_generation/background_entry_service.py`
    - `backend/app/services/batch_generation_orchestration_service.py`
    - `backend/app/services/single_chapter_background_generation_service.py`
  - Rust target map:
    - `backend-rs/src/api/chapter_generation_routes.rs`
    - `backend-rs/src/services/chapter_single_generation_write_workflow_service.rs`
    - `backend-rs/src/services/chapter_single_generation_existing_background_query_service.rs`

  behavior contract kept stable:
  - same single-chapter background route remains:
    `POST /chapters/{chapter_id}/generate-background`
  - same existing-background short-circuit shell remains:
    `task_id/chapter_id/status/message/estimated_time_minutes`
  - same existing-background message remains unchanged:
    `已有后台生成任务正在执行`
  - same pending/running stale-recovery thresholds remain unchanged:
    - pending timeout: 3 minutes
    - running timeout: 15 minutes
  - same chapter-id compatibility remains unchanged for Python fallback:
    - `["chapter-id"]`
    - `[{"id": "chapter-id"}]`

  implementation boundary for this checkpoint:
  - the surviving Python orchestration owner
    `backend/app/services/batch_generation_orchestration_service.py`
    now directly owns the remaining Python fallback behavior for:
    - single-generation existing-background active-task query
    - stale pending/running task recovery
    - object/string `chapter_ids` compatibility matching
    - single-generation background preparation payload
    - single-generation background task creation/enqueue branch
  - the old file
    `backend/app/services/single_chapter_background_generation_service.py`
    no longer owns real behavior and now stays only as a thin compatibility
    shim that re-exports from the surviving orchestration owner
  - this keeps the active Python fallback owner chain tighter:
    `route compat -> background entry -> batch_generation_orchestration`

  this is counted as real migration progress because one entire Python owner
  file stopped carrying runtime behavior and was downgraded into a frozen shim
  while the real Rust owner chain for single-generation background handling
  already exists on the strangler side:
  - Python fallback became thinner instead of preserving a second behavior file
  - future fallback retirement only needs to remove a shim instead of auditing
    another independent runtime owner

  validation passed with:
  - `python -m pytest backend/tests/test_services/test_single_chapter_background_generation_service.py -q`
  - `python -m pytest backend/tests/test_api/test_chapters.py -q -k reuse_active_background_task_for_same_chapter`

  rollback boundary after this change:
  - rollback is file-level only on the Python fallback shell
  - restore the previous implementation in
    `backend/app/services/single_chapter_background_generation_service.py`
    if import-side compatibility or fallback behavior regresses
  - no gateway route ownership, transport, or schema ownership changed in this
    slice

- 2026-06-07 chapter_single_generation Python fallback compatibility checkpoint:
  this round stayed on Package B `chapter_single_generation`, but instead of
  reopening another Rust micro-seam it tightened the remaining Python
  comparison shell so it stays aligned with the already-established Rust owner
  contract for single-generation existing-background reuse.

  package map for this checkpoint:
  - Python source map:
    - `backend/app/api/chapter_generation_routes.py`
    - `backend/app/services/compat/chapter_generation_route_compat_service.py`
    - `backend/app/services/chapter_generation_route_compat_service.py`
    - `backend/app/services/single_chapter_background_generation_service.py`
  - Rust target map:
    - `backend-rs/src/services/chapter_single_generation_existing_background_query_service.rs`
    - `backend-rs/src/services/chapter_single_generation_write_workflow_service.rs`

  behavior contract kept stable:
  - same single-chapter background route remains:
    `POST /chapters/{chapter_id}/generate-background`
  - same response shell remains unchanged:
    `task_id/chapter_id/status/message/estimated_time_minutes`
  - same existing-background short-circuit message remains unchanged:
    `已有后台生成任务正在执行`
  - same stale-task auto-recovery thresholds remain unchanged:
    - pending timeout: 3 minutes
    - running timeout: 15 minutes
  - no route payload, runtime launch, checkpoint payload, or fallback message
    semantics were intentionally changed in this slice

  implementation boundary for this checkpoint:
  - deleted obsolete Python shim file:
    - `backend/app/services/chapter_generation/route_compat_service.py`
  - surviving top-level shim
    `backend/app/services/chapter_generation_route_compat_service.py`
    now imports directly from:
    - `app.services.compat.chapter_generation_route_compat_service`
  - `backend/app/services/single_chapter_background_generation_service.py`
    now directly owns the remaining Python fallback compatibility guard for:
    - string-style `chapter_ids`: `["chapter-id"]`
    - object-style `chapter_ids`: `[{"id": "chapter-id"}]`
  - the same Python fallback owner now also uses a naive-UTC current time when
    recovering stale pending/running tasks so it matches the current DB
    timestamp shape instead of comparing fresh UTC timestamps against local
    wall-clock time
  - added focused Python regression tests:
    - `backend/tests/test_services/test_single_chapter_background_generation_service.py`
    - `backend/tests/test_api/test_chapters.py -k reuse_active_background_task_for_same_chapter`

  this is counted as real migration progress because it removes one more
  redundant Python shim layer and keeps the remaining fallback shell aligned
  with the real Rust owner contract instead of letting the comparison boundary
  drift:
  - Python shell remains thinner:
    `route -> compat shell -> background entry/orchestration`
  - existing-background reuse now stays consistent across:
    - Rust existing-background owner
    - Python fallback shell
    - shared task table timestamp semantics

  validation passed with:
  - `python -m pytest backend/tests/test_services/test_single_chapter_background_generation_service.py -q`
  - `python -m pytest backend/tests/test_api/test_chapters.py -q -k reuse_active_background_task_for_same_chapter`
  - `python -m pytest backend/tests/test_api/test_chapters_stream_routes.py -q -k "generate_stream or expected_builder"`
  - `python -m pytest backend/tests/test_api/test_chapters_batch_generation.py -q -k "execute_chapter_analysis_background or generate"`

  rollback boundary after this change:
  - there is no transport/gateway rollback change in this slice
  - rollback is file-level only:
    - restore the deleted shim if a legacy import surface is unexpectedly
      needed
    - restore the previous Python fallback helper if single-generation
      existing-background reuse or stale-recovery behavior regresses

- 2026-06-07 chapter_single_generation task-view payload file-collapse checkpoint:
  this round stayed on Package B `chapter_single_generation` and continued the
  whole-file migration path by collapsing the dedicated single-generation
  task-view payload file back into the surviving prepare owner.

  package map for this checkpoint:
  - Python source map:
    - `backend/app/api/chapter_generation_routes.py`
    - `backend/app/services/chapter_generation_route_compat_service.py`
      as the frozen Python-side comparison boundary
  - Rust target map:
    - `backend-rs/src/services/chapter_single_generation_prepare_service.rs`
    - `backend-rs/src/services/chapter_single_generation_existing_background_query_service.rs`
    - `backend-rs/src/services/chapter_single_generation_write_workflow_service.rs`
    - `backend-rs/src/services/mod.rs`

  behavior contract kept stable:
  - same single-generation active-status semantics, stage-code mapping,
    runtime payload base, task-view payload projection, and estimated-minutes
    contract remain unchanged
  - same existing-background payload projection keeps the same outer
    `task_id/chapter_id/status/message/estimated_time_minutes` shell
  - no route payload, runtime launch, checkpoint persistence, or fallback
    behavior was intentionally changed in this slice

  implementation boundary for this checkpoint:
  - removed the neighboring dedicated read-side payload file:
    - `backend-rs/src/services/chapter_single_generation_task_view_payload_service.rs`
  - the surviving prepare owner
    `backend-rs/src/services/chapter_single_generation_prepare_service.rs`
    now directly owns:
    - active-status semantics
    - pending/running/completed/failed/cancelled stage-code mapping
    - runtime payload base projection
    - task-view payload projection
    - focused payload/status/minutes/task-state contract tests
  - `backend-rs/src/services/chapter_single_generation_existing_background_query_service.rs`
    now consumes those payload helpers directly from the surviving prepare
    owner instead of a dedicated neighboring file
  - `backend-rs/src/services/mod.rs`
    now drops the deleted `chapter_single_generation_task_view_payload_service`
    module registration

  this is counted as real migration progress because it removes one more
  single-generation file that no longer owns an independent route/query or
  fallback boundary:
  - prepare owner -> request/target/runtime materialization + task-view payload
  - existing-background query owner -> query/recovery/existing payload
  - write-workflow owner -> existing payload vs prepared launch branch

  rollback boundary after this change:
  - there is no transport/gateway rollback change in this slice
  - rollback is file-level only: restore the deleted task-view payload file if
    the prepare-owner payload contract regression is observed

- 2026-06-07 chapter_single_generation runtime public-start wrapper collapse checkpoint:
  this round stayed on Package B `chapter_single_generation` and continued the
  whole-file migration path inside the single-generation runtime lane.

  package map for this checkpoint:
  - Python source map:
    - `backend/app/api/chapter_generation_routes.py`
    - `backend/app/services/chapter_generation_route_compat_service.py`
      as the frozen Python-side comparison boundary
  - Rust target map:
    - `backend-rs/src/services/chapter_single_generation_runtime_state_service.rs`
    - `backend-rs/src/services/chapter_single_generation_write_workflow_service.rs`
    - `backend-rs/src/services/chapter_batch_generation_resume_task_command_service.rs`

  behavior contract kept stable:
  - same single-generation runtime launch input contract remains unchanged
  - same preparation persistence, generation execution, follow-up analysis,
    terminal checkpoint persistence, and manual-review/completed/failed
    semantics remain unchanged
  - no route payload, task lifecycle, response shell, or fallback behavior
    was intentionally changed in this slice

  implementation boundary for this checkpoint:
  - removed the neighboring public free-function wrapper:
    - `dispatch_single_chapter_generation_runtime(...)`
  - the surviving lifecycle owner
    `SingleGenerationRuntimeLifecyclePlan`
    now exposes the public runtime boundary directly:
    - `from_runtime_launch(...)`
    - `spawn(...)`
  - `chapter_single_generation_write_workflow_service.rs`
    now calls the surviving lifecycle owner directly for background launch
    dispatch
  - `chapter_batch_generation_resume_task_command_service.rs`
    now calls the surviving lifecycle owner directly for single-chapter resume
    dispatch

  this is counted as real migration progress because it removes one more
  single-call runtime-lane shell around the surviving lifecycle owner instead
  of preserving a duplicate public boundary:
  - background/resume owner -> runtime lifecycle owner
  - runtime lifecycle owner -> prepare/execute/persist terminal outcome

  rollback boundary after this change:
  - there is no transport/gateway rollback change in this slice
  - rollback is file-level only: restore the deleted runtime public-start
    wrapper if the direct lifecycle-owner handoff introduces a regression

- 2026-06-07 chapter_single_generation background public-start wrapper collapse checkpoint:
  this round stayed on Package B `chapter_single_generation` and continued the
  whole-file migration path inside the single-generation background-write lane.

  package map for this checkpoint:
  - Python source map:
    - `backend/app/api/chapter_generation_routes.py`
    - `backend/app/services/chapter_generation_route_compat_service.py`
      as the frozen Python-side comparison boundary
  - Rust target map:
    - `backend-rs/src/services/chapter_single_generation_write_workflow_service.rs`
    - `backend-rs/src/api/chapter_generation_routes.rs`

  behavior contract kept stable:
  - same single-generation background route payload boundary remains unchanged
  - same existing-task short-circuit branch, launch-parts persistence,
    startup snapshot persistence, runtime dispatch, and response payload
    semantics remain unchanged
  - no task lifecycle, checkpoint shape, response shell, or fallback
    behavior was intentionally changed in this slice

  implementation boundary for this checkpoint:
  - removed the neighboring public free-function wrapper:
    - `start_owned_single_generation_background_write_entry(...)`
  - the surviving workflow-entry owner
    `SingleGenerationBackgroundWriteWorkflowEntry`
    now exposes the route-facing owner boundary directly:
    - `start_from_route_payload(...)`
  - `chapter_generation_routes.rs`
    now calls the surviving workflow-entry owner directly instead of reopening
    `route payload -> request -> workflow start` through a one-call handoff shell

  this is counted as real migration progress because it removes one more
  single-call background-lane shell around the surviving workflow-entry owner
  instead of preserving a duplicate public boundary:
  - route/public shell -> write workflow entry owner
  - write workflow entry owner -> existing-task payload | launch persist/dispatch

  rollback boundary after this change:
  - there is no transport/gateway rollback change in this slice
  - rollback is file-level only: restore the deleted background public-start
    wrapper if the direct workflow-entry handoff introduces a regression

- 2026-06-07 chapter_single_generation stream public-start wrapper collapse checkpoint:
  this round stayed on Package B `chapter_single_generation` and continued the
  whole-file migration path inside the single-generation stream lane.

  package map for this checkpoint:
  - Python source map:
    - `backend/app/api/chapter_generation_routes.py`
    - `backend/app/services/chapter_generation_route_compat_service.py`
      as the frozen Python-side comparison boundary
  - Rust target map:
    - `backend-rs/src/services/chapter_single_generation_stream_workflow_service.rs`
    - `backend-rs/src/services/chapter_single_generation_stream_entry_service.rs`

  behavior contract kept stable:
  - same single-generation stream route payload boundary remains unchanged
  - same lifecycle spawn cadence, progress SSE, success event ordering,
    follow-up analysis trigger, and failure shell semantics remain unchanged
  - no task lifecycle, checkpoint shape, response payload, or fallback
    behavior was intentionally changed in this slice

  implementation boundary for this checkpoint:
  - removed the neighboring public free-function wrapper:
    - `spawn_owned_single_generation_stream_from_runtime_launch(...)`
  - the surviving lifecycle owner
    `SingleGenerationStreamLifecyclePlan`
    now exposes the public owner boundary directly:
    - `from_runtime_launch(...)`
    - `spawn(...)`
  - `chapter_single_generation_stream_entry_service.rs`
    now calls the surviving lifecycle owner directly instead of reopening
    `runtime launch input -> spawn` through a one-call handoff shell

  this is counted as real migration progress because it removes one more
  single-call stream-lane shell around the surviving lifecycle owner instead
  of preserving a duplicate public boundary:
  - route/public shell -> stream entry owner -> lifecycle owner
  - lifecycle owner -> spawn/progress/runtime/success/error SSE

  rollback boundary after this change:
  - there is no transport/gateway rollback change in this slice
  - rollback is file-level only: restore the deleted stream public-start
    wrapper if the direct lifecycle-owner handoff introduces a regression

- 2026-06-07 chapter_single_generation prepare public-entry wrapper collapse checkpoint:
  this round stayed on Package B `chapter_single_generation` and continued the
  whole-file migration path inside the single-generation prepare lane.

  package map for this checkpoint:
  - Python source map:
    - `backend/app/api/chapter_generation_routes.py`
    - `backend/app/services/chapter_generation_route_compat_service.py`
      as the frozen Python-side comparison boundary
  - Rust target map:
    - `backend-rs/src/services/chapter_single_generation_prepare_service.rs`
    - `backend-rs/src/services/chapter_single_generation_stream_entry_service.rs`
    - `backend-rs/src/services/chapter_single_generation_write_workflow_service.rs`

  behavior contract kept stable:
  - same single-generation request validation, target load, restored runtime
    state materialization, runtime launch input, and background launch parts
    contracts remain unchanged
  - same stream entry and background write entry keep their existing route
    payload boundary and downstream runtime/persistence behavior
  - no route payload, task lifecycle, checkpoint, or fallback behavior was
    intentionally changed in this slice

  implementation boundary for this checkpoint:
  - removed the neighboring public free-function wrappers:
    - `prepare_single_generation_runtime_launch_input(...)`
    - `prepare_single_generation_background_launch_parts_from_target(...)`
  - the surviving prepare owner
    `PreparedSingleChapterGenerationRestoredRuntimeLaunch`
    now exposes the public owner entrypoints directly:
    - `prepare_runtime_launch_input(...)`
    - `prepare_background_launch_parts_from_target(...)`
  - `chapter_single_generation_stream_entry_service.rs`
    now calls the surviving prepare owner directly for runtime launch input
  - `chapter_single_generation_write_workflow_service.rs`
    now calls the surviving prepare owner directly for background launch parts

  this is counted as real migration progress because it removes one more
  single-call handoff shell around the surviving prepare owner instead of
  preserving a duplicate public boundary:
  - route/public shell -> stream entry/write workflow -> prepare owner
  - prepare owner -> restored launch/runtime/background products

  rollback boundary after this change:
  - there is no transport/gateway rollback change in this slice
  - rollback is file-level only: restore the deleted free-function wrappers if
    the direct prepare-owner entrypoint handoff introduces a regression

- 2026-06-07 chapter_single_generation task-view payload owner split checkpoint:
  this round stayed on Package B `chapter_single_generation` and continued the
  whole-file migration path inside the single-generation read-side payload lane.

  package map for this checkpoint:
  - Python source map:
    - `backend/app/api/chapter_generation_routes.py`
    - `backend/app/services/chapter_generation_route_compat_service.py`
      as the frozen Python-side comparison boundary
  - Rust target map:
    - `backend-rs/src/services/chapter_single_generation_task_view_payload_service.rs`
    - `backend-rs/src/services/chapter_single_generation_prepare_service.rs`
    - `backend-rs/src/services/chapter_single_generation_existing_background_query_service.rs`
    - `backend-rs/src/services/chapter_single_generation_write_workflow_service.rs`
    - `backend-rs/src/services/mod.rs`

  behavior contract kept stable:
  - same single-generation task payload base fields, stage-code mapping,
    execution-mode payload, active-status list, and estimated-minutes contract
    remain unchanged
  - same existing-background payload projection keeps the same outer
    `task_id/chapter_id/status/message/estimated_time_minutes` shell
  - no route payload, runtime launch, checkpoint persistence, or fallback
    behavior was intentionally changed in this slice

  implementation boundary for this checkpoint:
  - added new dedicated read-side payload owner:
    `backend-rs/src/services/chapter_single_generation_task_view_payload_service.rs`
  - that new file now owns:
    - active-status semantics
    - pending/running/completed/failed/cancelled stage-code mapping
    - runtime payload base projection
    - task view payload projection
    - focused task-view owner contract tests
  - `backend-rs/src/services/chapter_single_generation_prepare_service.rs`
    now keeps request/target/runtime preparation responsibilities instead of
    also carrying read-side task payload projection helpers
  - `backend-rs/src/services/chapter_single_generation_existing_background_query_service.rs`
    now consumes the dedicated task-view payload owner directly instead of
    reaching into the prepare owner for read-side payload assembly

  this is counted as real migration progress because it removes one mixed
  owner boundary from Package B instead of preserving read-side task payload
  semantics inside the large prepare owner file:
  - prepare owner -> request/target/runtime materialization
  - task-view payload owner -> read-side task/task-state payload projection
  - existing-background query owner -> query/recovery/payload enrichment

  rollback boundary after this change:
  - there is no transport/gateway rollback change in this slice
  - rollback is file-level only: move the task-view payload helpers back into
    the prepare owner if the dedicated read-side payload owner introduces a
    regression

- 2026-06-07 chapter_single_generation existing-background query owner split checkpoint:
  this round stayed on Package B `chapter_single_generation` and continued the
  whole-file migration path inside the single-generation background-write lane.

  package map for this checkpoint:
  - Python source map:
    - `backend/app/api/chapter_generation_routes.py`
    - `backend/app/services/chapter_generation_route_compat_service.py`
      as the frozen Python-side comparison boundary
  - Rust target map:
    - `backend-rs/src/api/chapter_generation_routes.rs`
    - `backend-rs/src/services/chapter_single_generation_existing_background_query_service.rs`
    - `backend-rs/src/services/chapter_single_generation_write_workflow_service.rs`
    - `backend-rs/src/services/chapter_single_generation_prepare_service.rs`
    - `backend-rs/src/services/chapter_single_generation_runtime_state_service.rs`
    - `backend-rs/src/services/mod.rs`

  behavior contract kept stable:
  - same single-chapter background generation route remains Rust-owned:
    - `POST /chapters/{chapter_id}/generate-background`
  - same route payload contract remains unchanged
  - same existing-background task short-circuit payload, quality/checkpoint
    payload fields, task creation, startup snapshot persistence, and runtime
    dispatch semantics remain unchanged
  - no background response shell, fallback boundary, or rollback behavior was
    intentionally changed in this slice

  implementation boundary for this checkpoint:
  - added new dedicated query owner:
    `backend-rs/src/services/chapter_single_generation_existing_background_query_service.rs`
  - that new file now owns:
    - active single-generation task query
    - recovery-aware snapshot/read-state loading
    - existing-background payload projection
    - focused existing-background owner contract tests
  - the surviving write workflow file
    `backend-rs/src/services/chapter_single_generation_write_workflow_service.rs`
    now keeps only:
    - request -> chapter target preparation
    - existing payload vs prepared launch branch decision
    - persisted launch + runtime dispatch handoff
    - focused write-workflow owner contract tests
  - service module registration now points at the explicit query owner instead
    of keeping the whole read/query/projection chain inline inside the write
    workflow file

  this is counted as real migration progress because it restores one explicit
  Rust owner map inside the single-generation background-write lane instead of
  preserving a mixed file boundary:
  - route/public shell -> write workflow owner -> existing-background query owner
  - route/public shell -> write workflow owner -> prepare/runtime owners
  - this keeps the remaining Package B work on stable file boundaries and makes
    the frozen Python comparison shell easier to audit

  rollback boundary after this change:
  - there is no transport/gateway rollback change in this slice
  - rollback is file-level only: repoint the write-workflow file back to the
    previous inline existing-background query chain if the new dedicated query
    owner introduces a regression

- 2026-06-07 chapter_single_generation stream entry/lifecycle owner split checkpoint:
  this round stayed on Package B `chapter_single_generation` and continued the
  whole-file migration path inside the single-generation stream lane.

  package map for this checkpoint:
  - Python source map:
    - `backend/app/api/chapter_generation_routes.py`
    - `backend/app/services/chapter_generation_route_compat_service.py`
      as the frozen Python-side comparison boundary
  - Rust target map:
    - `backend-rs/src/api/chapter_generation_routes.rs`
    - `backend-rs/src/services/chapter_single_generation_stream_entry_service.rs`
    - `backend-rs/src/services/chapter_single_generation_stream_workflow_service.rs`
    - `backend-rs/src/services/chapter_single_generation_prepare_service.rs`
    - `backend-rs/src/services/chapter_single_generation_runtime_state_service.rs`
    - `backend-rs/src/services/mod.rs`

  behavior contract kept stable:
  - same single-chapter stream generation route remains Rust-owned:
    - `POST /chapters/{chapter_id}/generate-stream`
  - same route payload contract remains unchanged
  - same runtime launch input preparation boundary, SSE progress cadence,
    success event ordering, follow-up analysis trigger, and failure shell
    semantics remain unchanged
  - no stream response payload shape, quality-gate payload, or rollback
    behavior was intentionally changed in this slice

  implementation boundary for this checkpoint:
  - added new dedicated route-facing owner:
    `backend-rs/src/services/chapter_single_generation_stream_entry_service.rs`
  - that new file now owns:
    - route payload -> request normalization handoff
    - request -> runtime launch input materialization
    - handoff into the stream lifecycle owner
    - focused entry-owner contract tests
  - the surviving lifecycle file
    `backend-rs/src/services/chapter_single_generation_stream_workflow_service.rs`
    now keeps only:
    - runtime launch input -> lifecycle spawn
    - progress / success / failure SSE emission
    - success artifacts / ordered completion payload ownership
  - route imports and service module registration now point at the explicit
    stream-entry owner instead of treating the lifecycle file as a mixed
    entry+lifecycle boundary

  this is counted as real migration progress because it restores one explicit
  Rust owner map inside the single-generation stream lane instead of preserving
  a mixed file boundary:
  - route/public shell -> stream entry owner -> stream lifecycle owner
  - this makes the remaining Python fallback shell easier to audit and keeps
    future module-level Package B work on stable file boundaries

  rollback boundary after this change:
  - there is no transport/gateway rollback change in this slice
  - rollback is file-level only: repoint the route import back to the previous
    mixed stream owner if the new entry/lifecycle split introduces a regression

- 2026-06-07 chapter_single_generation background write workflow owner canonicalization checkpoint:
  this round switched back from route-group cutover to Package B
  `chapter_single_generation` and used a whole-file migration unit instead of
  another helper-scale seam.

  package map for this checkpoint:
  - Python source map:
    - `backend/app/api/chapter_generation_routes.py`
    - `backend/app/services/chapter_generation_route_compat_service.py`
      as the frozen Python-side comparison boundary
  - Rust target map:
    - `backend-rs/src/api/chapter_generation_routes.rs`
    - `backend-rs/src/services/chapter_single_generation_write_workflow_service.rs`
    - `backend-rs/src/services/chapter_single_generation_prepare_service.rs`
    - `backend-rs/src/services/chapter_single_generation_runtime_state_service.rs`
    - `backend-rs/src/services/mod.rs`

  behavior contract kept stable:
  - same single-chapter background generation route remains Rust-owned:
    - `POST /chapters/{chapter_id}/generate-background`
  - same route payload contract remains unchanged
  - same existing-background task short-circuit payload, task creation,
    startup snapshot persistence, and runtime dispatch semantics remain unchanged
  - no background task response shell, checkpoint shape, or rollback behavior
    was intentionally changed in this slice

  implementation boundary for this checkpoint:
  - the previous dedicated file
    `chapter_single_generation_background_write_entry_service.rs`
    was folded back into the canonical module owner name
    `chapter_single_generation_write_workflow_service.rs`
  - route imports and service module registration now point at the canonical
    write-workflow owner directly
  - this is counted as real migration progress because it removes a naming and
    ownership drift between planning artifacts and production code, so the
    next whole-module Package B work can continue on one canonical file owner
    instead of splitting new work across an obsolete entry-shell filename

  rollback boundary after this change:
  - there is no transport/gateway rollback change in this slice
  - rollback is file-level only: restore the previous Rust file/module name and
    route import if the canonicalization caused an integration regression

- 2026-06-07 prompt_workshop, polish, and changelog gateway fallback collapse checkpoint:
  this round continued the route-group cutover path after `careers` and
  `inspiration` and picked the next three whole route-group packages with
  enough owner evidence: `prompt_workshop`, `polish`, and `changelog`.

  package map for this checkpoint:
  - Python source map:
    - `backend/app/api/prompt_workshop.py`
    - `backend/app/api/changelog.py`
    - `backend/app/api/polish.py`
  - Rust target map:
    - `backend-rs/src/api/prompt_workshop.rs`
    - `backend-rs/src/api/changelog.rs`
    - `backend-rs/src/api/polish.rs`
    - `deploy/nginx/mumunovel.conf`
    - `deploy/nginx/mumunovel-docker.conf`
    - `deploy/strangler-gateway-probes.json`
    - `backend/tests/test_tools/test_run_strangler_gateway_smoke.py`
    - route-group ownership / parity / rollback docs

  behavior contract kept stable:
  - same prompt workshop HTTP entrypoints remain Rust-owned:
    - public items / status
    - submit
    - import / download
    - my-submissions
    - admin review / publish / reject / delete
    - like / unlike
  - same polish HTTP entrypoints remain Rust-owned:
    - `POST /api/polish`
    - `POST /api/polish/batch`
    - polish history read-side endpoints
  - same changelog HTTP entrypoints remain Rust-owned:
    - `GET /api/changelog`
    - `POST /api/changelog/refresh`
  - auth-boundary shells and public JSON shells stay unchanged on the Rust path
  - no prompt workshop moderation semantics, polish provider/history semantics,
    or changelog cache/GitHub proxy semantics were intentionally changed in this slice

  this checkpoint is a real Python-to-Rust migration step because it removes
  stale same-path gateway fallback instead of only tightening internal Rust
  helpers:
  - kept `/api/prompt-workshop`, `/api/polish`, and `/api/changelog`
    active paths explicitly Rust-owned in both nginx configs
  - retired active `prompt_workshop`, `polish`, and `changelog`
    Python-fallback smoke probes because same-path Python API execution now
    requires an explicit gateway rollback step
  - updated smoke tests plus architecture/runbook docs to treat rollback as an
    explicit gateway action, not an always-on same-path fallback

  rollback boundary after this change:
  - retarget `/api/prompt-workshop` root and prefix back to Python, then reuse
    the preserved historical Python
    `401 {"detail":"未登录或用户ID缺失"}` submit / like clues
  - retarget `/api/polish` root and prefix back to Python, then reuse the
    preserved historical Python `401 {"detail":"需要登录"}` text / batch clues
  - retarget `/api/changelog` root and prefix back to Python, then reuse the
    preserved historical Python public JSON shell clues:
    - `GET /api/changelog` -> keys `commits`, `cached`, `cache_time`
    - `POST /api/changelog/refresh` -> keys `success`, `message`,
      `commit_count`, `cache_time`

- 2026-06-07 careers and inspiration gateway fallback collapse checkpoint:
  this round continued the route-group cutover path after `organizations`
  and picked the next two whole route-group packages with enough owner
  evidence: `careers` and `inspiration`.

  package map for this checkpoint:
  - Python source map:
    - `backend/app/api/careers.py`
    - `backend/app/api/inspiration.py`
  - Rust target map:
    - `backend-rs/src/api/careers.rs`
    - `backend-rs/src/api/inspiration.rs`
    - `deploy/nginx/mumunovel.conf`
    - `deploy/nginx/mumunovel-docker.conf`
    - `deploy/strangler-gateway-probes.json`
    - `backend/tests/test_tools/test_run_strangler_gateway_smoke.py`
    - route-group ownership / parity / rollback docs

  behavior contract kept stable:
  - same career HTTP entrypoints remain Rust-owned:
    - list
    - root create
    - detail read / update / delete
    - `generate-system`
    - character-career assignment / stage update / removal
  - same inspiration HTTP entrypoints remain Rust-owned:
    - `generate-options`
    - `refine-options`
    - `quick-generate`
  - auth-boundary shells stay unchanged on the Rust path
  - no career payload shape, character-career semantics, inspiration prompt
    shaping, or web-research toggling semantics were intentionally changed
    in this slice

  this checkpoint is a real Python-to-Rust migration step because it removes
  stale same-path gateway fallback instead of only tightening internal Rust
  helpers:
  - kept `/api/careers` and `/api/inspiration` active paths explicitly
    Rust-owned in both nginx configs
  - retired active `careers` and `inspiration` Python-fallback smoke probes
    because same-path Python API execution now requires an explicit gateway
    rollback step
  - updated smoke tests plus architecture/runbook docs to treat rollback as an
    explicit gateway action, not an always-on same-path fallback

  rollback boundary after this change:
  - retarget `/api/careers` root and prefix back to Python, then reuse the
    preserved historical Python `401 {"detail":"未登录"}` list clue and
    `401 {"detail":"需要登录"}` generate-system clue
  - retarget `/api/inspiration` root and prefix back to Python, then reuse the
    preserved historical Python `401 {"detail":"需要登录"}` generate-options /
    quick-generate clues

- 2026-06-07 organizations gateway fallback collapse checkpoint:
  this round continued the route-group cutover path after `writing_styles`
  and picked the next whole route-group package with enough owner evidence:
  `organizations`.

  package map for this checkpoint:
  - Python source map:
    - `backend/app/api/organizations.py`
  - Rust target map:
    - `backend-rs/src/api/organizations.rs`
    - `deploy/nginx/mumunovel.conf`
    - `deploy/nginx/mumunovel-docker.conf`
    - `deploy/strangler-gateway-probes.json`
    - `backend/tests/test_tools/test_run_strangler_gateway_smoke.py`
    - route-group ownership / parity / rollback docs

  behavior contract kept stable:
  - same organization HTTP entrypoints remain Rust-owned:
    - project list
    - detail read
    - root create
    - detail update / delete
    - member list / create / update / delete
    - `generate-stream`
  - auth-boundary shells stay unchanged on the Rust path
  - no organization payload shape, member-management semantics, member-count
    semantics, or generation-history write-side semantics were intentionally
    changed in this slice

  this checkpoint is a real Python-to-Rust migration step because it removes
  stale same-path gateway fallback instead of only tightening internal Rust
  helpers:
  - kept `/api/organizations` root + shared-prefix explicitly Rust-owned in
    both nginx configs
  - retired active `organizations` Python-fallback smoke probes because
    same-path Python API execution now requires an explicit gateway rollback step
  - updated smoke tests plus architecture/runbook docs to treat rollback as an
    explicit gateway action, not an always-on same-path fallback

  rollback boundary after this change:
  - retarget `/api/organizations` root and prefix back to Python
  - use the preserved historical Python `401 {"detail":"未登录"}` project-list
    clue and `401 {"detail":"需要登录"}` generate-stream clue only after that
    explicit gateway rollback step

- 2026-06-07 writing_styles gateway fallback collapse checkpoint:
  this round continued the route-group cutover path after `foreshadows`
  and picked the next whole route-group package with enough owner evidence:
  `writing_styles`.

  package map for this checkpoint:
  - Python source map:
    - `backend/app/api/writing_styles.py`
  - Rust target map:
    - `backend-rs/src/api/writing_styles.rs`
    - `deploy/nginx/mumunovel.conf`
    - `deploy/nginx/mumunovel-docker.conf`
    - `deploy/strangler-gateway-probes.json`
    - `backend/tests/test_tools/test_run_strangler_gateway_smoke.py`
    - route-group ownership / parity / rollback docs

  behavior contract kept stable:
  - same writing-style HTTP entrypoints remain Rust-owned:
    - `presets/list`
    - user styles list
    - project styles list
    - `project/{project_id}/initialize`
    - `project/{project_id}/init-defaults`
    - root create
    - detail read / update / delete
    - `{style_id}/set-default`
  - auth-boundary shells stay unchanged on the Rust path
  - no preset payload shape, user-defined style CRUD semantics, or
    `project_default_styles` write-side semantics were intentionally changed
    in this slice

  this checkpoint is a real Python-to-Rust migration step because it removes
  stale same-path gateway fallback instead of only tightening internal Rust
  helpers:
  - kept `/api/writing-styles` root + shared-prefix explicitly Rust-owned in
    both nginx configs
  - retired active `writing_styles` Python-fallback smoke probes because
    same-path Python API execution now requires an explicit gateway rollback step
  - updated smoke tests plus architecture/runbook docs to treat rollback as an
    explicit gateway action, not an always-on same-path fallback

  rollback boundary after this change:
  - retarget `/api/writing-styles` root and prefix back to Python
  - use the preserved historical Python `401 {"detail":"未登录"}` user / project
    list clues only after that explicit gateway rollback step

- 2026-06-07 foreshadows gateway fallback collapse checkpoint:
  this round continued the route-group cutover path after `relationships`
  and picked the next whole route-group package with enough owner evidence:
  `foreshadows`.

  package map for this checkpoint:
  - Python source map:
    - `backend/app/api/foreshadows.py`
  - Rust target map:
    - `backend-rs/src/api/foreshadows.rs`
    - `deploy/nginx/mumunovel.conf`
    - `deploy/nginx/mumunovel-docker.conf`
    - `deploy/strangler-gateway-probes.json`
    - `backend/tests/test_tools/test_run_strangler_gateway_smoke.py`
    - route-group ownership / parity / rollback docs

  behavior contract kept stable:
  - same foreshadow HTTP entrypoints remain Rust-owned:
    - project list
    - `stats`
    - `context/{chapter_number}`
    - `pending-resolve`
    - detail read
    - create / update / delete
    - `plant`
    - `resolve`
    - `abandon`
    - `sync-from-analysis`
  - auth-boundary shells stay unchanged on the Rust path
  - no foreshadow payload shape, chapter-context query semantics, or
    pending/overdue evaluation semantics were intentionally changed in this slice

  this checkpoint is a real Python-to-Rust migration step because it removes
  stale same-path gateway fallback instead of only tightening internal Rust
  helpers:
  - kept `/api/foreshadows` root + shared-prefix explicitly Rust-owned in both
    nginx configs
  - retired active `foreshadows` Python-fallback smoke probes because same-path
    Python API execution now requires an explicit gateway rollback step
  - updated smoke tests plus architecture/runbook docs to treat rollback as an
    explicit gateway action, not an always-on same-path fallback

  rollback boundary after this change:
  - retarget `/api/foreshadows` root and prefix back to Python
  - use the preserved historical Python `401 {"detail":"未登录"}` project-list /
    stats clues only after that explicit gateway rollback step

- 2026-06-07 relationships gateway fallback collapse checkpoint:
  this round continued the route-group cutover path after `mcp_plugins`
  and picked the next whole route-group package with enough owner evidence:
  `relationships`.

  package map for this checkpoint:
  - Python source map:
    - `backend/app/api/relationships.py`
  - Rust target map:
    - `backend-rs/src/api/relationships.rs`
    - `deploy/nginx/mumunovel.conf`
    - `deploy/nginx/mumunovel-docker.conf`
    - `deploy/strangler-gateway-probes.json`
    - `backend/tests/test_tools/test_run_strangler_gateway_smoke.py`
    - route-group ownership / parity / rollback docs

  behavior contract kept stable:
  - same relationships HTTP entrypoints remain Rust-owned:
    - `types`
    - root create/list
    - `project/{project_id}`
    - `graph/{project_id}`
    - detail read/update/delete
  - auth-boundary shells stay unchanged on the Rust path
  - no relationship payload shape, graph node/link aggregation, or
    organization-member edge semantics were intentionally changed in this slice

  this checkpoint is a real Python-to-Rust migration step because it removes
  stale same-path gateway fallback instead of only tightening internal Rust
  helpers:
  - kept `/api/relationships` root + shared-prefix explicitly Rust-owned in
    both nginx configs
  - retired active `relationships` Python-fallback smoke probes because
    same-path Python API execution now requires an explicit gateway rollback step
  - updated smoke tests plus architecture/runbook docs to treat rollback as an
    explicit gateway action, not an always-on same-path fallback

  rollback boundary after this change:
  - retarget `/api/relationships` root and prefix back to Python
  - use the preserved historical Python `401 {"detail":"未登录"}` project-list /
    graph clues only after that explicit gateway rollback step

- 2026-06-07 mcp_plugins gateway fallback collapse checkpoint:
  this round continued the route-group cutover path after `prompt_templates`
  and picked the next whole route-group package with enough owner evidence:
  `mcp_plugins`.

  package map for this checkpoint:
  - Python source map:
    - `backend/app/api/mcp_plugins.py`
  - Rust target map:
    - `backend-rs/src/api/mcp_plugins.rs`
    - `deploy/nginx/mumunovel.conf`
    - `deploy/nginx/mumunovel-docker.conf`
    - `deploy/strangler-gateway-probes.json`
    - `backend/tests/test_tools/test_run_strangler_gateway_smoke.py`
    - route-group ownership / parity / rollback docs

  behavior contract kept stable:
  - same MCP plugin HTTP entrypoints remain Rust-owned:
    - list / detail / create / update / delete
    - `simple`
    - `toggle`
    - `status`
    - `tools`
    - `call`
    - `test`
    - `metrics`
    - `cache-stats`
    - `session-stats`
    - `cache/clear`
  - auth-boundary shells stay unchanged on the Rust path
  - no plugin payload shape, runtime registration/disconnect semantics, cache
    clearing semantics, or metrics/session payload semantics were intentionally
    changed in this slice

  this checkpoint is a real Python-to-Rust migration step because it removes
  stale same-path gateway fallback instead of only tightening internal Rust
  helpers:
  - kept `/api/mcp` root + shared-prefix explicitly Rust-owned in both nginx
    configs
  - retired active `mcp_plugins` Python-fallback smoke probes because same-path
    Python API execution now requires an explicit gateway rollback step
  - updated smoke tests plus architecture/runbook docs to treat rollback as an
    explicit gateway action, not an always-on same-path fallback

  rollback boundary after this change:
  - retarget `/api/mcp` root and prefix back to Python
  - use the preserved historical Python `401 {"detail":"需要登录"}` list /
    simple-create clues only after that explicit gateway rollback step

- 2026-06-07 prompt_templates gateway fallback collapse checkpoint:
  this round continued the route-group cutover path after `background_tasks`
  and picked the next whole route-group package with enough owner evidence:
  `prompt_templates`.

  package map for this checkpoint:
  - Python source map:
    - `backend/app/api/prompt_templates.py`
  - Rust target map:
    - `backend-rs/src/api/prompt_templates.rs`
    - `deploy/nginx/mumunovel.conf`
    - `deploy/nginx/mumunovel-docker.conf`
    - `deploy/strangler-gateway-probes.json`
    - `backend/tests/test_tools/test_run_strangler_gateway_smoke.py`
    - route-group ownership / parity / rollback docs

  behavior contract kept stable:
  - same prompt template HTTP entrypoints remain Rust-owned:
    - list / detail / create / update / delete
    - `categories`
    - `system-defaults`
    - `sync-status`
    - `sync-to-default`
    - `reset`
    - `export`
    - `import`
    - `preview`
  - auth-boundary shells stay unchanged on the Rust path
  - no prompt template payload shape, managed sync behavior, preview parameter
    substitution, or import/export semantics were intentionally changed in this
    slice

  this checkpoint is a real Python-to-Rust migration step because it removes
  stale same-path gateway fallback instead of only tightening internal Rust
  helpers:
  - kept `/api/prompt-templates` root + shared-prefix explicitly Rust-owned in
    both nginx configs
  - retired active `prompt_templates` Python-fallback smoke probes because
    same-path Python API execution now requires an explicit gateway rollback step
  - updated smoke tests plus architecture/runbook docs to treat rollback as an
    explicit gateway action, not an always-on same-path fallback

  rollback boundary after this change:
  - retarget `/api/prompt-templates` root and prefix back to Python
  - use the preserved historical Python `401 {"detail":"未登录"}` list /
    system-defaults clues only after that explicit gateway rollback step

- 2026-06-07 background_tasks gateway fallback collapse checkpoint:
  this round continued the route-group cutover path after `memories` and
  picked the next whole route-group package with enough owner evidence:
  `background_tasks`.

  package map for this checkpoint:
  - Python source map:
    - `backend/app/api/background_tasks.py`
  - Rust target map:
    - `backend-rs/src/api/background_tasks.rs`
    - `deploy/nginx/mumunovel.conf`
    - `deploy/nginx/mumunovel-docker.conf`
    - `deploy/strangler-gateway-probes.json`
    - `backend/tests/test_tools/test_run_strangler_gateway_smoke.py`
    - route-group ownership / parity / rollback docs

  behavior contract kept stable:
  - same background task HTTP entrypoints remain Rust-owned:
    - root create/list
    - detail read
    - `stream`
    - `cancel`
    - `workflow-state`
  - auth-boundary shells stay unchanged on the Rust path
  - no task payload shape, task registry lifecycle, SSE keep-alive, or workflow
    state semantics were intentionally changed in this slice

  this checkpoint is a real Python-to-Rust migration step because it removes
  stale same-path gateway fallback instead of only tightening internal Rust
  helpers:
  - kept `/api/background-tasks` exact-root + shared-prefix explicitly Rust-owned
    in both nginx configs
  - retired active `background_tasks` Python-fallback smoke probes because
    same-path Python API execution now requires an explicit gateway rollback step
  - updated smoke tests plus architecture/runbook docs to treat rollback as an
    explicit gateway action, not an always-on same-path fallback

  rollback boundary after this change:
  - retarget `/api/background-tasks` root and prefix back to Python
  - keep SSE proxy behavior explicit while doing that rollback
  - use the preserved historical Python `401 {"detail":"Unauthorized"}` list/create
    clues only after that explicit gateway rollback step

- 2026-06-07 memories gateway fallback collapse checkpoint:
  this round continued the route-group cutover path after `projects` and
  picked the next whole route-group package with enough owner evidence:
  `memories`.

  package map for this checkpoint:
  - Python source map:
    - `backend/app/api/memories.py`
  - Rust target map:
    - `backend-rs/src/api/memories.rs`
    - `deploy/nginx/mumunovel.conf`
    - `deploy/nginx/mumunovel-docker.conf`
    - `deploy/strangler-gateway-probes.json`
    - `backend/tests/test_tools/test_run_strangler_gateway_smoke.py`
    - route-group ownership / parity / rollback docs

  behavior contract kept stable:
  - same memories HTTP entrypoints remain Rust-owned on `/api/memories/*`:
    - `projects/{project_id}/analyze-chapter/{chapter_id}`
    - `projects/{project_id}/memories`
    - `projects/{project_id}/analysis/{chapter_id}`
    - `projects/{project_id}/search`
    - `projects/{project_id}/foreshadows`
    - `projects/{project_id}/stats`
    - `projects/{project_id}/chapters/{chapter_id}/memories`
  - `/memories/*` remains a Python page / non-API boundary and was not merged
    into the API cutover scope
  - auth-boundary shells and route payload contracts stay unchanged on the
    Rust API path

  this checkpoint is a real Python-to-Rust migration step because it removes
  stale same-path gateway fallback instead of only tightening internal Rust
  helpers:
  - kept `/api/memories*` explicitly Rust-owned in both nginx configs
  - corrected docker nginx drift so `/memories*` keeps pointing to Python
    instead of accidentally leaking the non-API boundary into Rust
  - retired active `memories` Python-fallback smoke probes because same-path
    Python API execution now requires an explicit gateway rollback step
  - updated smoke tests plus architecture/runbook docs to treat rollback as an
    explicit gateway action, not an always-on same-path fallback

  rollback boundary after this change:
  - only retarget `/api/memories/` and the analyze-chapter location when
    restoring Python API ownership
  - do not retarget `/memories/`, because it remains the page/non-API Python
    boundary
  - use the preserved historical Python `401 {"detail":"未登录"}` memories
    clues only after that explicit gateway rollback step

- 2026-06-07 projects gateway fallback collapse checkpoint:
  this round continued the route-group cutover path after `settings` and
  picked the next whole route-group package with enough owner evidence:
  `projects`.

  package map for this checkpoint:
  - Python source map:
    - `backend/app/api/projects.py`
  - Rust target map:
    - `backend-rs/src/api/projects.rs`
    - `deploy/nginx/mumunovel.conf`
    - `deploy/nginx/mumunovel-docker.conf`
    - `deploy/strangler-gateway-probes.json`
    - `backend/tests/test_tools/test_run_strangler_gateway_smoke.py`
    - route-group ownership / parity / rollback docs

  behavior contract kept stable:
  - same projects HTTP entrypoints remain Rust-owned:
    - root create/list
    - detail read/update/delete
    - `export`
    - `export-data`
    - `validate-import`
    - `import`
    - `check-consistency`
    - `fix-organizations`
    - `fix-member-counts`
  - auth-boundary and public import-validation payload shells stay unchanged on
    the Rust path
  - no user-facing project schema, import/export contract, or maintenance
    workflow semantics were intentionally changed in this slice

  this checkpoint is a real Python-to-Rust migration step because it removes
  stale same-path gateway fallback instead of only tightening internal Rust
  helpers:
  - collapsed scattered `/api/projects*` nginx rules into one Rust-owned root
    + shared-prefix owner in both nginx configs to stop per-path drift
  - retired active `projects` Python-fallback smoke probes because same-path
    Python execution now requires an explicit gateway rollback step
  - updated smoke tests plus architecture/runbook docs to treat rollback as an
    explicit gateway action, not an always-on same-path fallback

  rollback boundary after this change:
  - delete or retarget the Rust `/api/projects/` prefix rule and root rule,
    then let the Python `/api/` catch-all or a temporary Python prefix take
    over
  - use the preserved historical Python `validate-import` public-success clue
    plus `401 {"detail":"未登录"}` create/read/write/maintenance clues only
    after that explicit rollback step

- 2026-06-07 settings gateway fallback collapse checkpoint:
  this round continued the route-group cutover path after `wizard-stream` and
  picked the next whole route-group package with enough owner evidence:
  `settings`.

  package map for this checkpoint:
  - Python source map:
    - `backend/app/api/settings.py`
  - Rust target map:
    - `backend-rs/src/api/settings.rs`
    - `deploy/nginx/mumunovel.conf`
    - `deploy/nginx/mumunovel-docker.conf`
    - `deploy/strangler-gateway-probes.json`
    - `backend/tests/test_tools/test_run_strangler_gateway_smoke.py`
    - route-group ownership / parity / rollback docs

  behavior contract kept stable:
  - same settings HTTP entrypoints remain Rust-owned:
    - root CRUD
    - `api-key`
    - `models`
    - `fetch-models`
    - `test`
    - `test-web-research`
    - `check-function-calling`
    - preset read/write/activate/test/from-current chain
  - auth-boundary and logged-in business payload shells stay unchanged on the
    Rust path
  - no user-facing schema, preset storage, or provider-default contract was
    intentionally changed in this slice

  this checkpoint is a real Python-to-Rust migration step because it removes
  stale same-path gateway fallback instead of only tightening internal Rust
  helpers:
  - collapsed scattered `/api/settings*` nginx rules into one Rust-owned root
    + shared-prefix owner in both nginx configs to stop per-path drift
  - retired active `settings` Python-fallback / business-fallback /
    models-asymmetric smoke probes because same-path Python execution now
    requires an explicit gateway rollback step
  - updated smoke tests plus architecture/runbook docs to treat rollback as an
    explicit gateway action, not an always-on same-path fallback

  rollback boundary after this change:
  - delete or retarget the Rust `/api/settings/` prefix rule and root rule,
    then let the Python `/api/` catch-all or a temporary Python prefix take
    over
  - use the preserved historical Python `401 {"detail":"需要登录"}` and
    `settings/models` public-network-error clues only after that explicit
    rollback step

- 2026-06-07 wizard-stream gateway fallback collapse checkpoint:
  this round switched from low-yield chapter-internal seam hunting to one
  real route-group cutover package: `wizard-stream`.

  package map for this checkpoint:
  - Python source map:
    - `backend/app/api/wizard_stream.py`
  - Rust target map:
    - `backend-rs/src/api/wizard.rs`
    - `deploy/nginx/mumunovel.conf`
    - `deploy/nginx/mumunovel-docker.conf`
    - `deploy/strangler-gateway-probes.json`
    - `backend/tests/test_tools/test_run_strangler_gateway_smoke.py`
    - route-group ownership / parity / rollback docs

  behavior contract kept stable:
  - same `wizard-stream` SSE entrypoints remain Rust-owned:
    - `world-building`
    - `world-building/{project_id}/regenerate`
    - `career-system`
    - `characters`
    - `outline`
    - `cleanup/{project_id}`
  - auth-boundary expectations stay unchanged on the Rust path
  - no user-facing payload or SSE shape changes were introduced in this slice

  this checkpoint is a real Python-to-Rust migration step because it removes
  stale same-path gateway fallback instead of only tightening internal Rust
  helpers:
  - removed the legacy `/api/wizard/` nginx rule because there is no
    independent Python route group behind it
  - collapsed `wizard-stream` routing to one Rust-owned prefix in both nginx
    configs to stop per-path drift
  - retired stale `wizard-stream` Python-fallback smoke probes and updated the
    smoke tests plus architecture/runbook docs to treat rollback as an
    explicit gateway action, not an always-on same-path fallback

  rollback boundary after this change:
  - delete or retarget the Rust `/api/wizard-stream/` prefix rule and let the
    Python `/api/` catch-all or a temporary Python prefix take over
  - use the preserved historical Python `401 {"detail":"需要登录"}` clues only
    after that explicit rollback step

- 2026-06-07 single-generation background-stream launch-wrapper checkpoint:
  this slice stayed on Package B, `chapter_single_generation`, and removed
  two remaining production launch-wrapper hops around the surviving
  background-write and stream lifecycle owners.

  before this change, the single-generation package already had real
  surviving owners for:
  - `SingleGenerationBackgroundWriteWorkflowEntry`
    for existing-task short-circuit vs launch-branch selection
  - `SingleGenerationStreamLifecyclePlan`
    for stream lifecycle spawn/run orchestration

  but the production lane still preserved two extra forwarding helpers:
  - `start_owned_single_generation_background_launch(...)`
  - `start_owned_single_generation_stream_lifecycle(...)`

  those helpers no longer owned validation, transport branching, fallback
  behavior, rollback seams, or distinct response contracts. They only replayed:
  - `launch parts -> persist and dispatch`
  - `runtime launch -> lifecycle spawn`

  this checkpoint tightens the single-generation owner boundary further:
  - `SingleGenerationBackgroundWriteWorkflowEntry::persist_and_dispatch(...)`
    now dispatches the launch branch directly into the surviving background
    persistence helper without preserving a second start wrapper name
  - the launch persistence helper is now explicitly named
    `persist_owned_single_generation_background_launch(...)` to reflect its
    real owner role
  - `create_owned_single_generation_stream(...)`
    now spawns the lifecycle directly through:
    `SingleGenerationStreamLifecyclePlan::from_runtime_launch(...).spawn(db)`
    instead of preserving a second public `start_*lifecycle(...)` hop
  - focused stream/background tests now assert the surviving owner contract
    directly instead of a wrapper-level start helper

  This is a real Phase 5 migration step because Rust now owns one tighter
  single-generation launch chain rather than preserving two compatibility
  wrappers that add no new behavior:
  - `route payload -> background workflow owner -> launch persist/dispatch`
  - `route payload -> stream workflow owner -> lifecycle spawn/run`

  The remaining Python dependency is unchanged in this slice: route payload
  shape, SSE payloads, fallback shells, task lifecycle, provider defaults, and
  rollback boundaries remain stable.

  Focused validation passed with:
  `cargo fmt --manifest-path "backend-rs/Cargo.toml" -- "backend-rs/src/services/chapter_single_generation_background_write_entry_service.rs" "backend-rs/src/services/chapter_single_generation_stream_workflow_service.rs"`
  `cargo test chapter_single_generation_background_write_entry_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-launch-wrapper-collapse" -- --nocapture`
  `cargo test chapter_single_generation_stream_workflow_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-launch-wrapper-collapse" -- --nocapture`
  `cargo test chapter_single_generation --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-launch-wrapper-collapse" -- --nocapture`
  `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-launch-wrapper-collapse-check"`

- 2026-06-07 batch-generation create-launch persistence-owner checkpoint:
  this slice stayed on Package C, `chapter_batch_generation`, and continued
  shrinking the batch write lane by collapsing one remaining create-side
  launch wrapper back into the real create persistence owner.

  before this change, the batch create chain already had two meaningful
  owners:
  - `PreparedBatchGenerationCreateWorkflowLaunch`
    for request/runtime preparation, effective style resolution, startup
    snapshot planning, and runtime launch-input materialization
  - `BatchGenerationCreateLaunchPersistencePlan`
    for task persistence, startup snapshot persistence, response payload
    ownership, and runtime dispatch

  but the create lane still preserved an extra compatibility hop between those
  two owners:
  - `PreparedBatchGenerationCreateWorkflowLaunch::prepare_persistence_plan(...)`
  - `PreparedBatchGenerationCreateWorkflowLaunch::into_persistence_plan(...)`
  - `PreparedBatchGenerationCreateWorkflowPersistenceParts`

  that layer no longer owned a route boundary, validation branch, rollback
  seam, error translation shell, or distinct persistence contract. It only
  replayed:
  - `prepare workflow launch -> build persistence parts`
  - `persistence parts -> create persistence plan`

  this checkpoint tightens the batch create owner boundary further:
  - surviving create owners:
    - `PreparedBatchGenerationCreateWorkflowLaunch`
    - `BatchGenerationCreateLaunchPersistencePlan`
    - `start_owned_batch_generation_write_workflow(...)`
  - `BatchGenerationCreateLaunchPersistencePlan::prepare(...)`
    now calls the workflow-launch owner directly and materializes the final
    persistence owner without a second wrapper hop
  - `start_owned_batch_generation_write_workflow(...)`
    now consumes `BatchGenerationCreateRouteRequest` directly, so the batch
    create route no longer depends on a second
    `start_owned_batch_generation_write_workflow_from_route_payload(...)`
    forwarding shell
  - `BatchGenerationCreateLaunchPersistencePlan::from_workflow_launch(...)`
    now directly owns:
    - create response payload materialization
    - create task-seed projection
    - startup snapshot plan handoff
    - runtime input handoff
  - removed shell-only layer:
    - `PreparedBatchGenerationCreateWorkflowPersistenceParts`
    - `PreparedBatchGenerationCreateWorkflowLaunch::prepare_persistence_plan(...)`
    - `PreparedBatchGenerationCreateWorkflowLaunch::into_persistence_plan(...)`
    - `start_owned_batch_generation_write_workflow_from_route_payload(...)`
  - focused tests now assert the surviving persistence owner contract directly
    instead of treating workflow-launch -> persistence-parts as an independent
    production boundary

  This is a real Phase 5 migration step because Rust now owns one tighter
  batch create write chain instead of preserving a create-side wrapper that
  adds no new behavior:
  - `create request/runtime preparation -> create persistence owner ->
    persist snapshot/task -> dispatch runtime`

  The remaining Python dependency is unchanged in this slice: HTTP payload
  shape, SSE payloads, task lifecycle, provider defaults, gateway fallback,
  and rollback boundaries remain stable.

  Focused validation passed with:
  `cargo fmt --manifest-path "backend-rs/Cargo.toml" -- "backend-rs/src/services/chapter_batch_generation_write_workflow_service.rs"`
  `cargo fmt --manifest-path "backend-rs/Cargo.toml" -- "backend-rs/src/api/chapter_batch_generation.rs"`
  `cargo test chapter_batch_generation_write_workflow_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-create-launch-persistence-owner" -- --nocapture`
  `cargo test chapter_batch_generation --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-create-route-start-collapse" -- --nocapture`
  `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-create-launch-persistence-owner-check"`

- 2026-06-07 batch-generation resume-cancel launch-wrapper checkpoint:
  this slice stayed on Package C, `chapter_batch_generation`, and collapsed
  the remaining resume/cancel public-start wrapper hop in the batch write
  lane back into the real persistence owners.

  before this change, the batch write lane already had explicit surviving
  owners for:
  - `BatchGenerationResumeLaunchPersistencePlan`
  - `BatchGenerationCancelledPersistencePlan`

  but the production write path still preserved one more compatibility hop:
  - `PreparedBatchGenerationResumeWorkflowLaunch`
  - `PreparedBatchGenerationCancelWorkflowLaunch`

  those wrappers no longer owned validation, branch selection, transport
  cutover, rollback seam, or persistence semantics. They only replayed:
  - `prepare owned resume -> persist_and_dispatch`
  - `prepare owned cancel -> persist`

  this checkpoint tightened the write owner boundary:
  - `resume_owned_batch_generation_write_workflow(...)`
    now calls
    `prepare_owned_batch_generation_resume(...).persist_and_dispatch(db)`
    directly
  - `cancel_owned_batch_generation_write_workflow(...)`
    now calls
    `prepare_owned_batch_generation_cancel_workflow(...).persist(db)` directly
  - focused tests now assert persistence-owner contracts directly instead of
    depending on wrapper-level `.persistence_plan()` behavior

  This is a real Phase 5 migration step because Rust now owns one tighter
  batch write owner chain across create/resume/cancel instead of preserving
  two pure forwarding launch wrappers.

  The remaining Python dependency is unchanged in this slice: HTTP payload,
  task lifecycle, checkpoint shape, fallback shell, and rollback boundary all
  remain stable.

  Focused validation passed with:
  `cargo test chapter_batch_generation_write_workflow_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-write-launch-wrapper-collapse" -- --nocapture`
  `cargo test chapter_batch_generation_resume_task_command_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-write-launch-wrapper-collapse" -- --nocapture`
  `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-write-launch-wrapper-collapse-check"`

- 2026-06-07 batch-generation status-semantics file-collapse checkpoint:
  this slice stayed on Package C, `chapter_batch_generation`, and collapsed
  one remaining status-semantics facade file back into the real batch
  task-payload base owner. Before this change, the batch module already had
  one surviving payload/value-contract owner for:
  - create/resume task response payload assembly
  - active task / active project / existing background task view payloads
  - retry/terminal metadata injection
  - loading-stage compatibility field projection

  but one neighboring file still sat beside that owner for lower-level shared
  batch status semantics:
  - `backend-rs/src/services/chapter_batch_generation_status_semantics_service.rs`

  that file no longer owned an independent route boundary, fallback shell,
  rollback seam, persistence transport, or semantic branch. It only replayed:
  - `BatchGenerationTaskKind`
  - `active_batch_generation_statuses()`
  - task kind / task type / stage-code mapping
  - execution-mode projection

  while every production consumer already treated those semantics as part of
  the same shared batch payload / read / runtime owner chain.

  this checkpoint tightens that shared owner boundary further:
  - surviving owner file:
    - `backend-rs/src/services/chapter_batch_generation_task_payload_base_service.rs`
  - the surviving payload-base owner now directly owns:
    - `BatchGenerationTaskKind`
    - `active_batch_generation_statuses()`
    - `batch_generation_task_kind(...)`
    - `task_kind(...)`
    - `batch_generation_task_type(...)`
    - `task_type(...)`
    - `batch_generation_stage_code(...)`
    - `task_execution_mode()`
    - focused batch status-semantics regression tests beside the surviving
      payload/value-contract owner
  - `backend-rs/src/services/chapter_batch_generation_read_context_service.rs`
    now imports active status / task-type semantics directly from the
    surviving payload-base owner
  - `backend-rs/src/services/chapter_batch_generation_resume_task_command_service.rs`
    now imports active status semantics directly from the surviving
    payload-base owner
  - `backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs`
    now imports task kind / stage-code semantics directly from the surviving
    payload-base owner
  - `backend-rs/src/services/chapter_batch_generation_write_workflow_service.rs`
    now imports shared batch task-type / execution-mode semantics directly
    from the surviving payload-base owner
  - `backend-rs/src/services/mod.rs`
    now drops the shell-only batch status-semantics module registration
  - deleted shell file:
    - `backend-rs/src/services/chapter_batch_generation_status_semantics_service.rs`

  This is a real Phase 5 migration step because Rust now owns one tighter
  batch payload/value-contract chain instead of preserving a separate
  status-semantics facade that adds no new validation layer, no route-local
  business boundary, and no independent rollback seam:
  - `shared batch payload owner -> task kind/type/status semantics -> read/runtime/write consumers`

  The remaining Python dependency is unchanged in this slice: HTTP payload
  shape, SSE payloads, task lifecycle semantics, provider defaults, and
  gateway fallback shells remain stable. The rollback boundary remains the
  existing gateway/Python fallback shell because no route ownership or
  transport cutover changed.

  Focused validation passed with:
  `cargo fmt --manifest-path "backend-rs/Cargo.toml" -- "backend-rs/src/services/chapter_batch_generation_task_payload_base_service.rs" "backend-rs/src/services/chapter_batch_generation_read_context_service.rs" "backend-rs/src/services/chapter_batch_generation_resume_task_command_service.rs" "backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs" "backend-rs/src/services/chapter_batch_generation_write_workflow_service.rs" "backend-rs/src/services/mod.rs"`
  `cargo test chapter_batch_generation_task_payload_base_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-status-semantics-file-collapse-payload" -- --nocapture`
  `cargo test chapter_batch_generation_runtime_state_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-status-semantics-file-collapse-runtime" -- --nocapture`
  `cargo test chapter_batch_generation_resume_task_command_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-status-semantics-file-collapse-resume" -- --nocapture`
  `cargo test chapter_batch_generation_read_context_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-status-semantics-file-collapse-read" -- --nocapture`
  `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-status-semantics-file-collapse-check"`

- 2026-06-07 batch-generation resume-semantics file-collapse checkpoint:
  this slice stayed on Package C, `chapter_batch_generation`, and collapsed
  one remaining resume-semantics facade file back into the real batch
  runtime-state owner. Before this change, the batch resume lane already had
  two real surviving owners:
  - `chapter_batch_generation_runtime_state_service.rs`
    for resume checkpoint projection, restored runtime-state recovery, reset
    persistence planning, and resume runtime launch materialization
  - `chapter_batch_generation_resume_task_command_service.rs`
    for status/manual-review gating, access/prerequisite validation, and final
    `prepare owned resume -> persist-and-dispatch` command flow

  but one neighboring file still sat between those two owners for lower-level
  shared resume task semantics:
  - `backend-rs/src/services/chapter_batch_generation_resume_semantics_service.rs`

  that file no longer owned an independent route boundary, fallback shell,
  rollback seam, transport contract, or separate persistence owner. It only
  replayed:
  - `ResumeBatchGenerationCommandState`
  - `ResumeRuntimeSemantics`
  - `ResumeResetSemantics`
  - `ResumeExecutionSelection`
  - resumable chapter-id parsing / selection
  - reset checkpoint projection helpers

  while every production consumer already treated those semantics as part of
  the runtime/reset owner chain or the command validation/dispatch chain.

  this checkpoint tightens the batch resume owner boundary further:
  - surviving owner file:
    - `backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs`
  - the surviving runtime owner now directly owns:
    - `ResumeBatchGenerationCommandState`
    - `ResumeRuntimeSemantics`
    - `ResumeResetSemantics`
    - `ResumeExecutionSelection`
    - `ResolveResumeExecutionSelectionError`
    - resumable batch chapter-id parsing / selection
    - resume checkpoint-with-seed projection
    - focused resume semantics regression tests beside the runtime owner
  - `backend-rs/src/services/chapter_batch_generation_resume_task_command_service.rs`
    now imports resume command state / selection / reset semantics directly
    from the surviving runtime owner
  - `backend-rs/src/services/chapter_batch_generation_write_workflow_service.rs`
    test-side resume workflow launch helpers now import
    `ResumeBatchGenerationCommandState` directly from the surviving runtime
    owner
  - `backend-rs/src/services/mod.rs`
    now drops the shell-only batch resume-semantics module registration
  - deleted shell file:
    - `backend-rs/src/services/chapter_batch_generation_resume_semantics_service.rs`

  This is a real Phase 5 migration step because Rust now owns one tighter
  batch resume chain instead of preserving a separate file-local semantics
  facade that adds no new validation, no route-local business boundary, and
  no independent rollback seam:
  - `resume task projection -> runtime/reset semantics -> restored resume launch / reset persistence -> command persist-and-dispatch`

  The remaining Python dependency is unchanged in this slice: HTTP payload
  shape, SSE payloads, task lifecycle semantics, quality-gate shells, provider
  defaults, and gateway fallback shells remain stable. The rollback boundary
  remains the existing gateway/Python fallback shell because no route cutover
  or transport ownership changed.

  Focused validation passed with:
  `cargo fmt --manifest-path "backend-rs/Cargo.toml" -- "backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs" "backend-rs/src/services/chapter_batch_generation_resume_task_command_service.rs" "backend-rs/src/services/chapter_batch_generation_write_workflow_service.rs" "backend-rs/src/services/mod.rs"`
  `cargo test chapter_batch_generation_runtime_state_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-resume-semantics-file-collapse-runtime" -- --nocapture`
  `cargo test chapter_batch_generation_resume_task_command_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-resume-semantics-file-collapse-resume" -- --nocapture`
  `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-resume-semantics-file-collapse-check"`

- 2026-06-07 chapter-generation snapshot-owner file-collapse checkpoint:
  this slice stayed on Package A, `chapter_generation`, and collapsed one
  remaining shared snapshot query facade file back into the real shared
  chapter-generation snapshot owner. Before this change, the shared snapshot
  lane already had one real write-side owner for:
  - runtime snapshot merge vs replace semantics
  - persisted quality field backfill from runtime state
  - final snapshot upsert / persistence

  but one neighboring file still sat beside that owner for the lower-level
  read/query half of the same snapshot contract:
  - `backend-rs/src/services/chapter_generation_snapshot_query_service.rs`

  that file no longer owned an independent route boundary, fallback shell,
  rollback seam, semantic branch, or schema owner. It only replayed:
  - `load_chapter_generation_snapshot(...)`
  - `load_chapter_generation_snapshot_map(...)`

  while every production consumer already treated those read helpers and the
  snapshot write helpers as one shared chapter-generation snapshot chain.

  this checkpoint tightens that shared owner boundary further:
  - added surviving shared owner file:
    - `backend-rs/src/services/chapter_generation_snapshot_service.rs`
  - the surviving shared owner now directly owns:
    - `load_chapter_generation_snapshot(...)`
    - `load_chapter_generation_snapshot_map(...)`
    - `merge_chapter_generation_runtime_state(...)`
    - `ChapterGenerationSnapshotWriteMode`
    - `persist_chapter_generation_runtime_snapshot(...)`
    - `upsert_chapter_generation_runtime_snapshot(...)`
    - focused snapshot merge / replace / backfill regression tests
  - `chapter_batch_generation_owned_task_query_service.rs`
    now consumes shared snapshot reads directly from the surviving owner
  - `chapter_batch_generation_read_context_service.rs`
    now consumes shared snapshot map reads directly from the surviving owner
  - `chapter_batch_generation_runtime_state_service.rs`
    now consumes both shared snapshot reads and writes directly from the
    surviving owner
  - `chapter_single_generation_background_write_entry_service.rs`
    now consumes shared snapshot map reads directly from the surviving owner
  - `chapter_single_generation_prepare_service.rs`
    and `chapter_single_generation_runtime_state_service.rs`
    now consume shared snapshot writes directly from the surviving owner
  - `backend-rs/src/services/mod.rs`
    now drops the split registration for:
    - `chapter_generation_snapshot_persistence_service`
    - `chapter_generation_snapshot_query_service`
    and keeps only:
    - `chapter_generation_snapshot_service`
  - deleted shell file:
    - `backend-rs/src/services/chapter_generation_snapshot_query_service.rs`

  This is a real Phase 5 migration step because Rust now owns one tighter
  shared snapshot chain instead of preserving a second file-local query facade
  for the same persisted snapshot owner:
  - `shared snapshot read -> shared snapshot merge/backfill -> shared snapshot write`

  The remaining Python dependency is unchanged in this slice: route payloads,
  SSE payloads, task lifecycle semantics, provider defaults, and gateway
  fallback shells are preserved. The rollback boundary remains the existing
  gateway/Python fallback shell because no route ownership or transport cutover
  changed.

  Focused validation passed with:
  `cargo fmt --manifest-path "backend-rs/Cargo.toml" -- "backend-rs/src/services/chapter_generation_snapshot_service.rs" "backend-rs/src/services/chapter_batch_generation_owned_task_query_service.rs" "backend-rs/src/services/chapter_batch_generation_read_context_service.rs" "backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs" "backend-rs/src/services/chapter_single_generation_background_write_entry_service.rs" "backend-rs/src/services/chapter_single_generation_prepare_service.rs" "backend-rs/src/services/chapter_single_generation_runtime_state_service.rs" "backend-rs/src/services/mod.rs"`
  `cargo test chapter_generation_snapshot_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/chapter-generation-snapshot-owner-collapse" -- --nocapture`
  `cargo test chapter_batch_generation_read_context_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/chapter-generation-snapshot-owner-collapse" -- --nocapture`
  `cargo test chapter_single_generation_prepare_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/chapter-generation-snapshot-owner-collapse" -- --nocapture`
  `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/chapter-generation-snapshot-owner-collapse-check"`

- 2026-06-07 batch-generation task-view-query file-collapse checkpoint:
  this slice stayed on Package C, `chapter_batch_generation`, and collapsed
  one remaining task-view query facade file back into the real batch
  read-context owner. Before this change, the batch active-query lane had
  already converged on one coherent read-side owner for:
  - owned `task + snapshot -> BatchGenerationReadContext` projection
  - active-task-list item payload projection
  - active-project task payload projection
  - owned status payload projection

  but one neighboring file still sat between that read owner and the final
  active query / route-query contract:
  - `backend-rs/src/services/chapter_batch_generation_task_view_query_service.rs`

  that file no longer owned an independent persistence seam, fallback shell,
  rollback boundary, snapshot source chain, or semantic branch. It only
  replayed:
  - `ActiveBatchGenerationTaskListRouteQuery`
  - active-task-list limit normalization
  - active-task-list route-query error shell
  - active-project route-query error shell
  - direct active task row loading
  - final active-task-list / active-project payload wrappers

  this checkpoint tightens that batch read-side owner boundary further:
  - `backend-rs/src/services/chapter_batch_generation_read_context_service.rs`
    now directly owns:
    - `ActiveBatchGenerationTaskListRouteQuery`
    - `ActiveBatchGenerationTaskListQueryRequest`
    - `ActiveBatchGenerationTaskListQueryRequestError`
    - `ActiveBatchGenerationTaskListRouteQueryError`
    - `ActiveProjectBatchGenerationRouteError`
    - direct active batch task row loading
    - active-task-list read start
    - active-project read start
    - final active-task-list / active-project payload wrappers
    - focused route-query bound / payload wrapper regression tests beside the
      surviving read owner
  - `backend-rs/src/api/chapter_batch_generation.rs`
    now imports the active-task-list query DTO and active-query route owners
    directly from the read-context owner
  - `backend-rs/src/api/chapter_batch_generation_error_mapper.rs`
    now imports the active-query route error shells directly from the
    read-context owner
  - `backend-rs/src/services/mod.rs`
    now drops the shell-only batch task-view query module registration
  - deleted shell file:
    - `backend-rs/src/services/chapter_batch_generation_task_view_query_service.rs`

  This is a real Phase 5 migration step because Rust now owns one tighter
  batch read/query chain instead of preserving a separate task-view query
  facade that adds no new validation layer, no route-local business owner,
  and no independent rollback seam:
  - `active task row load -> read-context projection -> active payload/view response`

  The remaining Python dependency is unchanged in this slice: HTTP payload
  shape, query bounds, project-access error semantics, task lifecycle, SSE
  payloads, provider defaults, and gateway fallback shell are preserved. The
  rollback boundary remains the existing gateway/Python fallback shell because
  no external route contract or transport cutover changed.

  Focused validation passed with:
  `rustfmt --edition 2021 "backend-rs/src/services/chapter_batch_generation_read_context_service.rs" "backend-rs/src/api/chapter_batch_generation.rs" "backend-rs/src/api/chapter_batch_generation_error_mapper.rs" "backend-rs/src/services/mod.rs"`
  `cargo test chapter_batch_generation_read_context_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-task-view-file-collapse-read-context" -- --nocapture`
  `cargo test chapter_batch_generation --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-task-view-file-collapse-batch" -- --nocapture`
  `cargo test chapter_single_generation_write_workflow_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-task-view-file-collapse-single" -- --nocapture`
  `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-task-view-file-collapse-check"`

- 2026-06-07 batch-generation runtime-checkpoint facade file-collapse checkpoint:
  this slice stayed on Package C, `chapter_batch_generation`, and collapsed
  one remaining runtime-checkpoint facade file back into the real batch
  runtime-state owner. Before this change, the batch runtime lane had already
  converged on one coherent runtime owner for:
  - queued startup snapshot planning
  - resume reset snapshot planning
  - runtime dispatch and lifecycle progression
  - per-step checkpoint persistence
  - failed/cancelled/retry terminal persistence planning

  but one neighboring file still sat between that runtime owner and the
  final checkpoint projection contract:
  - `backend-rs/src/services/chapter_batch_generation_runtime_checkpoint_service.rs`

  that file no longer owned an independent route boundary, fallback shell,
  rollback seam, snapshot write owner, or lifecycle branch. It only replayed:
  - `BatchGenerationFailureKind`
  - `BatchGenerationSnapshotStage`
  - pending/runtime checkpoint payload projection helpers
  - checkpoint progress / failure-message helpers

  this checkpoint tightens that batch runtime owner boundary further:
  - `backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs`
    now directly owns:
    - `BatchGenerationFailureKind`
    - `BatchGenerationSnapshotStage`
    - pending/runtime checkpoint payload projection
    - checkpoint progress / failure-message helpers
    - the focused runtime-checkpoint regression tests that belong beside the
      batch runtime lifecycle owner
  - `backend-rs/src/services/chapter_batch_generation_resume_semantics_service.rs`
    now consumes resume checkpoint stage projection directly from the runtime
    owner instead of reopening the deleted checkpoint facade
  - `backend-rs/src/services/chapter_batch_generation_resume_task_command_service.rs`
    test helpers now consume pending checkpoint projection directly from the
    runtime owner
  - `backend-rs/src/services/mod.rs`
    now drops the shell-only batch runtime-checkpoint module registration
  - deleted shell file:
    - `backend-rs/src/services/chapter_batch_generation_runtime_checkpoint_service.rs`

  This is a real Phase 5 migration step because Rust now owns one tighter
  batch runtime launch -> lifecycle stage -> checkpoint projection /
  persistence chain instead of preserving a separate checkpoint facade that
  adds no validation, no semantic branching, and no independent error
  contract:
  - `batch runtime owner -> lifecycle stage -> checkpoint projection/persistence`

  The remaining Python dependency is unchanged in this slice: route payloads,
  fallback shells, task lifecycle semantics, checkpoint shapes, SSE payloads,
  provider defaults, and rollback shell remain stable. The rollback boundary
  remains the existing gateway/Python fallback shell because no route
  ownership or transport cutover changed.

  Focused validation passed with:
  `rustfmt --edition 2021 "backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs" "backend-rs/src/services/chapter_batch_generation_resume_semantics_service.rs" "backend-rs/src/services/chapter_batch_generation_resume_task_command_service.rs" "backend-rs/src/services/mod.rs"`
  `cargo test chapter_batch_generation_runtime_state_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-runtime-checkpoint-file-collapse-runtime" -- --nocapture`
  `cargo test chapter_batch_generation_resume_task_command_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-runtime-checkpoint-file-collapse-resume" -- --nocapture`
  `cargo test chapter_batch_generation --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-runtime-checkpoint-file-collapse-batch" -- --nocapture`
  `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-runtime-checkpoint-file-collapse-check"`

- 2026-06-07 batch-generation quality-runtime-context facade file-collapse checkpoint:
  this slice stayed on Package C, `chapter_batch_generation`, and collapsed
  one remaining batch-named quality runtime-context facade file back into the
  shared chapter-generation quality owner. Before this change, the batch
  runtime, write-workflow, resume, payload, and quality-status lanes already
  consumed one coherent batch quality runtime-context contract, but that owner
  still lived behind a dedicated batch file:
  - `backend-rs/src/services/chapter_batch_generation_quality_runtime_context_service.rs`

  that file no longer owned an independent route boundary, fallback shell,
  rollback seam, or batch-only persistence transport. It only replayed:
  - `BatchGenerationQualityRuntimeContext`
  - batch summary/history rebuild helpers
  - batch snapshot/runtime-state restore helpers
  - batch payload application helpers
  - batch current-quality append / preserve-existing-quality helpers

  this checkpoint tightens the shared owner boundary further:
  - `backend-rs/src/services/chapter_generation_quality_runtime_context_service.rs`
    now directly owns:
    - `BatchGenerationQualityRuntimeContext` as a shared owner alias
    - batch summary/history rebuild helpers
    - batch snapshot/runtime-state restore helpers
    - batch payload application helpers
    - batch current-quality append / preserve-existing-quality helpers
    - focused batch-scope runtime-context regression tests beside the shared
      chapter-generation quality owner
  - batch runtime/write/resume/payload/status consumers now import their
    batch quality runtime-context contract directly from the shared
    `chapter_generation` quality owner
  - `backend-rs/src/services/mod.rs`
    now drops the shell-only batch quality runtime-context module registration
  - deleted shell file:
    - `backend-rs/src/services/chapter_batch_generation_quality_runtime_context_service.rs`

  This is a real Phase 5 migration step because Rust now owns one tighter
  shared chapter-generation quality-runtime-context chain instead of keeping a
  separate batch-named facade for algorithms and payload helpers that are
  already chapter-generation-scoped:
  - `shared persisted/runtime quality sources -> shared quality runtime context owner -> batch runtime/write/resume/payload consumers`

  The remaining Python dependency is unchanged in this slice: HTTP payloads,
  SSE payloads, task lifecycle semantics, checkpoint shapes, provider
  defaults, and fallback shells are preserved. The rollback boundary remains
  the existing gateway/Python fallback shell because no route cutover or
  transport ownership changed.

  Focused validation passed with:
  `rustfmt --edition 2021 "backend-rs/src/services/chapter_generation_quality_runtime_context_service.rs" "backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs" "backend-rs/src/services/chapter_batch_generation_write_workflow_service.rs" "backend-rs/src/services/chapter_batch_generation_resume_task_command_service.rs" "backend-rs/src/services/chapter_batch_generation_quality_status_service.rs" "backend-rs/src/services/chapter_batch_generation_task_payload_base_service.rs" "backend-rs/src/services/mod.rs"`
  `cargo test chapter_generation_quality_runtime_context_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-quality-runtime-context-collapse-generation" -- --nocapture`
  `cargo test chapter_batch_generation --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-quality-runtime-context-collapse-batch" -- --nocapture`
  `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-quality-runtime-context-collapse-check"`

- 2026-06-07 batch-generation task-model facade file-collapse checkpoint:
  this slice stayed on Package C, `chapter_batch_generation`, and collapsed
  one remaining batch task-model facade file back into the real batch create
  write-workflow owner. Before this change, the batch create lane had already
  converged on one coherent write-workflow owner for:
  - owned project/chapter access and prerequisite validation
  - normalized create workflow request preparation
  - startup snapshot planning and runtime launch assembly
  - create response payload projection
  - final task insert and runtime dispatch handoff

  but one neighboring file still sat between that write-workflow owner and
  the final persistence-ready task contract:
  - `backend-rs/src/services/chapter_batch_generation_task_model_service.rs`

  that file no longer owned an independent route boundary, fallback shell,
  rollback seam, or task lifecycle branch. It only replayed:
  - `BatchGenerationTaskPersistenceSeed`
  - `BatchGenerationTaskPersistenceSeed::into_active_model(...)`
  - `build_batch_generation_task_active_model(...)`

  this checkpoint tightens that batch create owner boundary further:
  - `backend-rs/src/services/chapter_batch_generation_write_workflow_service.rs`
    now directly owns:
    - `BatchGenerationTaskPersistenceSeed`
    - `BatchGenerationTaskPersistenceSeed::into_active_model(...)`
    - pending batch task `ActiveModel` materialization
    - `build_batch_generation_task_active_model(...)` as the focused
      test-facing helper for persistence-shape regression coverage
  - `backend-rs/src/services/chapter_single_generation_stream_workflow_service.rs`
    test helpers now consume
    `build_batch_generation_task_active_model(...)` directly from the batch
    write-workflow owner instead of reopening the deleted task-model facade
  - `backend-rs/src/services/mod.rs`
    now drops the shell-only batch task-model module registration
  - deleted shell file:
    - `backend-rs/src/services/chapter_batch_generation_task_model_service.rs`

  This is a real Phase 5 migration step because Rust now owns one tighter
  batch create launch -> persistence-ready task seed -> task insert chain
  inside the write-workflow lane instead of preserving a separate task-model
  facade that adds no validation, no route boundary, and no independent error
  contract:
  - `batch create workflow owner -> task persistence seed -> ActiveModel insert`

  The remaining Python dependency is unchanged in this slice: route payloads,
  fallback shells, task lifecycle semantics, checkpoint shape, SSE payloads,
  provider defaults, and rollback shell remain stable. The rollback boundary
  remains the existing gateway/Python fallback shell because no route
  ownership or transport cutover changed.

  Focused validation passed with:
  `cargo test chapter_batch_generation_write_workflow_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-task-model-collapse-write" -- --nocapture`
  `cargo test chapter_single_generation_stream_workflow_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-task-model-collapse-single-stream" -- --nocapture`
  `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-task-model-collapse-check"`

- 2026-06-07 batch-generation stream-semantics facade file-collapse checkpoint:
  this slice stayed on Package C, `chapter_batch_generation`, and collapsed
  one remaining stream-semantics facade file back into the real batch
  status-stream owner. Before this change, the batch status stream lane
  already owned the real production boundaries:
  - owned task + snapshot read for one batch stream session
  - `task/snapshot -> quality context -> BatchGenerationStreamState`
  - status poll loop, idle heartbeat cadence, stream close timeout, and
    final SSE send ordering
  - cursor-based observation gating for state-change driven emission

  but one neighboring file still sat between that stream owner and the final
  `task/snapshot -> stream semantics` contract:
  - `backend-rs/src/services/chapter_batch_generation_stream_semantics_service.rs`

  that file no longer owned an independent transport route, fallback shell,
  rollback boundary, or error translation lane. It only replayed:
  - `BatchGenerationStreamState`
  - `BatchGenerationStreamObservationKey`
  - `BatchGenerationStreamTerminalKind`
  - `BatchGenerationResolvedStreamStatus`
  - `from_task_state(...)`
  - `from_task_state_with_quality_context(...)`
  - `observation_key(...)`
  - `resolve_stream_quality_gate(...)`
  - `build_quality_gate_from_active_story_repair_payload(...)`
  - `resolve_stream_event_status(...)`
  - resolved-status default/event/terminal helpers

  this checkpoint tightens that batch owner boundary further:
  - `backend-rs/src/services/chapter_batch_generation_status_stream_service.rs`
    now directly owns:
    - stream-state materialization from owned task + snapshot sources
    - quality-gate projection for active story-repair payloads
    - observation-key projection for cursor change detection
    - terminal-kind and event-status semantics
    - resolved-status default message/progress helpers
    - the focused stream-semantics contract tests that belong beside the
      status-stream poll owner
  - `backend-rs/src/services/mod.rs`
    now drops the shell-only batch stream-semantics module registration
  - deleted shell file:
    - `backend-rs/src/services/chapter_batch_generation_stream_semantics_service.rs`

  This is a real Phase 5 migration step because Rust now owns one tighter
  batch stream chain instead of preserving a separate semantics facade that
  adds no validation, no route boundary, and no independent error contract:
  - `owned read-state -> stream-state semantics -> cursor resolution -> SSE event emission`

  The remaining Python dependency is unchanged in this slice: route payloads,
  SSE payload shapes, fallback shells, task lifecycle, checkpoint shape, and
  provider defaults are preserved. The rollback boundary remains the existing
  gateway/Python fallback shell because no route ownership or transport cutover
  changed.

  Focused validation passed with:
  `rustfmt --edition 2021 "backend-rs/src/services/chapter_batch_generation_status_stream_service.rs" "backend-rs/src/services/mod.rs"`
  `cargo test chapter_batch_generation_status_stream_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-stream-semantics-file-collapse-stream" -- --nocapture`
  `cargo test chapter_batch_generation --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-stream-semantics-file-collapse-batch" -- --nocapture`
  `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-stream-semantics-file-collapse-check"`

- 2026-06-07 batch-generation status-stream event facade file-collapse checkpoint:
  this slice stayed on Package C, `chapter_batch_generation`, and collapsed
  one remaining status-stream event facade file back into the real batch
  status-stream owner. Before this change, the batch status stream lane
  already owned the real production boundaries:
  - owned task + snapshot load for one batch stream session
  - `task/snapshot -> quality context -> BatchGenerationStreamState`
  - status poll loop, idle heartbeat cadence, and stream close timeout
  - SSE transport send/flush ordering for connected, heartbeat, data, and
    timeout/not-found flow

  but one neighboring file still sat between that stream owner and the final
  SSE payload / cursor resolution boundary:
  - `backend-rs/src/services/chapter_batch_generation_status_stream_event_service.rs`

  that file no longer owned an independent transport route, fallback shell,
  rollback boundary, or error translation lane. It only replayed:
  - connected/task-not-found/timeout event payload builders
  - heartbeat/data `Event` transport wrappers
  - `BatchGenerationStreamCursor`
  - `BatchGenerationStreamEventResolution`
  - `BatchGenerationStreamState::events(...)`
  - `analysis_started_event(...)`
  - `terminal_events(...)`

  this checkpoint tightens that batch owner boundary further:
  - `backend-rs/src/services/chapter_batch_generation_status_stream_service.rs`
    now directly owns:
    - connected/task-not-found/timeout SSE payload builders
    - heartbeat/data `Event` transport wrappers
    - `BatchGenerationStreamCursor`
    - `BatchGenerationStreamEventResolution`
    - `BatchGenerationStreamState` event projection helpers:
      `events(...)`, `analysis_started_event(...)`, `terminal_events(...)`
    - the cursor close/continue and SSE payload contract tests that belong
      beside the status-stream poll owner
  - `backend-rs/src/services/mod.rs`
    now drops the shell-only batch status-stream event module registration
  - deleted shell file:
    - `backend-rs/src/services/chapter_batch_generation_status_stream_event_service.rs`

  This is a real Phase 5 migration step because Rust now owns one tighter
  batch status-stream chain instead of preserving a separate event facade that
  adds no validation, no route boundary, and no independent error contract:
  - `owned read-state -> stream state -> cursor resolution -> SSE event emission`

  The remaining Python dependency is unchanged in this slice: route payloads,
  SSE payload shapes, fallback shells, task lifecycle, checkpoint shape, and
  provider defaults are preserved. The rollback boundary remains the existing
  gateway/Python fallback shell because no route ownership or transport cutover
  changed.

  Focused validation passed with:
  `rustfmt --edition 2021 "backend-rs/src/services/chapter_batch_generation_status_stream_service.rs" "backend-rs/src/services/mod.rs"`
  `cargo test chapter_batch_generation_status_stream_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-status-stream-event-collapse-stream" -- --nocapture`
  `cargo test chapter_batch_generation --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-status-stream-event-collapse-batch" -- --nocapture`
  `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-status-stream-event-collapse-check"`

- 2026-06-07 batch-generation snapshot facade file-collapse checkpoint:
  this slice stayed on Package C, `chapter_batch_generation`, and collapsed
  one remaining snapshot facade file back into the real batch runtime owner.
  Before this change, the batch runtime lifecycle and batch create workflow
  already owned the real production boundaries:
  - runtime checkpoint/stage mutation
  - runtime snapshot refresh and persistence writes
  - batch queued snapshot planning for create/startup seed
  - batch resume snapshot reset planning for resume/reset persistence

  but one neighboring file still sat between those owners and the shared
  chapter-generation snapshot persistence boundary:
  - `backend-rs/src/services/chapter_batch_generation_snapshot_service.rs`

  that file no longer owned an independent transport, fallback shell, or
  rollback boundary. It only replayed:
  - `BatchGenerationQueuedSnapshotPlan`
  - `BatchGenerationResumeSnapshotPlan`
  - `merge_batch_generation_runtime_state(...)`
  - `project_merged_batch_generation_runtime_state(...)`
  - `upsert_batch_generation_runtime_snapshot(...)`
  - queued snapshot persist/startup helper wrappers
  - resume snapshot replace helper wrappers

  this checkpoint tightens that batch owner boundary further:
  - `backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs`
    now directly owns:
    - `BatchGenerationQueuedSnapshotPlan`
    - `BatchGenerationResumeSnapshotPlan`
    - `merge_batch_generation_runtime_state(...)`
    - `project_merged_batch_generation_runtime_state(...)`
    - batch-local `upsert_batch_generation_runtime_snapshot(...)`
    - the queued/resume snapshot plan contract tests that belong beside the
      runtime owner
  - `backend-rs/src/services/chapter_batch_generation_write_workflow_service.rs`
    now consumes `BatchGenerationQueuedSnapshotPlan` directly from the batch
    runtime owner for create/startup persistence preparation
  - `backend-rs/src/services/mod.rs`
    now drops the shell-only batch snapshot module registration
  - deleted shell file:
    - `backend-rs/src/services/chapter_batch_generation_snapshot_service.rs`

  This is a real Phase 5 migration step because Rust now owns one tighter
  batch create/resume/runtime -> snapshot planning/persistence chain instead of
  preserving a separate batch snapshot facade that adds no validation, no
  route boundary, and no independent error contract:
  - `batch create/startup -> runtime owner snapshot plan -> shared snapshot persistence`
  - `batch resume/reset -> runtime owner snapshot plan -> shared snapshot persistence`
  - `batch runtime/status/cancel -> runtime owner snapshot merge/write`

  The remaining Python dependency is unchanged in this slice: route payloads,
  fallback shells, task lifecycle, checkpoint shape, SSE payloads, and
  provider defaults are preserved. The rollback boundary remains the existing
  gateway/Python fallback shell because no route ownership or transport cutover
  changed.

  Focused validation passed with:
  `rustfmt --edition 2021 "backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs" "backend-rs/src/services/chapter_batch_generation_write_workflow_service.rs" "backend-rs/src/services/mod.rs"`
  `cargo test chapter_batch_generation_runtime_state_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-snapshot-file-collapse-runtime" -- --nocapture`
  `cargo test chapter_batch_generation_write_workflow_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-snapshot-file-collapse-write" -- --nocapture`
  `cargo test chapter_batch_generation --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-snapshot-file-collapse-batch" -- --nocapture`
  `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-snapshot-file-collapse-check"`

- 2026-06-07 single-generation route/public-start workflow-collapse checkpoint:
  this slice stayed on Package B, `chapter_single_generation`, and collapsed
  the single-chapter route/start handoff back into the background/stream
  workflow owners. Before this change, the route file already delegated the
  real business flow, but it still locally rebuilt the same request contract
  before handing off to neighboring workflow-start owners:
  - `backend-rs/src/api/chapter_generation_routes.rs`
  - `backend-rs/src/services/chapter_single_generation_background_write_entry_service.rs`
  - `backend-rs/src/services/chapter_single_generation_stream_workflow_service.rs`

  that route/public-start layer no longer owned an independent transport
  branch, fallback shell, or rollback seam. It only replayed:
  - `route payload -> SingleChapterGenerationRequest`
  - request handoff into background workflow start
  - request handoff into stream workflow start

  this checkpoint tightens that boundary further:
  - `backend-rs/src/services/chapter_single_generation_background_write_entry_service.rs`
    now accepts `SingleChapterGenerationRouteRequest` directly at the public
    workflow-start entry and owns route-payload -> request normalization
    locally before entering the existing background workflow-entry branch
  - `backend-rs/src/services/chapter_single_generation_stream_workflow_service.rs`
    now accepts `SingleChapterGenerationRouteRequest` directly at the public
    stream workflow-start entry and owns route-payload -> request
    normalization locally before prepare/runtime launch handoff
  - `backend-rs/src/api/chapter_generation_routes.rs`
    now stays thinner and only forwards transport payloads into the matching
    background/stream workflow owners instead of rebuilding the same request
    contract inline

  This is a real Phase 5 migration step because Rust now owns one tighter
  single-generation route/start chain instead of preserving a separate
  route-local request-normalization hop around the same workflow-start owners:
  - `route payload -> background/stream workflow owner -> prepare/runtime owner`

  The remaining Python dependency is unchanged in this slice: route payload
  shapes, fallback shells, schema assumptions, task lifecycle, checkpoint
  shape, SSE payloads, and provider defaults are preserved. The rollback
  boundary remains the existing gateway/Python fallback shell because no
  transport cutover or route surface changed.

  Focused validation passed with:
  `rustfmt --edition 2021 "backend-rs/src/api/chapter_generation_routes.rs" "backend-rs/src/services/chapter_single_generation_background_write_entry_service.rs" "backend-rs/src/services/chapter_single_generation_stream_workflow_service.rs"`
  `cargo test chapter_generation_routes --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-route-start-collapse" -- --nocapture`
  `cargo test chapter_single_generation_background_write_entry_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-route-start-collapse" -- --nocapture`
  `cargo test chapter_single_generation_stream_workflow_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-route-start-collapse" -- --nocapture`
  `cargo test chapter_single_generation --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-route-start-collapse" -- --nocapture`
  `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-route-start-collapse"`

- 2026-06-07 single-generation runtime-restore file-collapse checkpoint:
  this slice stayed on Package B, `chapter_single_generation`, and collapsed
  the runtime-restore facade back into the prepare owner. Before this change,
  the prepare owner already owned:
  - request validation and Python-style route normalization
  - chapter target loading and prerequisite checks
  - execution-config preparation from request runtime-state
  - restored runtime launch materialization
  - background launch-parts materialization
  - task payload / task seed / background response projection

  but one neighboring file still sat between that prepare owner and the
  restored quality/runtime-state seed contract:
  - `backend-rs/src/services/chapter_single_generation_runtime_restore_service.rs`

  that file no longer owned an independent transport, fallback shell, or
  rollback boundary. It only replayed:
  - `merge_single_generation_runtime_state(...)`
  - `SingleGenerationStartupSnapshotPlan`
  - `RestoredSingleGenerationRuntimeState`
  - restored runtime-state seed payload construction
  - recent-history quality-summary fallback for story-repair recovery
  - persisted compat-option restore from runtime-state seed
  - final restored startup snapshot + runtime launch handoff

  this checkpoint tightens that boundary further:
  - `backend-rs/src/services/chapter_single_generation_prepare_service.rs`
    now directly owns:
    - `merge_single_generation_runtime_state(...)`
    - `SingleGenerationStartupSnapshotPlan`
    - `SingleGenerationRuntimeSeedSource`
    - `RestoredSingleGenerationRuntimeState`
    - restored runtime-state seed payload construction
    - recent-history quality-summary fallback
    - restored compat-option projection
    - `restore_single_generation_runtime_state(...)`
    - the restore/startup-snapshot contract tests that now live beside the
      prepare-owner restored launch chain
  - `backend-rs/src/services/mod.rs`
    now drops the shell-only runtime-restore module registration
  - deleted shell file:
    - `backend-rs/src/services/chapter_single_generation_runtime_restore_service.rs`

  This is a real Phase 5 migration step because Rust now owns one tighter
  single-generation prepare chain rather than preserving a separate
  runtime-restore file that adds no validation, no route boundary, and no
  independent error contract:
  - `request/target/config -> prepare owner -> restored seed/startup snapshot -> runtime/background launch`

  The remaining Python dependency is unchanged in this slice: route payloads,
  fallback shells, schema assumptions, task lifecycle, checkpoint shape, SSE
  payloads, and provider defaults are preserved. The rollback boundary remains
  the existing gateway/Python fallback shell because no route ownership or
  transport cutover changed.

  Focused validation passed with:
  `rustfmt --edition 2021 "backend-rs/src/services/chapter_single_generation_prepare_service.rs" "backend-rs/src/services/mod.rs"`
  `cargo test chapter_single_generation_prepare_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-runtime-restore-collapse" -- --nocapture`
  `cargo test chapter_single_generation --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-runtime-restore-collapse" -- --nocapture`
  `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-runtime-restore-collapse"`

- 2026-06-07 single-generation runtime-outcome file-collapse checkpoint:
  this slice stayed on Package B, `chapter_single_generation`, and collapsed
  the runtime outcome facade back into the runtime-state owner. Before this
  change, the runtime-state owner already owned:
  - runtime launch materialization
  - task-stage mutation and checkpoint projection
  - runtime preparation persistence
  - runtime lifecycle spawn/dispatch
  - generated content execution

  but one neighboring file still sat between the runtime lifecycle owner and
  the final success / failure / manual-review persistence boundary:
  - `backend-rs/src/services/chapter_single_generation_runtime_outcome_service.rs`

  that file no longer owned an independent transport, fallback, or rollback
  boundary. It only replayed:
  - generated-result success persistence
  - failed-generation persistence
  - follow-up analysis trigger for manual review
  - manual-review label resolution
  - quality-blocked snapshot persistence

  this checkpoint tightens that boundary further:
  - `backend-rs/src/services/chapter_single_generation_runtime_state_service.rs`
    now directly owns:
    - `SingleGenerationRuntimeOutcome`
    - success / failure / manual-review runtime outcome persistence
    - follow-up analysis trigger and manual-review label resolution
    - the outcome contract tests that used to live in the deleted shell file
  - `backend-rs/src/services/mod.rs`
    now drops the shell-only runtime-outcome module registration
  - deleted shell file:
    - `backend-rs/src/services/chapter_single_generation_runtime_outcome_service.rs`

  This is a real Phase 5 migration step because Rust now owns one tighter
  single-generation runtime launch -> task stage / checkpoint ->
  success|failed|manual-review outcome chain rather than preserving a
  separate runtime-outcome file that adds no validation, no route boundary,
  and no independent error contract:
  - `runtime launch -> runtime-state owner -> outcome persistence`

  The remaining Python dependency is unchanged in this slice: fallback
  shells, schema assumptions, task lifecycle, checkpoint shape, SSE payloads,
  and provider defaults are preserved. The rollback boundary remains the
  existing gateway/Python fallback shell because no route ownership or
  transport cutover changed.

  Focused validation passed with:
  `rustfmt --edition 2021 "backend-rs/src/services/chapter_single_generation_runtime_state_service.rs" "backend-rs/src/services/mod.rs"`
  `cargo test chapter_single_generation_runtime_state_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-runtime-outcome-collapse" -- --nocapture`
  `cargo test chapter_single_generation --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-runtime-outcome-collapse" -- --nocapture`
  `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-runtime-outcome-collapse"`

- 2026-06-07 single-generation request file-collapse checkpoint:
  this slice stayed on Package B, `chapter_single_generation`, and collapsed
  the route request / compatibility request facade back into the prepare
  owner. Before this change, the prepare owner already owned:
  - Python-style request validation and field normalization
  - chapter target loading and prerequisite checks
  - restored runtime launch materialization
  - background launch-parts materialization
  - task seed / active-model projection
  - task-view payload/minutes/status projection
  - background create response payload assembly

  but one neighboring file still sat between the route boundary and the
  actual prepare owner:
  - `backend-rs/src/services/chapter_single_generation_request_service.rs`

  that file no longer owned independent route behavior, fallback behavior,
  persistence, runtime dispatch, or validation boundaries outside the prepare
  lane. It only replayed:
  - `SingleChapterGenerationRouteRequest`
  - `SingleChapterGenerationRequest`
  - route-payload -> request normalization
  - nullable/default bool compatibility semantics
  - single-generation bounds and choice validation constants
  - `SingleChapterGenerationCompatOptions`
  - `PrepareSingleChapterGenerationRequestError` and detail messages

  this checkpoint tightens that boundary further:
  - `backend-rs/src/services/chapter_single_generation_prepare_service.rs`
    now directly owns:
    - route request shape and strict unknown-field deserialization contract
    - route-payload -> normalized request conversion
    - Python-style nullable/default request flag compatibility
    - target word-count, text-length, and choice-field bounds
    - `SingleChapterGenerationCompatOptions`
    - `PrepareSingleChapterGenerationRequestError`
    - the request/default/null/bounds/error contract tests that used to
      belong to the deleted request facade
  - `backend-rs/src/api/chapter_generation_routes.rs` now consumes the route
    request shape and request builder directly from the prepare owner
  - route, stream, background-write, runtime-state, runtime-restore,
    regeneration, research, resume, and story-repair callers now consume
    compat/request/error types directly from the prepare owner
  - `backend-rs/src/services/mod.rs` now drops the shell-only module
    registration
  - deleted shell file:
    - `backend-rs/src/services/chapter_single_generation_request_service.rs`

  This is a real Phase 5 migration step because Rust now owns one tighter
  single-generation route request -> prepare -> stream/background/runtime
  chain rather than preserving a separate request facade that adds no
  semantic branching and no independent error contract:
  - `route payload -> prepare owner -> target/runtime/background owners`

  The remaining Python dependency is unchanged in this slice: fallback
  shells, schema assumptions, task lifecycle, checkpoint shape, SSE payloads,
  and provider defaults are preserved. The rollback boundary remains the
  existing gateway/Python fallback shell because no route ownership or
  transport cutover changed.

  Focused validation passed with:
  `rustfmt --edition 2021 "backend-rs/src/services/chapter_single_generation_prepare_service.rs" "backend-rs/src/services/chapter_single_generation_background_write_entry_service.rs" "backend-rs/src/services/chapter_single_generation_stream_workflow_service.rs" "backend-rs/src/services/chapter_single_generation_runtime_state_service.rs" "backend-rs/src/services/chapter_single_generation_runtime_restore_service.rs" "backend-rs/src/services/chapter_generation_request_runtime_state_service.rs" "backend-rs/src/services/chapter_generation_research_payload_service.rs" "backend-rs/src/services/chapter_story_repair_quality_context_service.rs" "backend-rs/src/services/chapter_regeneration_prepare_service.rs" "backend-rs/src/services/chapter_batch_generation_resume_task_command_service.rs" "backend-rs/src/api/chapter_generation_routes.rs" "backend-rs/src/services/mod.rs"`
  `cargo test chapter_single_generation_prepare_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-request-service-collapse" -- --nocapture`
  `cargo test chapter_generation_routes --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-request-service-collapse" -- --nocapture`
  `cargo test chapter_single_generation --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-request-service-collapse" -- --nocapture`
  `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-request-service-collapse"`

- 2026-06-07 single-generation task-view payload file-collapse checkpoint:
  this slice stayed on Package B, `chapter_single_generation`, and collapsed
  the next prepare-adjacent task payload projection shell back into the
  prepare owner. Before this change, the prepare owner already owned:
  - request validation and Python-style route request normalization
  - chapter target loading and prerequisite checks
  - restored runtime launch materialization
  - background launch-parts materialization
  - task seed / active-model projection
  - background create response payload assembly

  but one neighboring file still sat between that owner and the final task
  view/status payload contract:
  - `backend-rs/src/services/chapter_single_generation_task_view_payload_service.rs`

  that file no longer owned independent route behavior, fallback behavior,
  error contracts, persistence, or validation boundaries. It only replayed:
  - single-generation active task status constants
  - estimated task minutes projection
  - runtime payload base projection
  - task-state -> task-view payload projection
  - timestamp and chapter-id payload helper logic

  this checkpoint tightens that boundary further:
  - `backend-rs/src/services/chapter_single_generation_prepare_service.rs`
    now directly owns:
    - `estimated_single_generation_task_minutes(...)`
    - `single_generation_pending_stage_code()`
    - `single_generation_active_task_statuses()`
    - `build_single_generation_runtime_payload_base(...)`
    - `build_single_generation_task_view_payload_from_task_state(...)`
    - the payload/minutes/status contract tests that used to live in the
      deleted shell file
  - `backend-rs/src/services/chapter_single_generation_background_write_entry_service.rs`
    now consumes task-view payload/minutes/status helpers directly from the
    prepare owner for the existing-background branch
  - `backend-rs/src/services/mod.rs`
    now drops the shell-only module registration
  - deleted shell file:
    - `backend-rs/src/services/chapter_single_generation_task_view_payload_service.rs`

  This is a real Phase 5 migration step because Rust now owns one tighter
  single-generation prepare -> task payload -> background-write chain rather
  than preserving a separate task-view payload shell that adds no validation,
  no semantic branching, and no independent error contract:
  - `request/target -> prepare owner -> task-view payload/minutes/status -> background-write owner`

  The remaining Python dependency is unchanged in this slice: route payloads,
  fallback shells, schema assumptions, task lifecycle, checkpoint shape, and
  provider defaults are preserved. The rollback boundary remains the existing
  gateway/Python fallback shell because no route ownership changed.

  Focused validation passed with:
  `cargo test chapter_single_generation_prepare_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-task-view-payload-collapse" -- --nocapture`
  `cargo test chapter_single_generation_background_write_entry_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-task-view-payload-collapse" -- --nocapture`
  `cargo test chapter_single_generation --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-task-view-payload-collapse" -- --nocapture`
  `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-task-view-payload-collapse"`

- 2026-06-07 single-generation existing-background read-state/quality-status
  file-collapse checkpoint:
  this slice stayed on Package B, `chapter_single_generation`, and collapsed
  the next existing-background function group back into the background-write
  owner. Before this change, the background write lane already owned:
  - chapter target loading
  - existing task short-circuit branch selection
  - existing-background payload assembly
  - new background launch persistence and runtime dispatch

  but two neighboring files still sat between that owner and the concrete
  existing-background branch:
  - `backend-rs/src/services/chapter_single_generation_existing_background_read_state_service.rs`
  - `backend-rs/src/services/chapter_single_generation_quality_status_service.rs`

  those files no longer owned independent route behavior, fallback behavior,
  error contracts, or validation boundaries. They only replayed:
  - active single-generation background task query and recovery
  - snapshot loading for active tasks
  - task chapter-match filtering
  - snapshot/runtime-state -> quality-status context projection
  - existing-background read-state materialization for the same
    background-write payload branch

  this checkpoint tightens that boundary further:
  - `backend-rs/src/services/chapter_single_generation_background_write_entry_service.rs`
    now directly owns:
    - `SingleGenerationQualityStatusContext`
    - `SingleGenerationExistingBackgroundTaskReadState`
    - active single-generation background task query
    - active task recovery and snapshot-backed read-state materialization
    - chapter-id matching for string/object `chapter_ids`
    - existing-background quality-status payload insertion
    - the contract tests that used to live in the deleted shell files
  - `backend-rs/src/services/mod.rs`
    now drops both shell-only module registrations
  - deleted shell files:
    - `backend-rs/src/services/chapter_single_generation_existing_background_read_state_service.rs`
    - `backend-rs/src/services/chapter_single_generation_quality_status_service.rs`

  This is a real Phase 5 migration step because Rust now owns one tighter
  single-generation background-write chain:
  - `chapter target -> active task query/recovery -> snapshot quality context -> existing payload`

  The remaining Python dependency is unchanged in this slice: route payloads,
  fallback shells, schema assumptions, task lifecycle, checkpoint shape, and
  provider defaults are preserved. The rollback boundary remains the existing
  gateway/Python fallback shell because no route ownership changed.

  Focused validation passed with:
  `cargo test chapter_single_generation_background_write_entry_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-existing-background-read-state-collapse" -- --nocapture`
  `cargo test chapter_single_generation --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-existing-background-read-state-collapse" -- --nocapture`
  `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-existing-background-read-state-collapse"`

- 2026-06-06 single-generation startup-snapshot file-collapse checkpoint:
  this slice stayed on package B, `chapter_single_generation`, and continued
  the whole-owner migration path around the single restored-runtime lane.
  After the runtime-restore owner split, stream-result owner collapse, and
  runtime-checkpoint owner collapse had already narrowed the surrounding
  production chain, the remaining production mismatch was that
  `backend-rs/src/services/chapter_single_generation_startup_snapshot_service.rs`
  still sat as one extra file-local startup snapshot facade beside the real
  restored-runtime owner.

  in the current module shape, that file no longer needed to remain separate
  because the neighboring restore owner already owns:
  - chapter-scoped restored runtime-state materialization
  - pending checkpoint merge semantics
  - startup snapshot plan derivation from restored runtime sources

  the deleted file only replayed:
  - `merge_single_generation_runtime_state(...)`
  - `SingleGenerationStartupSnapshotPlan`
  - startup snapshot quality/accessor projection
  - startup snapshot persistence helper

  this checkpoint tightens that boundary further:
  - `backend-rs/src/services/chapter_single_generation_runtime_restore_service.rs`
    now directly owns:
    - `merge_single_generation_runtime_state(...)`
    - `SingleGenerationStartupSnapshotPlan`
    - startup snapshot quality/accessor projection
    - startup snapshot persistence helper
    - the startup snapshot contract tests that used to live in the deleted
      shell file
  - `backend-rs/src/services/chapter_single_generation_runtime_seed_service.rs`
    now consumes `SingleGenerationStartupSnapshotPlan` directly from the
    restore owner
  - `backend-rs/src/services/mod.rs`
    now drops the shell-only
    `chapter_single_generation_startup_snapshot_service` module registration
  - deleted shell file:
    - `backend-rs/src/services/chapter_single_generation_startup_snapshot_service.rs`

  this is a real Phase 5 migration step because Rust now owns one tighter
  read-context / quality fallback -> restored runtime -> startup snapshot
  persistence chain rather than preserving a separate startup snapshot shell
  that adds no validation, no semantic branching, and no independent error
  contract:
  - `read-context / quality fallback -> restore owner -> startup snapshot plan`

  focused validation passed with:
  - `cargo test chapter_single_generation_runtime_restore_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-startup-snapshot-collapse" -- --nocapture`
  - `cargo test chapter_single_generation_runtime_seed_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-startup-snapshot-collapse" -- --nocapture`
  - `cargo test chapter_single_generation --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-startup-snapshot-collapse" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-startup-snapshot-collapse"`

- 2026-06-06 single-generation runtime-checkpoint file-collapse checkpoint:
  this slice stayed on package B, `chapter_single_generation`, and continued
  the whole-owner migration path around the single runtime lifecycle lane.
  After the stream-result owner and background-write owner had already been
  narrowed, the remaining production mismatch was that
  `backend-rs/src/services/chapter_single_generation_runtime_checkpoint_service.rs`
  still sat as one extra file-local checkpoint owner beside the real runtime
  lifecycle owner.

  in the current module shape, that file no longer needed to remain separate
  because the neighboring runtime-state owner already owns:
  - runtime launch materialization
  - runtime lifecycle dispatch
  - generation success / failure execution flow

  the deleted file only replayed:
  - single-generation checkpoint stage enum
  - checkpoint payload projection for runtime stages
  - `persist_runtime_preparation(...)`
  - `persist_with_checkpoint(...)`

  this checkpoint tightens that boundary further:
  - `backend-rs/src/services/chapter_single_generation_runtime_state_service.rs`
    now directly owns:
    - `SingleGenerationSnapshotStage`
    - checkpoint payload projection
    - runtime preparation snapshot persistence
    - staged lifecycle snapshot persistence helpers that used to live in the
      deleted shell file
    - the runtime checkpoint contract tests that used to live in the deleted
      shell file
  - `backend-rs/src/services/chapter_single_generation_prepare_service.rs`
    now consumes checkpoint stage projection directly from the runtime-state
    owner
  - `backend-rs/src/services/chapter_single_generation_runtime_outcome_service.rs`
    now consumes `SingleGenerationSnapshotStage` directly from the runtime-state
    owner
  - `backend-rs/src/services/mod.rs`
    now drops the shell-only
    `chapter_single_generation_runtime_checkpoint_service` module registration
  - deleted shell file:
    - `backend-rs/src/services/chapter_single_generation_runtime_checkpoint_service.rs`

  this is a real Phase 5 migration step because Rust now owns one tighter
  runtime launch -> lifecycle stage -> checkpoint persistence chain rather
  than preserving a separate checkpoint shell that adds no validation, no
  branch semantics, and no independent error contract:
  - `runtime launch -> runtime-state owner -> lifecycle checkpoint persistence`

  focused validation passed with:
  - `cargo test chapter_single_generation_runtime_state_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-runtime-checkpoint-collapse" -- --nocapture`
  - `cargo test chapter_single_generation_prepare_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-runtime-checkpoint-collapse" -- --nocapture`
  - `cargo test chapter_single_generation_runtime_outcome_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-runtime-checkpoint-collapse" -- --nocapture`
  - `cargo test chapter_single_generation --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-runtime-checkpoint-collapse" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-runtime-checkpoint-collapse"`

- 2026-06-06 single-generation stream-result file-collapse checkpoint:
  this slice stayed on package B, `chapter_single_generation`, and continued
  the whole-owner migration path around the single stream success lane. After
  the route-facing stream entry shell had already been removed, the remaining
  production mismatch was that
  `backend-rs/src/services/chapter_single_generation_stream_result_service.rs`
  still sat as one extra file-local projection owner beside the real stream
  workflow owner.

  in the current module shape, that file no longer needed to remain separate
  because the neighboring stream-workflow owner already owns:
  - runtime launch preparation
  - stream lifecycle start
  - generation success / failure transport emission

  the deleted file only replayed:
  - success analysis projection
  - story-runtime contract materialization
  - quality-gate / quality-metrics / result event assembly
  - ordered stream success emission payload construction

  this checkpoint tightens that boundary further:
  - `backend-rs/src/services/chapter_single_generation_stream_workflow_service.rs`
    now directly owns:
    - `SingleGenerationStreamSuccessArtifacts`
    - success event payload ordering and response payload assembly
    - story-runtime contract projection
    - quality-gate action / message normalization
    - the stream success contract tests that used to live in the deleted shell
      file
  - `backend-rs/src/services/mod.rs`
    now drops the shell-only
    `chapter_single_generation_stream_result_service` module registration
  - deleted shell file:
    - `backend-rs/src/services/chapter_single_generation_stream_result_service.rs`

  this is a real Phase 5 migration step because Rust now owns one tighter
  stream runtime -> success projection -> ordered SSE emission chain rather
  than preserving a separate success-result shell that adds no validation, no
  branch semantics, and no independent error contract:
  - `runtime launch -> stream workflow owner -> success/result emission`

  focused validation passed with:
  - `cargo test chapter_single_generation_stream_workflow_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-stream-result-collapse" -- --nocapture`
  - `cargo test chapter_single_generation --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-stream-result-collapse" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-stream-result-collapse"`

- 2026-06-06 single-generation existing-background-payload file-collapse checkpoint:
  this slice stayed on package B, `chapter_single_generation`, and continued
  the whole-owner migration path around the single existing-background
  background-write lane. After the existing-background read-state owner and
  background-write entry owner had already been narrowed, the remaining
  production mismatch was that
  `backend-rs/src/services/chapter_single_generation_existing_background_payload_service.rs`
  still sat as one extra file-local projection shell between the real owned
  read-state and the background-write owner.

  in the current module shape, that file no longer needed to remain separate
  because the neighboring background-write entry owner already owns:
  - target access
  - existing-task short-circuit
  - new background launch preparation
  - final persist-and-dispatch boundary

  the deleted file only replayed:
  - `owned single-generation read-state -> existing background payload`
  - task/runtime/quality payload projection for the same background-write lane

  this checkpoint tightens that boundary further:
  - `backend-rs/src/services/chapter_single_generation_background_write_entry_service.rs`
    now directly owns:
    - existing-background payload projection from owned read-state
    - task/runtime/quality payload assembly for the background-write lane
    - the richer/minimal existing-background payload contract tests that used
      to live in the deleted shell file
  - `backend-rs/src/services/mod.rs`
    now drops the shell-only
    `chapter_single_generation_existing_background_payload_service` module
    registration
  - deleted shell file:
    - `backend-rs/src/services/chapter_single_generation_existing_background_payload_service.rs`

  this is a real Phase 5 migration step because Rust now owns one tighter
  existing-background read-state -> payload -> background-write chain rather
  than preserving a separate payload shell that adds no validation, no branch
  semantics, and no independent error contract:
  - `owned read-state -> background-write owner -> existing task payload`

  focused validation passed with:
  - `cargo test chapter_single_generation_background_write_entry_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-existing-background-payload-collapse" -- --nocapture`
  - `cargo test chapter_single_generation --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-existing-background-payload-collapse" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-existing-background-payload-collapse"`

- 2026-06-06 single-generation background-launch file-collapse checkpoint:
  this slice stayed on package B, `chapter_single_generation`, and continued
  the whole-owner migration path around the single background-write lane.
  After the route/write shells had already been removed, the remaining
  production mismatch was that
  `backend-rs/src/services/chapter_single_generation_background_launch_service.rs`
  still sat as one extra file-local launch hop between the real background
  entry owner and the runtime dispatch chain:
  - task seed active-model insert
  - startup snapshot persistence
  - runtime dispatch

  in the current module shape, those directions no longer need to stay in a
  separate file because the neighboring background-write entry owner already
  owns:
  - target access
  - existing-task short-circuit
  - launch-parts preparation handoff
  - final persist-and-dispatch boundary

  this checkpoint tightens that boundary further:
  - `backend-rs/src/services/chapter_single_generation_background_write_entry_service.rs`
    now directly owns:
    - task seed active-model insert
    - startup snapshot persistence
    - runtime dispatch handoff
    - background launch-parts test helpers previously living in the shell file
  - `backend-rs/src/services/mod.rs`
    now drops the shell-only
    `chapter_single_generation_background_launch_service` module registration
  - deleted shell file:
    - `backend-rs/src/services/chapter_single_generation_background_launch_service.rs`

  this is a real Phase 5 migration step because Rust now owns one tighter
  background create -> persist -> runtime dispatch chain rather than
  preserving a separate launch shell that adds no validation, no branch
  selection, and no error-translation boundary:
  - background-write entry owner:
    `request -> target access/existing-task check -> launch prepare -> persist+dispatch`

  focused validation passed with:
  - `cargo fmt --manifest-path "backend-rs/Cargo.toml"`
  - `cargo test chapter_single_generation_background_write_entry_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-background-launch-collapse" -- --nocapture`
  - `cargo test chapter_generation_routes --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-background-launch-collapse" -- --nocapture`
  - `cargo test chapter_single_generation_runtime_seed_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-background-launch-collapse" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-background-launch-collapse"`

  follow-up package entry:
  continue package B only where one remaining production lane still mixes
  single-generation stream/public-shell/query semantics with adjacent owners;
  prefer another whole file or whole function-group collapse instead of
  reopening launch wrappers.

- 2026-06-06 single-generation route/write shell collapse checkpoint:
  this slice stayed on package B, `chapter_single_generation`, and continued
  the whole-owner migration path around the single route-facing and background
  write entry lane. After the request owner, route file owner, stream-entry
  owner, background-write entry owner, background-launch owner, and
  task-seed/task-stage owners were already narrowed, the remaining production
  mismatch was that two file-local shells still sat between the route boundary
  and the real Rust owners:
  - `chapter_single_generation_route_workflow_service.rs`
  - `chapter_single_generation_write_workflow_service.rs`

  in the current module shape, those directions no longer need to stay as
  separate files:
  - route file owner only needs
    `route payload -> request owner -> real entry owner`
  - background-write entry owner already needs
    `target access / existing-task short-circuit / launch prepare -> persist+dispatch`
  - stream-entry owner already needs
    `route request -> runtime launch prepare -> stream lifecycle spawn`

  this checkpoint tightens that boundary further:
  - `backend-rs/src/api/chapter_generation_routes.rs`
    now directly:
    - normalizes route payload with the request owner
    - dispatches background requests to
      `start_owned_single_generation_background_write_entry(...)`
    - dispatches stream requests to
      `create_owned_single_generation_stream_entry(...)`
  - `backend-rs/src/services/chapter_single_generation_background_write_entry_service.rs`
    remains the dedicated background entry owner for:
    - existing-task payload reuse
    - new task id allocation
    - launch-parts preparation handoff
    - final persist-and-dispatch boundary
  - `backend-rs/src/services/chapter_single_generation_stream_entry_service.rs`
    remains the dedicated stream entry owner for:
    - runtime launch preparation
    - stream lifecycle spawn boundary
  - `backend-rs/src/services/mod.rs`
    now drops both shell-only module registrations
  - deleted shell files:
    - `backend-rs/src/services/chapter_single_generation_route_workflow_service.rs`
    - `backend-rs/src/services/chapter_single_generation_write_workflow_service.rs`

  this is a real Phase 5 migration step because Rust now owns one tighter
  route/request -> entry-owner chain rather than preserving shell-only hops
  that add no validation, no error translation, and no branch semantics:
  - background route lane:
    `route payload -> request owner -> background-write entry owner`
  - stream route lane:
    `route payload -> request owner -> stream-entry owner`

  focused validation passed with:
  - `cargo fmt --manifest-path "backend-rs/Cargo.toml"`
  - `cargo test chapter_generation_routes --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-route-write-shell-collapse" -- --nocapture`
  - `cargo test chapter_single_generation_stream_entry_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-route-write-shell-collapse" -- --nocapture`
  - `cargo test chapter_single_generation_background_write_entry_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-route-write-shell-collapse" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-route-write-shell-collapse"`

  follow-up package entry:
  continue package B only where one remaining production lane still mixes
  single-generation public-shell, query, or stream lifecycle semantics with
  adjacent owners; prefer another whole file or whole function-group collapse
  instead of reintroducing wrapper shells.

- 2026-06-06 single-generation task-seed/task-stage owner split checkpoint:
  this slice stayed on package B, `chapter_single_generation`, and continued
  the whole-owner migration path around the single background-create/runtime
  lifecycle lane. After the startup-snapshot, runtime-checkpoint,
  runtime-outcome, runtime-restore, and runtime-seed owners were already
  narrowed, the remaining production mismatch was that
  `backend-rs/src/services/chapter_single_generation_task_model_service.rs`
  still carried one mixed owner chain:
  - background task create seed / active-model materialization
  - runtime task-stage mutation / persistence

  in the current module shape, those directions no longer need to stay inside
  the same file:
  - task-seed owner only needs
    `validated target/runtime launch -> persistence-ready task seed`
  - task-stage owner only needs
    `task lifecycle stage -> active-model mutation -> persisted task state`

  this checkpoint tightens that boundary further:
  - `backend-rs/src/services/chapter_single_generation_task_seed_service.rs`
    now owns:
    - `SingleGenerationTaskPersistenceSeed`
    - `build_single_generation_background_task_persistence_seed(...)`
    - `build_single_generation_background_task_active_model(...)`
    - file-local owner contract tests for task seed projection / insert model
  - `backend-rs/src/services/chapter_single_generation_task_stage_service.rs`
    now owns:
    - `ModelFieldUpdate`
    - `TaskTimestampUpdate`
    - `SingleGenerationTaskStage`
    - `SingleGenerationTaskStage::persist_for_task(...)`
    - `SingleGenerationTaskStage::apply_to_active_model(...)`
    - file-local owner contract tests for stage mutation / persistence
  - `backend-rs/src/services/chapter_single_generation_prepare_service.rs`
    now consumes the dedicated task-seed owner explicitly instead of reopening
    batch task-seed semantics locally
  - `backend-rs/src/services/chapter_single_generation_runtime_state_service.rs`
    plus the runtime-checkpoint/runtime-outcome neighbors now consume the
    dedicated task-stage owner instead of keeping local task mutation semantics
  - `backend-rs/src/services/mod.rs`
    now registers the two dedicated owner files and drops the mixed
    `chapter_single_generation_task_model_service.rs`

  this is a real Phase 5 migration step because Rust now owns one tighter
  single-generation task create lane and one tighter runtime lifecycle lane
  rather than leaving chapter-only task-model semantics mixed together:
  - task-seed owner:
    `validated target/runtime launch -> persistence-ready task seed`
  - task-stage owner:
    `runtime stage -> task mutation -> persisted lifecycle state`

  focused validation passed with:
  - `cargo fmt --manifest-path "backend-rs/Cargo.toml"`
  - `cargo test chapter_single_generation_task_seed_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-task-seed-stage-owner" -- --nocapture`
  - `cargo test chapter_single_generation_task_stage_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-task-seed-stage-owner" -- --nocapture`
  - `cargo test chapter_single_generation_prepare_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-task-seed-stage-owner" -- --nocapture`
  - `cargo test chapter_single_generation_runtime_checkpoint_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-task-seed-stage-owner" -- --nocapture`
  - `cargo test chapter_single_generation_runtime_outcome_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-task-seed-stage-owner" -- --nocapture`
  - `cargo test chapter_single_generation_runtime_state_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-task-seed-stage-owner" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-task-seed-stage-owner"`

  follow-up package entry:
  continue package B only where one remaining production lane still mixes
  single-generation write/query/stream/public-shell semantics with adjacent
  owners; prefer another whole file or whole function-group collapse instead
  of helper-only relocation.

- 2026-06-06 single-generation runtime-restore owner split checkpoint:
  this slice stayed on package B, `chapter_single_generation`, and continued
  the whole-owner migration path around the single runtime-seed lane.
  After the runtime-checkpoint, startup-snapshot, runtime-outcome, and
  runtime-seed launch-preparation owners were already narrowed, the remaining
  production mismatch was that
  `backend-rs/src/services/chapter_single_generation_runtime_seed_service.rs`
  still carried one mixed owner chain:
  - outer launch preparation / request-to-runtime materialization
  - restored runtime-state read-side / seed recovery / compat restore

  in the current module shape, those directions no longer need to stay inside
  the same file:
  - runtime-seed owner only needs
    `request/target/config -> restored launch preparation`
  - runtime-restore owner only needs
    `analysis read-context / quality fallback / runtime-state seed -> restored runtime launch parts`

  this checkpoint tightens that boundary further:
  - `backend-rs/src/services/chapter_single_generation_runtime_restore_service.rs`
    now owns:
    - restored runtime-state seed payload construction
    - recent-history summary fallback for story-repair seed recovery
    - compat option restore from persisted runtime-state
    - `RestoredSingleGenerationRuntimeState`
    - runtime launch input projection from restored seed state
    - file-local owner contract tests
  - `backend-rs/src/services/chapter_single_generation_runtime_seed_service.rs`
    now keeps only the outer launch-preparation owner:
    - execution config preparation from request runtime-state
    - request/target/config -> restored launch handoff
    - background response payload materialization
  - `backend-rs/src/services/chapter_single_generation_background_launch_service.rs`
    now consumes the dedicated runtime-restore owner boundary explicitly in
    tests instead of depending on the mixed seed file
  - `backend-rs/src/services/mod.rs`
    now registers the dedicated runtime-restore owner file

  this is a real Phase 5 migration step because Rust now owns one tighter
  single-generation runtime restore chain rather than leaving restored
  read-side seed recovery mixed with outer launch preparation:
  - runtime-seed owner:
    `request/target/config -> restored launch owner`
  - runtime-restore owner:
    `read-context / quality fallback -> startup snapshot + runtime launch`

  focused validation passed with:
  - `cargo fmt --manifest-path "backend-rs/Cargo.toml"`
  - `cargo test chapter_single_generation_runtime_restore_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-runtime-restore-owner" -- --nocapture`
  - `cargo test chapter_single_generation_runtime_seed_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-runtime-restore-owner" -- --nocapture`
  - `cargo test chapter_single_generation_background_launch_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-runtime-restore-owner" -- --nocapture`
  - `cargo test chapter_single_generation_prepare_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-runtime-restore-owner" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-runtime-restore-owner"`

  follow-up package entry:
  continue package B only where one remaining production lane still mixes
  single-generation prepare/runtime/public-shell semantics with adjacent
  owners; prefer another whole file or whole function-group collapse instead
  of helper-only relocation.

- 2026-06-06 single-generation existing-background-payload owner split checkpoint:
  this slice stayed on package B, `chapter_single_generation`, and continued
  the whole-owner migration path around the single existing-background lane.
  After the existing-background read-state, background-write entry, and
  stream-entry owners were already narrowed, the remaining production mismatch
  was that
  `backend-rs/src/services/chapter_single_generation_existing_background_query_service.rs`
  still carried one mixed read-side owner:
  - owned read-state query load
  - final existing-background payload projection

  in the current module shape, those directions no longer need to stay inside
  the same file:
  - existing-background query owner only needs
    `owned read-state load -> payload owner handoff`
  - existing-background payload owner only needs
    `task/runtime/quality read-state -> final existing-background payload`

  this checkpoint tightens that boundary further:
  - `backend-rs/src/services/chapter_single_generation_existing_background_payload_service.rs`
    now owns:
    - final existing-background payload projection
    - task-state/runtime-state/quality-context materialization for payload
    - file-local owner contract tests for richer/minimal payload branches
  - `backend-rs/src/services/chapter_single_generation_existing_background_query_service.rs`
    now keeps only the query entry owner:
    - `load_owned_single_generation_existing_background_task_payload(...)`
    - owned read-state load
    - direct delegation to the dedicated payload owner
  - `backend-rs/src/services/chapter_single_generation_background_write_entry_service.rs`
    now consumes the narrowed query owner boundary instead of relying on a
    mixed query/projection file
  - `backend-rs/src/services/mod.rs`
    now registers the dedicated existing-background-payload owner file

  this is a real Phase 5 migration step because Rust now owns one tighter
  single-generation existing-background read-side chain rather than leaving
  payload projection mixed with the query file:
  - existing-background query owner:
    `owned task read-state -> payload owner`
  - existing-background payload owner:
    `read-state -> final compat payload`

  focused validation passed with:
  - `cargo fmt --manifest-path "backend-rs/Cargo.toml"`
  - `cargo test chapter_single_generation_existing_background_payload_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-existing-background-payload-owner" -- --nocapture`
  - `cargo test chapter_single_generation_existing_background_query_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-existing-background-payload-owner" -- --nocapture`
  - `cargo test chapter_single_generation_background_write_entry_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-existing-background-payload-owner" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-existing-background-payload-owner"`

  follow-up package entry:
  continue package B only where one remaining production lane still mixes
  single-generation write/query/public-shell semantics with adjacent owners;
  prefer another whole file or whole function-group collapse instead of
  helper-only relocation.

- 2026-06-06 single-generation stream-entry owner split checkpoint:
  this slice stayed on package B, `chapter_single_generation`, and continued
  the whole-owner migration path around the single stream lane.
  After the request/route contract, stream-result, runtime-outcome, and
  background-write entry owners were already narrowed, the remaining
  production mismatch was that
  `backend-rs/src/services/chapter_single_generation_stream_workflow_service.rs`
  still carried one mixed stream-start owner:
  - request -> runtime launch input preparation
  - runtime launch input -> stream lifecycle spawn

  in the current module shape, those directions no longer need to stay inside
  the same file:
  - stream entry owner only needs
    `request -> runtime launch input -> lifecycle handoff`
  - stream lifecycle owner only needs
    `runtime launch input -> stream lifecycle spawn / progress / success-failure emission`

  this checkpoint tightens that boundary further:
  - `backend-rs/src/services/chapter_single_generation_stream_entry_service.rs`
    now owns:
    - request -> runtime launch input materialization
    - handoff to stream lifecycle owner
    - file-local owner contract tests for runtime-input and spawn branches
  - `backend-rs/src/services/chapter_single_generation_stream_workflow_service.rs`
    now keeps only the stream lifecycle owner:
    - `start_owned_single_generation_stream_lifecycle(...)`
    - lifecycle spawn
    - progress / success / failure stream emission
  - `backend-rs/src/services/chapter_single_generation_route_workflow_service.rs`
    now delegates stream route-start directly to the dedicated stream-entry
    owner instead of the mixed workflow file
  - `backend-rs/src/services/mod.rs`
    now registers the dedicated stream-entry owner file

  this is a real Phase 5 migration step because Rust now owns one tighter
  single-generation stream start chain rather than leaving the entry owner
  mixed with the lifecycle file:
  - route/public shell:
    `route payload -> request normalization -> stream entry owner`
  - entry owner:
    `request -> runtime launch input -> lifecycle owner`
  - lifecycle owner:
    `runtime launch input -> stream spawn / progress / success / failure`

  focused validation passed with:
  - `cargo fmt --manifest-path "backend-rs/Cargo.toml"`
  - `cargo test chapter_single_generation_stream_entry_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-stream-entry-owner" -- --nocapture`
  - `cargo test chapter_single_generation_stream_workflow_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-stream-entry-owner" -- --nocapture`
  - `cargo test chapter_single_generation_route_workflow_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-stream-entry-owner" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-stream-entry-owner"`

  follow-up package entry:
  continue package B only where one remaining production lane still mixes
  single-generation stream/write/query public shells with adjacent lifecycle
  owners; prefer another whole file or whole function-group collapse instead
  of helper-only relocation.

- 2026-06-06 single-generation background-write-entry owner split checkpoint:
  this slice stayed on package B, `chapter_single_generation`, and continued
  the whole-owner migration path around the single background write lane.
  After the runtime-seed/restored-launch, existing-background read-state, and
  background-launch owners were already narrowed, the remaining production
  mismatch was that
  `backend-rs/src/services/chapter_single_generation_write_workflow_service.rs`
  still carried one mixed entry owner:
  - target load / existing-task short-circuit
  - launch-or-existing public entry shell

  in the current module shape, those directions no longer need to stay inside
  the same file as the public workflow-start shell:
  - background write entry owner only needs
    `request -> target load -> existing-task short-circuit or prepared launch`
  - public write workflow shell only needs
    `request -> timestamp shell -> background write entry start`

  this checkpoint tightens that boundary further:
  - `backend-rs/src/services/chapter_single_generation_background_write_entry_service.rs`
    now owns:
    - target load
    - existing-task payload short-circuit
    - prepared launch branch materialization
    - handoff to background launch owner
    - file-local owner contract tests for existing payload / launch branches
  - `backend-rs/src/services/chapter_single_generation_write_workflow_service.rs`
    now keeps only the public start shell:
    - `start_owned_single_generation_background_write_workflow(...)`
    - timestamp injection
    - direct delegation to background write entry owner
  - `backend-rs/src/services/mod.rs`
    now registers the dedicated background-write-entry owner file

  this is a real Phase 5 migration step because Rust now owns one tighter
  single-generation background write entry chain rather than leaving the entry
  owner mixed with the public workflow-start shell:
  - public shell:
    `request -> timestamp shell -> entry owner`
  - entry owner:
    `request -> target/existing branch -> launch owner`
  - launch owner:
    `launch parts -> task insert -> snapshot persist -> runtime dispatch`

  focused validation passed with:
  - `cargo fmt --manifest-path "backend-rs/Cargo.toml"`
  - `cargo test chapter_single_generation_background_write_entry_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-background-write-entry-owner" -- --nocapture`
  - `cargo test chapter_single_generation_write_workflow_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-background-write-entry-owner" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-background-write-entry-owner"`

  follow-up package entry:
  continue package B only where one remaining production lane still mixes
  single-generation write/query public shells with adjacent lifecycle owners;
  prefer another whole file or whole function-group collapse instead of
  helper-only relocation.

- 2026-06-06 single-generation background-launch owner split checkpoint:
  this slice stayed on package B, `chapter_single_generation`, and continued
  the whole-owner migration path around the single background write lane.
  After the runtime-seed/restored-launch and existing-background read-side
  owners were already narrowed, the remaining production mismatch was that
  `backend-rs/src/services/chapter_single_generation_write_workflow_service.rs`
  still carried two adjacent responsibilities in the same owner chain:
  - target/load existing-task branch selection
  - launch persistence / startup snapshot persist / runtime dispatch

  in the current module shape, those two directions no longer belong to one
  production owner:
  - write workflow entry only needs
    `request -> target load -> existing-task short-circuit or new launch`
  - background launch owner needs
    `prepared launch parts -> task insert -> startup snapshot persist -> runtime dispatch -> response payload`

  this checkpoint tightens that boundary further:
  - `backend-rs/src/services/chapter_single_generation_background_launch_service.rs`
    now owns:
    - `start_owned_single_generation_background_launch(...)`
    - background task insert
    - startup snapshot persistence
    - runtime dispatch
    - response payload return
    - test-only launch-parts materialization helpers used to verify the owner
      contract directly
  - `backend-rs/src/services/chapter_single_generation_write_workflow_service.rs`
    now keeps only the background workflow entry owner:
    - target load
    - existing-task payload short-circuit
    - launch-or-existing branch entry
  - the background launch owner now has file-local tests instead of remaining a
    production-only shell with `0 tests`

  this is a real Phase 5 migration step because Rust now owns one tighter
  single-generation background launch chain rather than leaving launch
  persistence and runtime dispatch as an implicit neighboring helper hop:
  - workflow entry:
    `request -> target/existing branch -> launch owner`
  - launch owner:
    `launch parts -> task insert -> snapshot persist -> runtime dispatch`

  focused validation passed with:
  - `cargo fmt --manifest-path "backend-rs/Cargo.toml"`
  - `cargo test chapter_single_generation_background_launch_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-background-launch-owner" -- --nocapture`
  - `cargo test chapter_single_generation_write_workflow_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-background-launch-owner" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-background-launch-owner"`

  follow-up package entry:
  continue package B only where one remaining production lane still mixes
  single-generation target loading / existing-task selection with adjacent
  write-workflow entry semantics; prefer another whole file or whole
  function-group collapse instead of helper-only relocation.

- 2026-06-06 single-generation route-workflow owner split checkpoint:
  this slice stayed on package B, `chapter_single_generation`, and selected
  the remaining route-facing workflow-entry shell as one coherent owner file.
  After the previous request/route contract owner checkpoint, the remaining
  production mismatch was that route entry semantics still lived across two
  neighboring workflow files:
  - stream route payload -> request -> stream workflow start
  - background route payload -> request -> write workflow start

  In the current module shape, those two directions no longer belong inside
  the stream/write lifecycle owners themselves. They own one explicit adjacent
  contract:
  - `route payload + request normalization -> workflow start entry`

  This checkpoint tightens the package boundary further:
  - `backend-rs/src/services/chapter_single_generation_route_workflow_service.rs`
    now owns:
    - stream route payload -> request -> stream workflow entry
    - background route payload -> request -> write workflow entry
    - single-generation route-facing workflow start shell
  - `backend-rs/src/services/chapter_single_generation_stream_workflow_service.rs`
    now keeps only the stream lifecycle owner:
    - request -> runtime launch input -> stream lifecycle spawn
    - SSE generation / success / failure lifecycle
  - `backend-rs/src/services/chapter_single_generation_write_workflow_service.rs`
    now keeps only the background workflow owner:
    - request -> target/read-state selection -> launch persistence/dispatch
  - `backend-rs/src/api/chapter_generation_routes.rs`
    now depends on the dedicated route-workflow owner directly instead of
    choosing between two workflow files for route-facing start shells.

  This is counted as real Phase 5 migration progress because it removes one
  more mixed route-facing/workflow-lifecycle boundary from the active
  single-generation package and leaves three clearer neighboring owners:
  - request contract owner:
    `route payload + compat defaults -> validated single-generation request`
  - route-workflow owner:
    `route payload + request normalization -> workflow start entry`
  - stream/write lifecycle owners:
    `request -> stream/background runtime lifecycle`

  Validation:
  - `cargo fmt --manifest-path "backend-rs/Cargo.toml"`
  - `cargo test chapter_single_generation_route_workflow_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-route-workflow-owner" -- --nocapture`
  - `cargo test chapter_generation_routes --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-route-workflow-owner" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-route-workflow-owner"`

  Follow-up package entry:
  continue package B only where one remaining production lane still mixes
  single-generation read/query ownership or stream/write lifecycle internals
  with adjacent owners; prefer another whole file or whole function-group
  collapse instead of helper-only relocation.

- 2026-06-06 single-generation stream-result owner split checkpoint:
  this slice stayed on package B, `chapter_single_generation`, and selected
  the remaining stream success/result projection chain as one coherent owner
  file. After the previous route-workflow owner checkpoint, the remaining
  production mismatch was that
  `backend-rs/src/services/chapter_single_generation_stream_workflow_service.rs`
  still mixed two adjacent responsibilities in the same file:
  - stream lifecycle spawn / execute / failure shell
  - success analysis/result projection / quality gate / story runtime contract

  In the current module shape, those two directions no longer belong to the
  same production owner:
  - stream lifecycle lanes only need
    `request -> runtime launch input -> stream lifecycle spawn/failure`
  - success/result lanes need
    `generated result -> analysis projection -> quality/result emission contract`

  This checkpoint tightens the package boundary further:
  - `backend-rs/src/services/chapter_single_generation_stream_result_service.rs`
    now owns:
    - `SingleGenerationStreamSuccessArtifacts`
    - `SingleGenerationStreamSuccessEventPayload`
    - follow-up analysis / quality gate / story runtime contract projection
    - ordered SSE success emission planning and result payload projection
  - `backend-rs/src/services/chapter_single_generation_stream_workflow_service.rs`
    now keeps only the stream lifecycle owner:
    - request -> runtime launch input -> stream lifecycle spawn
    - execute generation and failure-path SSE shell
  - route and workflow neighbors now depend on the dedicated stream-result
    owner directly instead of reaching back into the mixed lifecycle file for
    success/result semantics

  This is counted as real Phase 5 migration progress because it removes one
  more mixed stream-lifecycle/result boundary from the active
  single-generation package and leaves two explicit neighboring owners:
  - stream lifecycle owner:
    `request -> runtime launch input -> spawn/failure shell`
  - stream result owner:
    `generated result -> analysis projection -> quality/result emission`

  Validation:
  - `cargo fmt --manifest-path "backend-rs/Cargo.toml"`
  - `cargo test chapter_single_generation_stream_result_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-stream-result-owner" -- --nocapture`
  - `cargo test chapter_single_generation_stream_workflow_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-stream-result-owner" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-stream-result-owner"`

  Follow-up package entry:
  continue package B only where one remaining production lane still mixes
  single-generation runtime lifecycle or background workflow launch with
  adjacent result/query owners; prefer another whole file or whole
  function-group collapse instead of helper-only relocation.

- 2026-06-06 single-generation request/route contract owner split checkpoint:
  this slice stayed on package B, `chapter_single_generation`, and selected
  the remaining request/route contract chain as one coherent owner file. After
  the previous runtime-seed/restored-launch owner checkpoint, the remaining
  production mismatch was that
  `backend-rs/src/services/chapter_single_generation_prepare_service.rs`
  still mixed two adjacent responsibilities in the same file:
  - route/request schema, compat defaults, request normalization, and
    validation error shell
  - accessible target loading and prepare-stage target contract

  In the current module shape, those two directions no longer belong to the
  same production owner:
  - request-entry lanes only need
    `route payload + compat defaults -> validated single-generation request`
  - prepare lanes only need
    `validated single-generation request + chapter access -> validated target`

  This checkpoint tightens the package boundary further:
  - `backend-rs/src/services/chapter_single_generation_request_service.rs`
    now owns:
    - `SingleChapterGenerationRouteRequest`
    - `SingleChapterGenerationRequest`
    - `SingleChapterGenerationCompatOptions`
    - `PrepareSingleChapterGenerationRequestError`
    - route payload normalization and validation
    - Python-compatible request defaults and compat contract
  - `backend-rs/src/services/chapter_single_generation_prepare_service.rs`
    now keeps only the prepare owner:
    - `SingleChapterGenerationExecutionInput`
    - `SingleChapterGenerationTarget`
    - accessible target loading and prepare-stage access contract
    - transitional re-export for request error / compat options
  - route/runtime/query consumers now depend on the dedicated request owner
    directly instead of reaching back into the mixed prepare file for request
    contract semantics:
    - `backend-rs/src/api/chapter_generation_routes.rs`
    - `backend-rs/src/services/chapter_generation_request_runtime_state_service.rs`
    - `backend-rs/src/services/chapter_story_repair_quality_context_service.rs`
    - `backend-rs/src/services/chapter_generation_research_payload_service.rs`
    - `backend-rs/src/services/chapter_regeneration_prepare_service.rs`
    - `backend-rs/src/services/chapter_single_generation_runtime_state_service.rs`
    - `backend-rs/src/services/chapter_single_generation_runtime_seed_service.rs`
    - `backend-rs/src/services/chapter_single_generation_stream_workflow_service.rs`
    - `backend-rs/src/services/chapter_single_generation_write_workflow_service.rs`

  This is counted as real Phase 5 migration progress because it removes one
  more mixed request/prepare boundary from the active single-generation package
  and leaves two explicit neighboring owners:
  - request contract owner:
    `route payload + compat defaults -> validated single-generation request`
  - prepare owner:
    `validated single-generation request + chapter access -> validated target`

  Validation:
  - `cargo fmt --manifest-path "backend-rs/Cargo.toml"`
  - `cargo test chapter_single_generation_request_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-request-owner" -- --nocapture`
  - `cargo test chapter_single_generation_prepare_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-request-owner" -- --nocapture`
  - `cargo test chapter_single_generation_stream_workflow_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-request-owner" -- --nocapture`
  - `cargo test chapter_single_generation_write_workflow_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-request-owner" -- --nocapture`
  - `cargo test chapter_generation_routes --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-request-owner" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-request-owner"`

  Follow-up package entry:
  continue package B only where one remaining production lane still mixes
  single-generation route-facing/public-start shells or read/query ownership
  with adjacent owner files; prefer another whole file or whole function-group
  collapse instead of helper-only relocation.

- 2026-06-06 single-generation runtime-seed/restored-launch owner split checkpoint:
  this slice stayed on package B, `chapter_single_generation`, and selected
  the remaining runtime-seed / restored-launch chain as one coherent owner
  file. After the previous quality/manual-review owner checkpoint, the
  remaining production mismatch was that
  `backend-rs/src/services/chapter_single_generation_prepare_service.rs`
  still mixed two adjacent responsibilities in the same file:
  - request validation / target loading / prepare-stage public contract
  - restored runtime state / startup seed / runtime launch materialization

  In the current module shape, those two directions no longer belong to the
  same production owner:
  - prepare entry lanes only need
    `route request + target access -> validated single-generation target`
  - runtime seed lanes need
    `validated target + runtime state sources -> startup snapshot + runtime launch input`

  This checkpoint tightens the package boundary further:
  - `backend-rs/src/services/chapter_single_generation_runtime_seed_service.rs`
    now owns:
    - restored runtime-state seed restoration
    - recent-history summary aggregation for single-generation repair
    - startup snapshot seed materialization
    - runtime launch input materialization
    - background launch parts materialization
    - runtime compat restore from seeded payload
  - `backend-rs/src/services/chapter_single_generation_prepare_service.rs`
    now keeps only the prepare owner:
    - route payload normalization
    - request validation
    - accessible chapter target loading
    - prepare-stage request/target public contract
  - `backend-rs/src/services/chapter_single_generation_stream_workflow_service.rs`
    and
    `backend-rs/src/services/chapter_single_generation_write_workflow_service.rs`
    now consume the dedicated runtime-seed owner directly instead of reaching
    back into the mixed prepare file for restored-launch materialization.

  This is counted as real Phase 5 migration progress because it removes one
  more mixed prepare/runtime-seed boundary from the active
  single-generation package and leaves two explicit neighboring owners:
  - prepare owner:
    `route request + chapter access -> validated single-generation target`
  - runtime-seed owner:
    `runtime-state sources + target -> startup snapshot + runtime launch parts`

  Validation:
  - `cargo fmt --manifest-path "backend-rs/Cargo.toml"`
  - `cargo test chapter_single_generation_runtime_seed_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-runtime-seed-owner" -- --nocapture`
  - `cargo test chapter_single_generation_prepare_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-runtime-seed-owner" -- --nocapture`
  - `cargo test chapter_single_generation_write_workflow_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-runtime-seed-owner" -- --nocapture`
  - `cargo test chapter_single_generation_stream_workflow_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-runtime-seed-owner" -- --nocapture`
  - `cargo test chapter_batch_generation_runtime_state_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-runtime-seed-owner" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-runtime-seed-owner"`

  Follow-up package entry:
  continue package B only where one remaining production lane still mixes
  single-generation read/query ownership or route-facing/public-start shells
  with adjacent owner files; prefer another whole file or whole function-group
  collapse instead of helper-only relocation.

- 2026-06-06 single-generation quality/manual-review owner split checkpoint:
  this slice stayed on package B, `chapter_single_generation`, and selected
  the remaining mixed quality-status file as one coherent function-group
  migration unit. After the previous existing-background read-state owner
  checkpoint, the remaining production mismatch was that
  `backend-rs/src/services/chapter_single_generation_quality_status_service.rs`
  still mixed two adjacent responsibilities in the same file:
  - read-side quality context materialization and payload projection support
  - runtime-side manual-review label resolution for follow-up analysis

  In the current module shape, those two directions no longer belong to the
  same production owner:
  - existing-background read-state and payload lanes only need
    `snapshot/runtime_state -> quality status context`
  - runtime lifecycle only needs
    `analysis payload -> manual review decision label`

  This checkpoint tightens the package boundary further:
  - `backend-rs/src/services/chapter_single_generation_quality_status_service.rs`
    now keeps only the read-side quality-context owner:
    - `SingleGenerationQualityStatusContext`
    - `from_snapshot_and_runtime_state(...)`
    - `insert_into_payload(...)`
    - `from_runtime_quality_context_and_active_payload(...)`
  - `backend-rs/src/services/chapter_single_generation_manual_review_service.rs`
    now owns the runtime-side manual-review decision chain:
    - `manual_review_label_from_single_generation_quality_context(...)`
    - payload/manual-review label fallback resolution
  - `backend-rs/src/services/chapter_single_generation_runtime_state_service.rs`
    now consumes the dedicated manual-review owner directly instead of
    depending on the mixed quality-status file for runtime follow-up analysis
    semantics.

  This is counted as real Phase 5 migration progress because it removes one
  more mixed read-side/runtime-side boundary from the active
  single-generation package and leaves two explicit neighboring owners:
  - quality-context read-side owner:
    `snapshot/runtime_state -> quality payload context`
  - manual-review runtime owner:
    `analysis payload -> manual review decision label`

  Validation:
  - `cargo fmt --manifest-path "backend-rs/Cargo.toml"`
  - `cargo test chapter_single_generation_manual_review_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-quality-manual-review-owner" -- --nocapture`
  - `cargo test chapter_single_generation_quality_status_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-quality-manual-review-owner" -- --nocapture`
  - `cargo test chapter_single_generation_runtime_state_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-quality-manual-review-owner" -- --nocapture`
  - `cargo test chapter_single_generation_existing_background_read_state_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-quality-manual-review-owner" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-quality-manual-review-owner"`

  Follow-up package entry:
  continue package B only where one remaining production lane still mixes
  single-generation read/query, runtime lifecycle, or startup/runtime payload
  semantics across neighboring owner files; prefer another whole file or
  whole function-group collapse instead of helper-only relocation.

- 2026-06-06 single-generation existing-background read-state owner file-collapse checkpoint:
  this slice stayed on package B, `chapter_single_generation`, and selected
  the existing-background read-state chain as one coherent owner file. After
  the previous startup-snapshot owner checkpoint, the remaining production
  mismatch was that
  `backend-rs/src/services/chapter_single_generation_existing_background_query_service.rs`
  still mixed two adjacent responsibilities in the same file:
  - owned task / recovery / snapshot loading
  - final existing-background payload projection

  In the current module shape, the read-state chain no longer belonged to the
  final payload projection contract. It owned one explicit local boundary:
  - `owned task + recover + snapshot -> single-generation read state`

  This checkpoint tightens the package boundary further:
  - `backend-rs/src/services/chapter_single_generation_existing_background_read_state_service.rs`
    now owns:
    - `SingleGenerationExistingBackgroundTaskReadState`
    - active background task loading
    - recovery-aware active-task filtering
    - snapshot map loading
    - chapter membership matching
    - final owned read-state selection
  - `backend-rs/src/services/chapter_single_generation_existing_background_query_service.rs`
    now keeps only:
    - `load_owned_single_generation_existing_background_task_payload(...)`
    - `build_single_generation_existing_background_task_payload_from_read_state(...)`
  - `backend-rs/src/services/chapter_single_generation_write_workflow_service.rs`
    continues to consume the query owner, but the query owner now depends on a
    dedicated read-state owner instead of hosting the lower-level read chain
    inline.

  This is counted as real Phase 5 migration progress because it removes one
  more mixed read-state/payload-projection boundary from the active
  single-generation package and leaves two explicit local owners:
  - existing-background read-state owner:
    `owned task + recover + snapshot -> read state`
  - existing-background payload owner:
    `read state -> final payload projection`

  Validation:
  - `cargo fmt --manifest-path "backend-rs/Cargo.toml"`
  - `cargo test chapter_single_generation_existing_background_read_state_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-existing-background-read-state-owner" -- --nocapture`
  - `cargo test chapter_single_generation_existing_background_query_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-existing-background-read-state-owner" -- --nocapture`
  - `cargo test chapter_single_generation_write_workflow_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-existing-background-read-state-owner" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-existing-background-read-state-owner"`

  Follow-up package entry:
  continue package B only where one remaining production lane still mixes
  single-generation quality-status projection with neighboring payload owners
  or where route/query ownership can be collapsed as a whole file; avoid
  helper-only relocations.

- 2026-06-06 single-generation startup-snapshot owner file-collapse checkpoint:
  this slice stayed on package B, `chapter_single_generation`, and selected
  the single-generation startup snapshot planning contract as one coherent
  owner file. After the previous runtime-checkpoint owner checkpoint, the
  remaining production mismatch was that
  `backend-rs/src/services/chapter_single_generation_snapshot_service.rs`
  still mixed two adjacent responsibilities in the same file:
  - startup snapshot planning / runtime-state merge
  - lower-level runtime snapshot persistence

  In the current module shape, startup snapshot planning no longer belonged to
  the lower-level `task id + runtime state -> snapshot upsert` owner. It owned
  one explicit local contract:
  - `pending checkpoint + runtime state seed -> startup snapshot plan`

  This checkpoint tightens the package boundary further:
  - `backend-rs/src/services/chapter_single_generation_startup_snapshot_service.rs`
    now owns:
    - `merge_single_generation_runtime_state(...)`
    - `SingleGenerationStartupSnapshotPlan`
    - `SingleGenerationStartupSnapshotPlan::persist(...)`
  - `backend-rs/src/services/chapter_single_generation_snapshot_service.rs`
    now keeps only the lower-level
    `upsert_single_generation_runtime_snapshot(...)` persistence boundary.
  - `backend-rs/src/services/chapter_single_generation_prepare_service.rs`
    and
    `backend-rs/src/services/chapter_single_generation_write_workflow_service.rs`
    now consume the startup snapshot owner directly instead of reaching into a
    mixed snapshot/persistence file.

  This is counted as real Phase 5 migration progress because it removes one
  more mixed startup-planning/snapshot-persistence boundary from the active
  single-generation package and leaves two explicit local owners:
  - startup snapshot owner:
    `pending checkpoint + runtime state seed -> startup snapshot plan`
  - snapshot persistence owner:
    `task id + runtime state -> snapshot upsert`

  Validation:
  - `cargo fmt --manifest-path "backend-rs/Cargo.toml" --check`
  - `cargo test chapter_single_generation_startup_snapshot_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-startup-snapshot-owner" -- --nocapture`
  - `cargo test chapter_single_generation_prepare_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-startup-snapshot-owner" -- --nocapture`
  - `cargo test chapter_single_generation_write_workflow_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-startup-snapshot-owner" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-startup-snapshot-owner"`

  Follow-up package entry:
  continue package B only where one remaining production lane still mixes
  existing-background read/query ownership or quality-status projection with a
  neighboring owner; prefer another whole file or whole function-group
  collapse instead of helper-only moves.

- 2026-06-06 single-generation runtime-checkpoint owner file-collapse checkpoint:
  this slice stayed on package B, `chapter_single_generation`, and selected
  the single-generation runtime checkpoint contract as one coherent owner file.
  After the previous task-view payload owner checkpoint, the remaining
  production mismatch was that
  `backend-rs/src/services/chapter_single_generation_runtime_state_service.rs`
  still mixed two adjacent responsibilities in the same file:
  - runtime lifecycle execution
  - runtime checkpoint stage projection and stage persistence

  In the current module shape, the checkpoint group no longer belonged to
  prompt overrides, generation execution, follow-up analysis, or runtime
  lifecycle spawning. It owned one explicit local contract:
  - `task stage -> runtime checkpoint stage -> snapshot persistence`

  This checkpoint tightens the package boundary further:
  - `backend-rs/src/services/chapter_single_generation_runtime_checkpoint_service.rs`
    now owns:
    - `SingleGenerationSnapshotStage`
    - `build_single_generation_runtime_checkpoint_for_stage(...)`
    - `SingleGenerationTaskStage::persist_runtime_preparation(...)`
    - `SingleGenerationTaskStage::persist_with_checkpoint(...)`
  - `backend-rs/src/services/chapter_single_generation_runtime_state_service.rs`
    now consumes that checkpoint owner for preparation/completed/failed stage
    persistence instead of hosting the same checkpoint semantics locally.
  - `backend-rs/src/services/chapter_single_generation_prepare_service.rs`
    now consumes the checkpoint owner directly for pending checkpoint
    projection, instead of reaching into the runtime lifecycle owner file.

  This is counted as real Phase 5 migration progress because it removes one
  more mixed runtime-execution/checkpoint-persistence boundary from the active
  single-generation package and leaves two explicit local owners:
  - runtime lifecycle owner:
    `runtime launch input -> execute generation -> follow-up analysis`
  - runtime checkpoint owner:
    `task stage -> checkpoint projection -> snapshot persistence`

  Validation:
  - `cargo fmt --manifest-path "backend-rs/Cargo.toml" --check`
  - `cargo test chapter_single_generation_runtime_checkpoint_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-runtime-checkpoint-owner" -- --nocapture`
  - `cargo test chapter_single_generation_runtime_state_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-runtime-checkpoint-owner" -- --nocapture`
  - `cargo test chapter_single_generation_prepare_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-runtime-checkpoint-owner" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-runtime-checkpoint-owner"`

  Follow-up package entry:
  continue package B only where one remaining production lane still mixes
  runtime lifecycle, startup snapshot, or existing-background query ownership;
  prefer another whole file or whole function-group collapse instead of
  helper-only relocation.

- 2026-06-06 single-generation task-view payload owner file-collapse checkpoint:
  this slice stayed on package B, `chapter_single_generation`, and selected
  the single-generation task-view payload base as one coherent owner file.
  After the previous background payload-base and existing-background query
  checkpoints, the remaining production mismatch was that one single-chapter
  read-side payload contract still lived inside
  `chapter_single_generation_prepare_service.rs` even though it was no longer
  part of request preparation:
  - `estimated_single_generation_task_minutes(...)`
  - `single_generation_pending_stage_code()`
  - `single_generation_active_task_statuses()`
  - `build_single_generation_runtime_payload_base(...)`
  - `build_single_generation_task_view_payload_from_task_state(...)`

  In the current module shape, those helpers no longer owned request
  validation, target lookup, restored-launch materialization, or launch
  preparation semantics. They only replayed one single-generation-local
  read-side chain used by:
  - background create response payload projection
  - existing-background task payload projection

  This checkpoint tightens the package boundary further:
  - `backend-rs/src/services/chapter_single_generation_task_view_payload_service.rs`
    now owns the full single-generation task-view payload base contract.
  - `backend-rs/src/services/chapter_single_generation_prepare_service.rs`
    now consumes that local owner only for background create payload
    projection, instead of directly hosting the same read-side payload base.
  - `backend-rs/src/services/chapter_single_generation_existing_background_query_service.rs`
    now consumes that same local owner directly for existing-background task
    payload projection, instead of reaching back into the prepare owner.

  This is counted as real Phase 5 migration progress because it removes one
  more mixed prepare/read-side ownership boundary from the active
  single-generation package and leaves both create/existing payload lanes
  attached to one explicit single-generation-local owner file:
  - `task state/runtime state -> single-generation payload base -> create/existing payload`

  Validation:
  - `cargo fmt --manifest-path "backend-rs/Cargo.toml" --check`
  - `cargo test chapter_single_generation_task_view_payload_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-task-view-payload-owner-file" -- --nocapture`
  - `cargo test chapter_single_generation_existing_background_query_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-task-view-payload-owner-file" -- --nocapture`
  - `cargo test chapter_single_generation_prepare_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-task-view-payload-owner-file" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-task-view-payload-owner-file"`

  Follow-up package entry:
  continue package B only where one remaining production lane still mixes
  single-generation request preparation with read-side/query/runtime owner
  chains; do not count helper-only moves that do not clarify the active Rust
  owner boundary.

- 2026-06-06 single-generation public-start request-wrapper collapse checkpoint:
  this slice stayed on package B, `chapter_single_generation`, and selected
  the stream/background public-start function group as one coherent owner
  boundary. After the previous workflow-wrapper and prepare-owner checkpoints,
  the remaining production mismatch was that both neighboring files still
  preserved two public entrypoints per lane:
  - one `*_from_route_payload(...)` route-facing start
  - one request-facing public start that only replayed the same owner chain

  In the current module shape, those request-facing public starts no longer
  owned access control, request validation, request normalization, timestamp
  policy beyond the immediate call, error translation, or semantic branching.
  They only replayed one already-owned path:
  - background:
    `request -> SingleGenerationBackgroundWorkflowEntry::start(...)`
  - stream:
    `request -> prepare_single_generation_runtime_launch_input(...) -> lifecycle.spawn(...)`

  This checkpoint tightens the package boundary further:
  - `backend-rs/src/services/chapter_single_generation_write_workflow_service.rs`
    now keeps `start_owned_single_generation_background_write_workflow_from_route_payload(...)`
    as the single public background start boundary and lets it call
    `SingleGenerationBackgroundWorkflowEntry::start(...)` directly after
    route-payload normalization.
  - `backend-rs/src/services/chapter_single_generation_stream_workflow_service.rs`
    now keeps `create_single_generation_stream_workflow_from_route_payload(...)`
    as the single public stream start boundary and lets it normalize the route
    payload, prepare runtime launch input, and spawn the lifecycle owner
    directly.
  - the redundant request-facing public starts were removed because they no
    longer owned an independent production contract.

  This is counted as real Phase 5 migration progress because it removes one
  more pair of public forwarding seams from the active single-generation
  package and leaves both route-facing production chains closer to one explicit
  Rust owner boundary:
  - background:
    `route payload -> workflow entry start -> persist/dispatch`
  - stream:
    `route payload -> prepare runtime input -> lifecycle spawn/run`

  Validation:
  - `cargo fmt --manifest-path "backend-rs/Cargo.toml" --check`
  - `cargo test chapter_single_generation_stream_workflow_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-public-start-collapse" -- --nocapture`
  - `cargo test chapter_single_generation_write_workflow_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-public-start-collapse" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-public-start-collapse"`

  Follow-up package entry:
  continue package B only where one remaining production lane still keeps a
  neighboring route/start wrapper, cross-file handoff, or mixed route/prepare/
  write/runtime ownership; do not count test-only or naming-only cleanup as
  migration progress.

- 2026-06-06 single-generation stream workflow-start wrapper checkpoint:
  this slice stayed on package B, `chapter_single_generation`, and selected
  the stream workflow public-start boundary as one coherent wrapper-collapse
  unit. After the prepare-owner direct-output checkpoint, the remaining
  production mismatch was that `chapter_single_generation_stream_workflow_service.rs`
  still preserved `SingleGenerationStreamWorkflowStart` as a neighboring
  single-call wrapper over:
  - `prepare_single_generation_runtime_launch_input(...)`
  - `SingleGenerationStreamLifecyclePlan::from_runtime_launch(...)`
  - `spawn(...)`

  That wrapper no longer added request normalization, validation ownership,
  timestamp ownership, error translation, or branching. It only replayed one
  already-owned chain:
  `prepare runtime input -> lifecycle owner -> spawn stream`

  This checkpoint tightens the package boundary further:
  - `backend-rs/src/services/chapter_single_generation_stream_workflow_service.rs`
    now lets the public stream entrypoint call
    `prepare_single_generation_runtime_launch_input(...)` directly and then
    dispatch straight into
    `SingleGenerationStreamLifecyclePlan::from_runtime_launch(...).spawn(...)`
  - `SingleGenerationStreamWorkflowStart` has been removed because it no
    longer owned an independent production contract
  - stream-lifecycle tests now assert the lifecycle owner directly instead of
    asserting the removed wrapper shell

  This is counted as real Phase 5 migration progress because it removes one
  more production wrapper from the active single-generation stream lane and
  leaves the runtime stream path closer to the final owner chain:
  `request -> prepare runtime input -> lifecycle spawn/run`

  Validation:
  - `cargo test chapter_single_generation_stream_workflow_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-stream-wrapper-collapse" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-stream-wrapper-collapse"`
  - `cargo fmt --manifest-path "backend-rs/Cargo.toml" --check`

  Follow-up package entry:
  continue package B only where one remaining production lane still keeps a
  neighboring wrapper, cross-file handoff, or mixed route/prepare/write/
  runtime ownership; do not count test-only naming cleanup as migration
  progress.

- 2026-06-06 single-generation background write-owner execution checkpoint:
  this slice stayed on package B, `chapter_single_generation`, and selected
  the background write execution group as one coherent owner boundary. After
  the previous direct-output prepare checkpoint, the remaining production
  mismatch was that `PreparedSingleGenerationBackgroundLaunchParts` still
  carried its own `persist_and_dispatch(...)` execution owner inside
  `chapter_single_generation_prepare_service.rs`.

  That meant the single-generation prepare file still owned write-side task
  insertion, startup snapshot persistence, and runtime dispatch for the
  background lane even though those semantics belong to the write workflow
  module, not to request preparation.

  This checkpoint tightens the package boundary further:
  - `backend-rs/src/services/chapter_single_generation_prepare_service.rs`
    now stops at preparing `PreparedSingleGenerationBackgroundLaunchParts`.
    It no longer owns background task insert, startup snapshot persistence, or
    runtime dispatch execution.
  - `backend-rs/src/services/chapter_single_generation_write_workflow_service.rs`
    now owns the full background execution boundary through
    `persist_and_dispatch_single_generation_background_launch(...)`.
  - the active production chain is now clearer:
    `request -> prepare owner -> launch parts -> write workflow persist/dispatch`

  This is counted as real Phase 5 migration progress because it removes one
  more mixed prepare/write boundary inside the active single-generation
  package and makes the write workflow the explicit Rust owner of:
  - task insert
  - startup snapshot persistence
  - runtime dispatch

  Validation:
  - `cargo test chapter_single_generation_write_workflow_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-write-owner-collapse" -- --nocapture`
  - `cargo test chapter_single_generation_prepare_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-write-owner-collapse" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-write-owner-collapse"`
  - `cargo fmt --manifest-path "backend-rs/Cargo.toml" --check`

  Follow-up package entry:
  continue package B only where one neighboring production lane still mixes
  route/prepare/write/runtime ownership in the same cross-file hop; avoid
  counting test-only owner reshapes as migration progress.

- 2026-06-06 single-generation prepare-owner direct-output checkpoint:
  this slice stayed on package B, `chapter_single_generation`, and selected
  one whole prepare-owner function group instead of another route-only seam.
  The production issue was not HTTP transport ownership anymore; it was that
  neighboring single-generation stream and background write workflow lanes
  still imported the cross-file intermediate owner type
  `PreparedSingleChapterGenerationRestoredRuntimeLaunch` directly in order to
  obtain their final runtime/background launch products.

  That made the prepare boundary look half-collapsed: Rust already owned the
  restored-launch materialization internally, but the surrounding production
  lanes still depended on the same intermediate owner type instead of one
  explicit prepare-service output boundary.

  This checkpoint narrows the package owner boundary further:
  - `backend-rs/src/services/chapter_single_generation_prepare_service.rs`
    now exports direct prepare-owner APIs for the two production outputs:
    - `prepare_single_generation_runtime_launch_input(...)`
    - `prepare_single_generation_background_launch_parts_from_target(...)`
  - `backend-rs/src/services/chapter_single_generation_stream_workflow_service.rs`
    now consumes the prepare owner through
    `prepare_single_generation_runtime_launch_input(...)` instead of
    importing the restored-launch intermediate type directly.
  - `backend-rs/src/services/chapter_single_generation_write_workflow_service.rs`
    now consumes the prepare owner through
    `prepare_single_generation_background_launch_parts_from_target(...)`
    instead of importing the restored-launch intermediate type directly in the
    production owner chain.
  - `PreparedSingleChapterGenerationRestoredRuntimeLaunch` remains inside the
    prepare owner and contract-focused tests; it is no longer part of the
    production cross-file API for the stream/background start lanes.

  This is counted as real Phase 5 migration progress because it removes one
  more intermediate cross-file owner hop inside the active single-generation
  package and clarifies the Rust prepare boundary as:
  `request -> prepare owner -> final launch product`.

  Validation:
  - `cargo test chapter_single_generation_prepare_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-prepare-owner-api" -- --nocapture`
  - `cargo test chapter_single_generation_write_workflow_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-prepare-owner-api" -- --nocapture`
  - `cargo test chapter_single_generation_stream_workflow_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-prepare-owner-api" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-prepare-owner-api"`
  - `cargo fmt --manifest-path "backend-rs/Cargo.toml" --check`

  Follow-up package entry:
  continue package B by collapsing the next whole function group only where a
  production lane still reopens startup/runtime/background materialization
  from a neighboring owner, rather than by counting test-only helper moves as
  migration progress.

- 2026-06-06 single-generation route-file owner evidence checkpoint:
  this slice selected package B, `chapter_single_generation`, and treated the
  Python `backend/app/api/chapter_generation_routes.py` file as one coherent
  migration package instead of advancing only one endpoint seam. The Python
  file contains exactly two single-chapter generation endpoints:
  `POST /api/chapters/{chapter_id}/generate-stream` and
  `POST /api/chapters/{chapter_id}/generate-background`.

  Rust already owns both primary routes through
  `backend-rs/src/api/chapter_generation_routes.rs`, but the test evidence was
  asymmetric: the Phase 5 P0 profile test asserted only the background owner
  probe, and the fallback profile test asserted only the background fallback
  probe. This made the Python route file look half-migrated even though the
  manifest and gateway routing already carried both endpoints.

  This checkpoint closes that package-level evidence gap:
  - `backend-rs/src/api/chapter_generation_routes.rs` now keeps a small
    same-file route owner manifest for the two single-generation endpoints and
    tests that the whole route file stays Rust-owned as a pair.
  - `backend/tests/test_tools/test_run_strangler_gateway_smoke.py` now asserts
    both `chapters-generate-background-auth-guard-rust` and
    `chapters-generate-stream-auth-guard-rust` in `phase5-p0`.
  - the same smoke tool test now asserts both
    `chapters-generate-background-auth-guard-python-fallback` and
    `chapters-generate-stream-auth-guard-python-fallback` in
    `phase5-p0-fallback`.
  - `backend/app/api/chapter_generation_routes.py` remains a frozen Python
    fallback shell; it must not be deleted until rollback/business smoke
    evidence no longer depends on the same-path fallback profile.

  Validation:
  - `python -m pytest "backend/tests/test_tools/test_run_strangler_gateway_smoke.py" -q`
  - `cargo test chapter_generation_routes --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-route-file-owner" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-route-file-owner"`
  - `cargo fmt --manifest-path "backend-rs/Cargo.toml" --check`

  Follow-up package entry:
  continue package B by either shrinking the frozen Python fallback shell only
  after rollback evidence is upgraded, or move the next whole-function group
  inside `chapter_single_generation` where stream/background still share task
  lifecycle, checkpoint, or SSE completion semantics.

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

- 2026-06-06 single-generation snapshot service file-collapse checkpoint:
  this slice stayed on the same `chapter_single_generation` Phase 5 module
  package and closed the next real owner gap after the existing-background
  query lane and background-write lane had already been tightened. Before this
  change, the single-generation module already had three concrete snapshot
  producers:
  - startup snapshot persistence
  - runtime checkpoint persistence
  - runtime outcome/manual-review persistence

  but `backend-rs/src/services/chapter_single_generation_snapshot_service.rs`
  still remained as one extra file-local forwarding shell between those
  single-generation owners and the real shared snapshot persistence boundary.
  That file no longer owned any independent behavior. It only replayed:
  - `single runtime state -> shared chapter snapshot merge/persist`
  - `Utc::now().naive_utc()` timestamp assignment

  `backend-rs/src/services/chapter_generation_snapshot_persistence_service.rs`
  already owns the real shared write contract:
  - `task id + runtime state + write timestamp -> merge/persist snapshot`

  this checkpoint tightens the boundary further:
  - `backend-rs/src/services/chapter_single_generation_startup_snapshot_service.rs`
    now calls the shared snapshot persistence owner directly for startup-state
    persistence
  - `backend-rs/src/services/chapter_single_generation_runtime_checkpoint_service.rs`
    now calls the shared snapshot persistence owner directly for stage/checkpoint
    writes
  - `backend-rs/src/services/chapter_single_generation_runtime_outcome_service.rs`
    now calls the shared snapshot persistence owner directly for
    quality-blocked/manual-review snapshot writes
  - `backend-rs/src/services/mod.rs`
    now drops the shell-only
    `chapter_single_generation_snapshot_service` module registration
  - deleted shell file:
    - `backend-rs/src/services/chapter_single_generation_snapshot_service.rs`

  This is a real Phase 5 migration step because Rust now owns one tighter
  single-generation snapshot-producer -> shared snapshot-persistence chain
  rather than preserving a single-generation-only forwarding shell that adds
  no validation, no semantic branching, and no independent error contract:
  - startup:
    `startup snapshot owner -> shared snapshot persistence owner`
  - checkpoint:
    `runtime checkpoint owner -> shared snapshot persistence owner`
  - outcome:
    `runtime outcome owner -> shared snapshot persistence owner`

  Focused validation passed with:
  `cargo test chapter_single_generation_runtime_checkpoint_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-snapshot-service-collapse" -- --nocapture`
  `cargo test chapter_single_generation_runtime_outcome_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-snapshot-service-collapse" -- --nocapture`
  `cargo test chapter_single_generation_startup_snapshot_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-snapshot-service-collapse" -- --nocapture`
  `cargo test chapter_single_generation --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-snapshot-service-collapse" -- --nocapture`
  `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-snapshot-service-collapse"`

- 2026-06-06 single-generation stream-entry file-collapse checkpoint:
  this slice stayed on the same `chapter_single_generation` Phase 5 module
  package and closed the next real owner gap after the snapshot-producer lane
  had already been tightened. Before this change, the single-generation stream
  lane already had:
  - request normalization owner
  - runtime launch-input preparation owner
  - stream lifecycle owner

  but `backend-rs/src/services/chapter_single_generation_stream_entry_service.rs`
  still remained as one extra file-local public-entry shell between the route
  boundary and the real stream lifecycle owner. That file no longer owned any
  independent behavior. It only replayed:
  - `prepare runtime launch input from request`
  - `spawn stream lifecycle from prepared runtime input`

  those semantics already belong to the neighboring single-generation owners:
  - request/prepare boundary:
    `route payload -> request -> runtime launch input`
  - stream lifecycle boundary:
    `runtime launch input -> stream lifecycle`

  this checkpoint tightens the boundary further:
  - `backend-rs/src/services/chapter_single_generation_stream_workflow_service.rs`
    now directly exposes the route-facing stream public entry:
    - `create_owned_single_generation_stream(...)`
    - `start_owned_single_generation_stream_lifecycle(...)`
  - `backend-rs/src/api/chapter_generation_routes.rs`
    now dispatches stream requests directly to the stream-workflow owner
  - `backend-rs/src/services/mod.rs`
    now drops the shell-only
    `chapter_single_generation_stream_entry_service` module registration
  - deleted shell file:
    - `backend-rs/src/services/chapter_single_generation_stream_entry_service.rs`

  This is a real Phase 5 migration step because Rust now owns one tighter
  route stream-entry -> runtime-prepare -> stream-lifecycle chain rather than
  preserving a separate public-entry shell that adds no validation, no branch
  semantics, and no independent error contract:
  - `route payload -> stream workflow owner -> lifecycle owner`

  Focused validation passed with:
  `cargo test chapter_generation_routes --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-stream-entry-collapse" -- --nocapture`
  `cargo test chapter_single_generation_stream_workflow_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-stream-entry-collapse" -- --nocapture`
  `cargo test chapter_single_generation --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-stream-entry-collapse" -- --nocapture`
  `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-stream-entry-collapse"`

- 2026-06-06 single-generation manual-review file-collapse checkpoint:
  this slice stayed on the same `chapter_single_generation` Phase 5 module
  package and closed the next real owner gap after the route-facing stream
  entry shell had already been removed. Before this change, the
  single-generation runtime outcome lane already owned:
  - follow-up analysis execution
  - quality-blocked/manual-review snapshot persistence
  - final failed/completed task-stage persistence

  but `backend-rs/src/services/chapter_single_generation_manual_review_service.rs`
  still remained as one extra file-local helper owner beside the runtime
  outcome chain. That file no longer owned any independent behavior. It only
  replayed:
  - `quality payload -> manual review decision label`
  - fallback label resolution for the runtime outcome lane

  those semantics already belong to the neighboring runtime outcome owner:
  - `analysis payload -> manual-review label -> outcome persistence`

  this checkpoint tightens the boundary further:
  - `backend-rs/src/services/chapter_single_generation_runtime_outcome_service.rs`
    now directly owns:
    - `manual_review_label_from_single_generation_quality_context(...)`
    - payload/manual-review label fallback resolution
    - follow-up analysis label projection beside outcome persistence
  - `backend-rs/src/services/mod.rs`
    now drops the shell-only
    `chapter_single_generation_manual_review_service` module registration
  - deleted shell file:
    - `backend-rs/src/services/chapter_single_generation_manual_review_service.rs`

  This is a real Phase 5 migration step because Rust now owns one tighter
  runtime outcome -> manual-review label -> failure/quality-blocked outcome
  chain rather than preserving a separate file-local label helper that adds no
  validation, no branch semantics, and no independent error contract:
  - `analysis payload -> runtime outcome owner -> manual-review persistence`

  Focused validation passed with:
  `cargo test chapter_single_generation_runtime_outcome_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-manual-review-collapse" -- --nocapture`
  `cargo test chapter_single_generation --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-manual-review-collapse" -- --nocapture`
  `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-manual-review-collapse"`

- 2026-06-07 single-generation runtime-seed file-collapse checkpoint:
  this slice stayed on the same `chapter_single_generation` Phase 5 module
  package and continued the restored-runtime / background-write tightening
  path after the startup-snapshot owner had already been collapsed back into
  the restore chain. Before this change, the module already had three real
  production owners:
  - request / target / prepare owner
  - restored runtime / startup snapshot owner
  - background-write entry / stream workflow consumers

  but `backend-rs/src/services/chapter_single_generation_runtime_seed_service.rs`
  still remained as one extra file-local forwarding shell between those real
  owners. That file no longer owned any independent behavior. It only replayed:
  - request runtime-state -> execution-config materialization
  - restored runtime -> startup snapshot + runtime launch materialization
  - restored launch -> background launch parts / response payload projection

  those semantics now belong directly to the neighboring prepare owner because
  the same boundary already owns:
  - request validation
  - chapter target ownership
  - restored-runtime launch preparation
  - background launch materialization consumed by stream/background lanes

  this checkpoint tightens the boundary further:
  - `backend-rs/src/services/chapter_single_generation_prepare_service.rs`
    now directly owns:
    - `prepare_single_chapter_generation_execution_config_from_runtime_state(...)`
    - `build_single_generation_runtime_launch_input_from_request_runtime_state(...)`
    - `prepare_single_chapter_runtime_launch_input_from_request_runtime_state(...)`
    - `PreparedSingleChapterGenerationRestoredRuntimeLaunch`
    - `PreparedSingleGenerationBackgroundLaunchParts`
    - `prepare_single_generation_runtime_launch_input(...)`
    - `prepare_single_generation_background_launch_parts_from_target(...)`
    - `build_single_generation_background_create_response_payload(...)`
  - `backend-rs/src/services/chapter_single_generation_stream_workflow_service.rs`
    now consumes the prepare owner directly for runtime launch input
    materialization
  - `backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs`
    now consumes the prepare owner directly for restored single runtime launch
    input materialization
  - `backend-rs/src/services/chapter_single_generation_background_write_entry_service.rs`
    now consumes the prepare owner directly for restored launch /
    background-launch parts materialization
  - `backend-rs/src/services/mod.rs`
    now drops the shell-only
    `chapter_single_generation_runtime_seed_service` module registration
  - deleted shell file:
    - `backend-rs/src/services/chapter_single_generation_runtime_seed_service.rs`

  This is a real Phase 5 migration step because Rust now owns one tighter
  single-generation request/runtime-state -> restored launch -> stream or
  background launch chain rather than preserving a separate runtime-seed file
  that adds no validation, no semantic branching, and no independent error
  contract:
  - stream:
    `request -> prepare owner -> runtime launch input -> stream workflow owner`
  - background:
    `request/target -> prepare owner -> background launch parts -> background-write owner`
  - resume/runtime-state:
    `restored single request runtime-state -> prepare owner -> single runtime launch input`

  Focused validation passed with:
  `cargo test chapter_single_generation_prepare_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-runtime-seed-collapse" -- --nocapture`
  `cargo test chapter_single_generation_background_write_entry_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-runtime-seed-collapse" -- --nocapture`
  `cargo test chapter_single_generation_stream_workflow_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-runtime-seed-collapse" -- --nocapture`
  `cargo test chapter_batch_generation_runtime_state_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-runtime-seed-collapse" -- --nocapture`
  `cargo test chapter_single_generation --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-runtime-seed-collapse" -- --nocapture`
  `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-runtime-seed-collapse"`

- 2026-06-07 single-generation task-seed file-collapse checkpoint:
  this slice stayed on the same `chapter_single_generation` Phase 5 module
  package and continued the prepare / background-write tightening path after
  the runtime-seed owner had already been collapsed back into the prepare
  chain. Before this change, the module already had one real single-chapter
  background create owner:
  - prepare owner:
    `chapter target -> task persistence seed / launch parts`

  but `backend-rs/src/services/chapter_single_generation_task_seed_service.rs`
  still remained as one extra file-local shell beside that prepare owner.
  That file no longer owned any independent behavior. It only replayed:
  - `SingleGenerationTaskPersistenceSeed`
  - `task seed -> active model`
  - single background task default field projection

  those semantics now belong directly to the prepare owner because the same
  boundary already owns chapter-scoped target metadata, response payload
  projection, and background launch materialization.

  this checkpoint tightens the boundary further:
  - `backend-rs/src/services/chapter_single_generation_prepare_service.rs`
    now directly owns:
    - `SingleGenerationTaskPersistenceSeed`
    - `build_single_generation_background_task_persistence_seed(...)`
    - `build_single_generation_background_task_active_model(...)`
    - target-scoped `background_task_persistence_seed(...)`
    - target-scoped `background_task_active_model(...)`
    - the task-seed / active-model contract assertions that used to live in
      the deleted shell file
  - `backend-rs/src/services/mod.rs`
    now drops the shell-only
    `chapter_single_generation_task_seed_service` module registration
  - deleted shell file:
    - `backend-rs/src/services/chapter_single_generation_task_seed_service.rs`

  This is a real Phase 5 migration step because Rust now owns one tighter
  single-generation target -> task-seed -> background-write persistence chain
  rather than preserving a separate task-seed file that adds no validation, no
  semantic branching, and no independent error contract:
  - `chapter target -> prepare owner -> task seed / active model -> background-write owner`

  Focused validation passed with:
  `cargo test chapter_single_generation_prepare_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-task-seed-collapse" -- --nocapture`
  `cargo test chapter_single_generation_background_write_entry_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-task-seed-collapse" -- --nocapture`
  `cargo test chapter_single_generation --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-task-seed-collapse" -- --nocapture`
  `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-task-seed-collapse"`

- 2026-06-07 single-generation task-stage file-collapse checkpoint:
  this slice stayed on the same `chapter_single_generation` Phase 5 module
  package and continued the runtime lifecycle tightening path after the
  runtime-checkpoint owner had already been collapsed back into the runtime
  chain. Before this change, the module already had one real runtime lifecycle
  owner:
  - runtime-state owner:
    `runtime launch -> task-stage persistence -> checkpoint persistence`

  but `backend-rs/src/services/chapter_single_generation_task_stage_service.rs`
  still remained as one extra file-local shell beside that runtime owner.
  That file no longer owned any independent behavior. It only replayed:
  - `SingleGenerationTaskStage`
  - `TaskTimestampUpdate`
  - `ModelFieldUpdate`
  - task active-model mutation and persistence

  those semantics now belong directly to the runtime owner because the same
  boundary already owns runtime launch orchestration, checkpoint stage
  projection, preparation persistence, and runtime outcome handoff.

  this checkpoint tightens the boundary further:
  - `backend-rs/src/services/chapter_single_generation_runtime_state_service.rs`
    now directly owns:
    - `SingleGenerationTaskStage`
    - `TaskTimestampUpdate`
    - `ModelFieldUpdate`
    - task active-model mutation helpers
    - `persist_for_task(...)`
    - `persist_runtime_preparation(...)`
    - `persist_with_checkpoint(...)`
    - the task-stage mutation contract tests that used to live in the deleted
      shell file
  - `backend-rs/src/services/chapter_single_generation_runtime_outcome_service.rs`
    now consumes `SingleGenerationTaskStage` directly from the runtime-state
    owner
  - `backend-rs/src/services/mod.rs`
    now drops the shell-only
    `chapter_single_generation_task_stage_service` module registration
  - deleted shell file:
    - `backend-rs/src/services/chapter_single_generation_task_stage_service.rs`

  This is a real Phase 5 migration step because Rust now owns one tighter
  single-generation runtime launch -> task-stage mutation -> checkpoint /
  outcome persistence chain rather than preserving a separate task-stage file
  that adds no validation, no semantic branching, and no independent error
  contract:
  - `runtime launch -> runtime-state owner -> task stage / checkpoint / outcome`

  Focused validation passed with:
  `cargo test chapter_single_generation_runtime_state_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-task-stage-collapse" -- --nocapture`
  `cargo test chapter_single_generation_runtime_outcome_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-task-stage-collapse" -- --nocapture`
  `cargo test chapter_single_generation --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-task-stage-collapse" -- --nocapture`
  `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-generation-task-stage-collapse"`

- 2026-06-07 chapter-generation shared quality-gate semantics owner-lift
  checkpoint:
  this slice switched back to Package A, `chapter_generation`, and moved one
  genuinely shared lower-level quality-gate owner out of the batch-named
  quality-status file. Before this change, batch and single/story-repair
  flows already depended on the same lower-level label / decision parsing
  semantics, but those helpers still lived inside:
  - `backend-rs/src/services/chapter_batch_generation_quality_status_service.rs`

  that batch file no longer owned those shared lower-level semantics by
  itself. It only happened to host:
  - latest failed-chapter manual-review label resolution
  - latest failed-chapter retryable-repair label resolution
  - quality-context manual-review fallback resolution
  - exhausted auto-repair -> manual-review fallback resolution
  - retry-budget-aware retryable-repair label resolution

  this checkpoint tightens the shared owner boundary further:
  - added new shared owner file:
    - `backend-rs/src/services/chapter_generation_quality_gate_semantics_service.rs`
  - that new chapter-generation-scoped owner now directly owns:
    - `manual_review_label(...)`
    - `retryable_repair_label(...)`
    - `manual_review_label_from_quality_context(...)`
    - `manual_review_label_from_quality_context_with_retry_budget(...)`
    - `retryable_repair_label_from_quality_context_with_retry_budget(...)`
  - `backend-rs/src/services/chapter_story_repair_quality_context_service.rs`
    now consumes the shared quality-gate semantics owner directly when it
    reconciles quality-gate payloads across current and recent-history sources
  - `backend-rs/src/services/chapter_batch_generation_resume_task_command_service.rs`
    now consumes the shared owner directly for resume blockers and retry-budget
    gating instead of reaching back into the batch quality-status file for the
    same lower-level semantics
  - `backend-rs/src/services/chapter_batch_generation_quality_status_service.rs`
    now keeps only batch-specific owners:
    - `BatchGenerationQualityStatusContext`
    - `BatchGenerationFailedTerminalKind`
    - `BatchGenerationFailedTerminalSemantics`
    - `resolve_failed_terminal_semantics_from_sources(...)`
    - `insert_batch_generation_terminal_status_payload(...)`

  This is a real Phase 5 migration step because Rust now owns one explicit
  shared chapter-generation quality-gate semantics boundary instead of leaving
  batch and single/story-repair lanes attached to a batch-named lower-level
  parsing file by accident:
  - shared lower-level quality-gate semantics:
    `failed chapter or quality context -> manual review | retry label`
  - batch-specific terminal semantics remain:
    `batch failed task -> terminal reason / label / resume semantics`

  The remaining Python dependency is unchanged in this slice: route payloads,
  fallback shells, task lifecycle, checkpoint shape, SSE payloads, provider
  defaults, and batch terminal status shell remain stable. The rollback
  boundary remains the existing gateway/Python fallback shell because no route
  ownership or transport cutover changed.

  Focused validation passed with:
  `cargo test chapter_generation_quality_gate_semantics_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/shared-quality-gate-owner-lift" -- --nocapture`
  `cargo test chapter_batch_generation_quality_status_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/shared-quality-gate-owner-lift" -- --nocapture`
  `cargo test chapter_story_repair_quality_context_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/shared-quality-gate-owner-lift" -- --nocapture`
  `cargo test chapter_batch_generation_resume_task_command_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/shared-quality-gate-owner-lift" -- --nocapture`
  `cargo test chapter_batch_generation --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/shared-quality-gate-owner-lift" -- --nocapture`
  `cargo test chapter_single_generation --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/shared-quality-gate-owner-lift" -- --nocapture`
  `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/shared-quality-gate-owner-lift"`

- 2026-06-07 chapter-generation shared quality runtime-context algorithm
  owner-lift checkpoint:
  this slice stayed on Package A, `chapter_generation`, and continued the
  shared owner-lift path for quality runtime-context semantics. Before this
  change, batch and single-generation lanes already shared the same lower-level
  history append and summary-state rebuild mechanics, but the batch file still
  carried its own parallel implementation for several of those algorithms:
  - `backend-rs/src/services/chapter_batch_generation_quality_runtime_context_service.rs`

  that batch file still locally owned duplicated lower-level algorithm groups:
  - append bounded quality-metrics history
  - rebuild history from summary recent-metrics payloads
  - rebuild quality summary from `summary_state + history`
  - merge fallback history-context payload fields

  this checkpoint tightens the shared algorithm owner boundary further:
  - `backend-rs/src/services/chapter_generation_quality_runtime_context_service.rs`
    now directly exposes shared lower-level helpers for:
    - `append_generation_quality_metrics_history_event(...)`
    - `build_generation_quality_metrics_history_from_summary(...)`
    - `build_generation_quality_summary_from_state_or_history(...)`
    - `merge_generation_quality_history_context_with_recent_metric_fallback(...)`
  - `backend-rs/src/services/chapter_batch_generation_quality_runtime_context_service.rs`
    now reuses those chapter-generation-scoped shared helpers for the batch
    runtime-context algorithm path instead of keeping a second full copy of
    the same lower-level mechanics
  - batch-specific wrapper responsibilities remain in the batch file:
    - batch scope selection (`"batch"`)
    - persisted-source extraction from snapshot/runtime-state
    - startup-seed/current-quality/preserved-state batch owner entrypoints
    - `BatchGenerationQualityRuntimeContext` response shape

  This is a real Phase 5 migration step because Rust now owns one explicit
  shared quality runtime-context algorithm boundary instead of leaving batch
  and single-generation lanes to drift with parallel lower-level
  implementations:
  - shared lower-level algorithm owner:
    `summary/history/state -> runtime quality context mechanics`
  - batch wrapper owner remains:
    `batch persisted/runtime sources -> batch entrypoint contract`

  The remaining Python dependency is unchanged in this slice: route payloads,
  fallback shells, task lifecycle, checkpoint shape, SSE payloads, and startup
  seed contracts remain stable. The rollback boundary remains the existing
  gateway/Python fallback shell because no route ownership or transport cutover
  changed.

  Focused validation passed with:
  `cargo test chapter_batch_generation_quality_runtime_context_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/shared-quality-runtime-context-owner-lift" -- --nocapture`
  `cargo test chapter_batch_generation --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/shared-quality-runtime-context-owner-lift" -- --nocapture`
  `cargo test chapter_single_generation --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/shared-quality-runtime-context-owner-lift" -- --nocapture`
  `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/shared-quality-runtime-context-owner-lift"`

- 2026-06-07 chapter-generation shared task semantics owner-lift checkpoint:
  this slice stayed on Package A, `chapter_generation`, and continued the
  whole-function-group owner-lift path for lower-level task semantics shared
  by batch read/runtime/resume/write lanes. Before this change, those shared
  task semantics still lived in a batch-named payload owner file:
  - `backend-rs/src/services/chapter_batch_generation_task_payload_base_service.rs`

  that file no longer owned those semantics exclusively. The same lower-level
  contract was already consumed across multiple batch subflows and had become
  a chapter-generation-scoped shared decision boundary:
  - active task statuses
  - single-chapter vs batch task kind classification
  - task type projection

  this checkpoint tightens that shared owner boundary further:
  - added new shared owner file:
    - `backend-rs/src/services/chapter_generation_task_semantics_service.rs`
  - that new shared owner now directly owns:
    - `active_batch_generation_statuses()`
    - `BatchGenerationTaskKind`
    - `batch_generation_task_kind(...)`
    - `task_kind(...)`
    - `batch_generation_task_type(...)`
    - `task_type(...)`
    - focused regression tests for active-status and task-kind/task-type
      contracts
  - `backend-rs/src/services/chapter_batch_generation_read_context_service.rs`
    now consumes the shared active-status owner directly for active-task query
    filtering and status classification
  - `backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs`
    now consumes the shared task-kind/task-type owner directly for resume
    semantics, runtime launch labeling, and related tests
  - `backend-rs/src/services/chapter_batch_generation_resume_task_command_service.rs`
    now consumes the shared task-kind type directly in runtime/tests instead of
    reaching through the batch payload base file
  - `backend-rs/src/services/chapter_batch_generation_write_workflow_service.rs`
    now consumes the shared task-type projection directly for create/resume
    write-lane labeling
  - `backend-rs/src/services/chapter_batch_generation_task_payload_base_service.rs`
    now keeps batch payload / quality-status / checkpoint metadata ownership,
    but no longer hosts the shared task semantics group

  This is a real Phase 5 migration step because Rust now owns one explicit
  chapter-generation-scoped shared task semantics boundary instead of leaving
  batch read/runtime/resume/write lanes attached to a payload-base file by
  accident:
  - shared lower-level task semantics owner:
    `task row/chapter ids -> active statuses | task kind | task type`
  - batch payload/value-contract owner remains:
    `task row/runtime state -> payload/checkpoint/terminal status projection`

  The remaining Python dependency is unchanged in this slice: route payloads,
  SSE payloads, task lifecycle semantics, fallback shells, and gateway
  rollback boundaries remain stable. No route ownership or transport cutover
  changed in this step.

  Focused validation passed with:
  `cargo test chapter_generation_task_semantics_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/shared-task-semantics-owner-lift" -- --nocapture`
  `cargo test chapter_batch_generation_task_payload_base_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/shared-task-semantics-owner-lift" -- --nocapture`
  `cargo test chapter_batch_generation_read_context_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/shared-task-semantics-owner-lift" -- --nocapture`
  `cargo test chapter_batch_generation_runtime_state_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/shared-task-semantics-owner-lift" -- --nocapture`
  `cargo test chapter_batch_generation_resume_task_command_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/shared-task-semantics-owner-lift" -- --nocapture`
  `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/shared-task-semantics-owner-lift"`

- 2026-06-07 batch-generation quality-status facade file-collapse checkpoint:
  this slice stayed on Package C, `chapter_batch_generation`, and collapsed
  one remaining batch quality-status facade file back into the surviving
  shared batch task-payload base owner. Before this change, the batch
  read/status/stream/runtime/resume lanes already depended on one coherent
  batch payload/read/value-contract chain, but terminal quality-status
  semantics still sat behind a dedicated neighboring file:
  - `backend-rs/src/services/chapter_batch_generation_quality_status_service.rs`

  that file no longer owned an independent route boundary, fallback shell,
  rollback seam, persistence transport, or batch-only branch. It only replayed:
  - `BatchGenerationQualityStatusContext`
  - `BatchGenerationFailedTerminalKind`
  - `BatchGenerationFailedTerminalSemantics`
  - `insert_batch_generation_terminal_status_payload(...)`
  - `resolve_failed_terminal_semantics(...)`
  - `resolve_failed_terminal_semantics_from_sources(...)`

  this checkpoint tightens the batch owner boundary further:
  - surviving shared batch payload/value-contract owner:
    - `backend-rs/src/services/chapter_batch_generation_task_payload_base_service.rs`
  - that surviving owner now directly owns:
    - `BatchGenerationQualityStatusContext`
    - `BatchGenerationFailedTerminalKind`
    - `BatchGenerationFailedTerminalSemantics`
    - `insert_batch_generation_terminal_status_payload(...)`
    - `resolve_failed_terminal_semantics(...)`
    - `resolve_failed_terminal_semantics_from_sources(...)`
    - focused quality-status regression tests beside the surviving payload
      owner
  - `backend-rs/src/services/chapter_batch_generation_read_context_service.rs`
    keeps the final
    `build_batch_generation_status_task_payload_with_quality_context(...)`
    status/read payload projection owner, so the runtime lane no longer needs
    a deleted quality-status facade to rebuild final status payloads
  - `backend-rs/src/services/chapter_batch_generation_status_stream_service.rs`
    now consumes failed-terminal semantics directly from the surviving
    payload/value-contract owner
  - `backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs`
    now imports:
    - failed-terminal semantics from the surviving payload/value-contract owner
    - final status payload projection from the surviving read-context owner
  - `backend-rs/src/services/chapter_batch_generation_resume_task_command_service.rs`
    keeps its test/runtime quality-status context imports on the surviving
    payload/value-contract owner instead of a deleted facade file
  - `backend-rs/src/services/mod.rs`
    now drops the shell-only batch quality-status module registration
  - deleted shell file:
    - `backend-rs/src/services/chapter_batch_generation_quality_status_service.rs`

  This is a real Phase 5 migration step because Rust now owns one tighter
  batch payload/read/status chain instead of preserving a separate
  quality-status facade that adds no new validation layer, no route boundary,
  and no independent rollback seam:
  - `shared batch payload owner -> terminal quality-status semantics -> read/status/stream/runtime consumers`

  The remaining Python dependency is unchanged in this slice: HTTP payloads,
  SSE payloads, task lifecycle semantics, checkpoint shapes, provider
  defaults, fallback shells, and gateway rollback boundaries remain stable.
  No route ownership or transport cutover changed in this step.

  Focused validation passed with:
  `cargo fmt --manifest-path "backend-rs/Cargo.toml" -- "backend-rs/src/services/chapter_batch_generation_task_payload_base_service.rs" "backend-rs/src/services/chapter_batch_generation_read_context_service.rs" "backend-rs/src/services/chapter_batch_generation_status_stream_service.rs" "backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs" "backend-rs/src/services/chapter_batch_generation_resume_task_command_service.rs" "backend-rs/src/services/mod.rs"`
  `cargo test chapter_batch_generation_task_payload_base_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-quality-status-file-collapse-payload" -- --nocapture`
  `cargo test chapter_batch_generation_read_context_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-quality-status-file-collapse-read" -- --nocapture`
  `cargo test chapter_batch_generation_status_stream_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-quality-status-file-collapse-stream" -- --nocapture`
  `cargo test chapter_batch_generation_runtime_state_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-quality-status-file-collapse-runtime" -- --nocapture`
  `cargo test chapter_batch_generation --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-quality-status-file-collapse" -- --nocapture`
  `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-quality-status-file-collapse-check"`

- 2026-06-07 chapter-generation shared execution-contract owner-lift
  checkpoint:
  this slice stayed on Package A, `chapter_generation`, and continued the
  whole-function-group owner-lift path for execution contract semantics that
  had already escaped the single-generation prepare/runtime owners. Before this
  change, the same lower-level compat and prompt-override contract was already
  reused across:
  - single-generation prepare / runtime / stream / write lanes
  - batch runtime / resume / write lanes
  - shared request-runtime / research / story-repair / regeneration helpers

  but the shared contract still lived across two single-generation files:
  - `backend-rs/src/services/chapter_single_generation_prepare_service.rs`
  - `backend-rs/src/services/chapter_single_generation_runtime_state_service.rs`

  those files no longer owned that contract exclusively. The same lower-level
  execution boundary had become a chapter-generation-scoped shared owner:
  - single-generation compat options
  - single-generation execution input
  - compat-options -> prompt-overrides projection

  this checkpoint tightens that shared owner boundary further:
  - added new shared owner file:
    - `backend-rs/src/services/chapter_generation_execution_contract_service.rs`
  - that new shared owner now directly owns:
    - `SingleChapterGenerationCompatOptions`
    - `SingleChapterGenerationExecutionInput`
    - `build_prompt_overrides_from_compat_options(...)`
    - focused regression tests for empty-string trimming and web-research
      prompt-override projection
  - `backend-rs/src/services/chapter_single_generation_prepare_service.rs`
    now keeps request normalization, target loading, restore/launch planning,
    and task persistence preparation, but no longer defines the shared
    execution contract itself
  - `backend-rs/src/services/chapter_single_generation_runtime_state_service.rs`
    now keeps lifecycle/runtime persistence ownership, but no longer owns the
    shared compat-options -> prompt-overrides builder
  - the following consumers now depend on the shared execution-contract owner
    directly for their real cross-module contract:
    - `backend-rs/src/services/chapter_generation_request_runtime_state_service.rs`
    - `backend-rs/src/services/chapter_generation_research_payload_service.rs`
    - `backend-rs/src/services/chapter_story_repair_quality_context_service.rs`
    - `backend-rs/src/services/chapter_regeneration_prepare_service.rs`
    - `backend-rs/src/services/chapter_single_generation_stream_workflow_service.rs`
    - `backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs`
    - `backend-rs/src/services/chapter_batch_generation_resume_task_command_service.rs`
    - `backend-rs/src/services/chapter_batch_generation_write_workflow_service.rs`
  - single-generation prepare/runtime files keep thin compatibility re-exports
    for neighboring older imports, but the real owner is now chapter-generation
    scoped

  This is a real Phase 5 migration step because Rust now owns one explicit
  chapter-generation-scoped shared execution contract boundary instead of
  leaving batch/runtime/stream/research neighbors attached to
  single-generation-named files by accident:
  - shared lower-level execution contract owner:
    `compat options / execution input / prompt overrides`
  - single-generation prepare/runtime owners remain:
    `request normalization / target loading / lifecycle persistence`

  The remaining Python dependency is unchanged in this slice: route payloads,
  SSE payloads, task lifecycle semantics, checkpoint shapes, fallback shells,
  and gateway rollback boundaries remain stable. No route ownership or
  transport cutover changed in this step.

  Focused validation passed with:
  `cargo test chapter_generation_execution_contract_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/shared-single-generation-compat-owner-lift" -- --nocapture`
  `cargo test chapter_generation_request_runtime_state_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/shared-single-generation-compat-owner-lift" -- --nocapture`
  `cargo test chapter_single_generation_runtime_state_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/shared-single-generation-compat-owner-lift" -- --nocapture`
  `cargo test chapter_batch_generation_runtime_state_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/shared-single-generation-compat-owner-lift" -- --nocapture`
  `cargo test chapter_batch_generation_resume_task_command_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/shared-single-generation-compat-owner-lift" -- --nocapture`
  `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/shared-single-generation-compat-owner-lift"`

- 2026-06-07 project-quality trend compat owner retirement checkpoint:
  this Python fallback cleanup stayed on Package D, `chapters` compatibility
  shell cleanup, and retired one whole compat owner file instead of preserving
  another micro seam. Before this change, project quality trend production
  routing still depended on:
  - `backend/app/services/compat/project_quality_trend_compat_service.py`
  - `backend/tests/test_services/test_project_quality_trend_compat_service.py`

  That compat file no longer owned an independent fallback boundary, route
  transport, rollback seam, or behavior branch. It only replayed quality-trend
  snapshot calls into the real project-quality trend service while carrying
  default route wiring for:
  - summary state build / advance / projection
  - trend snapshot load / persist

  The default wiring has now been moved to the stable real service import
  surface:
  - `backend/app/services/project_quality_trend_service.py`

  API call sites now consume the real owner directly:
  - `backend/app/api/chapter_quality_routes.py`
  - `backend/app/api/chapters.py`

  Focused tests were migrated to the real owner:
  - added `backend/tests/test_services/test_project_quality_trend_service.py`
  - updated `backend/tests/test_api/test_chapters_quality_views.py` to patch
    `app.services.project_quality_trend_service`
  - deleted the old compat service test

  This is counted as real migration progress because the active code path no
  longer depends on a Python compat owner for project-quality trend snapshot
  resolution. The remaining behavior contract is unchanged: HTTP response
  shell, quality metrics summary semantics, in-memory snapshot cache,
  persisted snapshot load/persist, and project access checks remain stable.

  Focused validation passed with:
  `rg -n "project_quality_trend_compat_service|_get_project_quality_trend_snapshot_compat_service" backend/app backend/tests`
  `python -c "import sys; sys.path.insert(0, 'backend'); from app.api import chapters, chapter_quality_routes; from app.services import project_quality_trend_service; print('ok')"`
  `python -m pytest backend/tests/test_services/test_project_quality_trend_service.py -q`
  `python -m pytest backend/tests/test_api/test_chapters_quality_views.py -q`

- 2026-06-07 batch-generation entry compat owner retirement checkpoint:
  this Python fallback cleanup stayed on Package D, `chapters` compatibility
  shell cleanup, and retired the whole batch-generation entry compat owner
  after the real wiring services were already stable. Before this change,
  batch create/resume and the `chapters.py` legacy wrapper still depended on:
  - `backend/app/services/compat/batch_generation_entry_compat_service.py`
  - `backend/tests/test_services/test_batch_generation_entry_compat_service.py`

  That compat file no longer owned an independent fallback boundary, route
  transport, rollback seam, or behavior branch. It only replayed two public
  entry calls into real wiring owners:
  - `execute_batch_generation_in_order(...)`
    -> `batch_generation_run_wiring_service.execute_batch_generation_in_order_with_default_wiring(...)`
  - `generate_single_chapter_for_batch(...)`
    -> `batch_generation_single_chapter_wiring_service.generate_single_chapter_for_batch_with_default_wiring(...)`

  The API / route surfaces now consume real owners directly:
  - `backend/app/api/chapters.py`
  - `backend/app/services/compat/batch_generation_route_compat_service.py`
  - `backend/app/services/compat/chapter_generation_route_compat_service.py`

  Test patch surfaces were migrated off the deleted compat module:
  - `backend/tests/test_api/chapters_test_support.py`
  - `backend/tests/test_api/test_chapters_batch_status_resume.py`

  Focused owner tests already existed and now replace the deleted compat
  pass-through test:
  - `backend/tests/test_services/test_batch_generation_run_wiring_service.py`
  - `backend/tests/test_services/test_batch_generation_single_chapter_wiring_service.py`

  This is counted as real migration progress because the active batch
  execution path no longer depends on a Python entry compat owner. The
  remaining behavior contract is unchanged: HTTP response shell, batch task
  lifecycle, story-repair state propagation, single-chapter generation
  candidate wiring, stream heartbeat defaults, and API monkeypatch behavior
  remain stable.

  Focused validation passed with:
  `rg -n --glob "*.py" "batch_generation_entry_compat_service|_execute_batch_generation_in_order_compat_service|_generate_single_chapter_for_batch_compat_service" backend/app backend/tests`
  `python -c "import sys; sys.path.insert(0, 'backend'); from app.api import chapters, chapter_batch_generation_routes; from app.services.compat import batch_generation_route_compat_service, chapter_generation_route_compat_service; from app.services import batch_generation_run_wiring_service, batch_generation_single_chapter_wiring_service; print('ok')"`
  `python -m pytest backend/tests/test_services/test_batch_generation_run_wiring_service.py backend/tests/test_services/test_batch_generation_single_chapter_wiring_service.py -q`
  `python -m pytest backend/tests/test_api/test_chapters_batch_status_resume.py backend/tests/test_api/test_chapters_batch_generation.py backend/tests/test_api/test_chapters_stream_routes.py -q`

- 2026-06-07 batch-generation route compat owner retirement checkpoint:
  this Python fallback cleanup stayed on Package D, `chapters` compatibility
  shell cleanup, and retired the whole batch-generation route compat owner
  after the route module had become its only real production consumer. Before
  this change, batch route default wiring still depended on:
  - `backend/app/services/compat/batch_generation_route_compat_service.py`

  That compat file no longer owned an independent fallback boundary, route
  transport shape, rollback seam, or business branch. It only replayed three
  route-facing helper calls for:
  - create default wiring
  - resume default wiring
  - stream access + SSE wiring

  The surviving route owner now keeps those helpers directly:
  - `backend/app/api/chapter_batch_generation_routes.py`

  Test patch surfaces were moved to the route module:
  - `backend/tests/test_api/test_chapters_stream_routes.py`

  This is counted as real migration progress because the active batch route
  path no longer depends on a Python route compat owner. The remaining
  behavior contract is unchanged: batch create/resume response shells, access
  checks, stream access validation, SSE event streaming, prerequisite checks,
  story-packet assembly, and execution callable patch behavior remain stable.

  Focused validation passed with:
  `rg -n --glob "*.py" "batch_generation_route_compat_service" backend/app backend/tests`
  `python -m pytest backend/tests/test_api/test_chapters_batch_generation.py backend/tests/test_api/test_chapters_batch_status_resume.py backend/tests/test_api/test_chapters_stream_routes.py -q`

### 2026-06-08 chapter-candidate-executor Rust staged owner checkpoint

This round continued the Rust-first, whole-function-group migration plan and
ported the candidate executor orchestration boundary from:

- `backend/app/services/chapter_candidate_executor_service.py`
- `backend/app/services/chapter_candidate_executor_wiring_service.py`
  remains the production wiring source map for the next cutover package
- `backend/app/services/chapter_candidate_rerank_service.py`
  remains the formula source map for injected rerank decisions

New Rust staged owner:

- `backend-rs/src/services/chapter_candidate_executor_service.rs`
- `backend-rs/src/services/mod.rs`

This owner composes the existing staged Rust candidate package instead of
adding another isolated helper:

- `chapter_candidate_output_service.rs`
- `chapter_candidate_generation_service.rs`
- `chapter_candidate_record_service.rs`
- `chapter_candidate_word_budget_repair_service.rs`
- `chapter_candidate_targeted_final_repair_service.rs`
- `chapter_candidate_finalize_service.rs`
- `chapter_candidate_runtime_state_service.rs`

Behavior migrated into the Rust executor owner:

- generation stage
- word-budget repair stage
- optional pre-finalize targeted final repair
- finalize-state resolution with word-budget repair promotion enabled
- post-finalize targeted repair seed selection
- optional post-finalize targeted final repair
- optional follow-up targeted final repair after another finalize-state
  resolution
- final finalize result projection
- runtime-state handoff through every stage request
- base prompt / temperature resolution and stable propagation

The staged boundary is intentional: rerank-heavy choices still remain
injectable callbacks, and Python `chapter_candidate_executor_service.py` is
not yet retired from the active production path. This round therefore counts
as Rust owner readiness progress, not production cutover completion.

Focused validation passed with:

- `cargo fmt --manifest-path "backend-rs/Cargo.toml"`
- `cargo test chapter_candidate_executor_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/mumunovel-candidate-executor-owner" -- --nocapture`
  -> 3 passed
- `cargo test chapter_candidate_generation_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/mumunovel-candidate-executor-owner" -- --nocapture`
  -> 5 passed
- `cargo test chapter_candidate_record_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/mumunovel-candidate-executor-owner" -- --nocapture`
  -> 3 passed
- `cargo test chapter_candidate_word_budget_repair_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/mumunovel-candidate-executor-owner" -- --nocapture`
  -> 3 passed
- `cargo test chapter_candidate_targeted_final_repair_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/mumunovel-candidate-executor-owner" -- --nocapture`
  -> 3 passed
- `cargo test chapter_candidate_finalize_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/mumunovel-candidate-executor-owner" -- --nocapture`
  -> 3 passed
- `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/mumunovel-candidate-executor-owner"`
  -> passed with existing unused/dead-code warnings
- `git diff --check -- "backend-rs/src/services/chapter_candidate_executor_service.rs" "backend-rs/src/services/mod.rs"`
  -> passed

Next acceleration target:

1. build Rust default dependency wiring for the candidate executor, or
2. create the production cutover adapter that lets the active generation path
   consume this Rust executor while keeping Python fallback explicit.

Do not count the Python executor as retired until one of those follow-up
packages lands and is validated.

### 2026-06-08 chapter-candidate-executor-wiring Rust staged owner checkpoint

This round continued the same candidate executor package and ported the wiring
source map from Python into a Rust-owned, testable dependency graph contract.
The Python source map is:

- `backend/app/services/chapter_candidate_executor_wiring_service.py`
- `backend/app/services/chapter_candidate_executor_service.py`
- `backend/app/services/chapter_candidate_rerank_service.py`

New Rust staged owner:

- `backend-rs/src/services/chapter_candidate_executor_wiring_service.rs`
- `backend-rs/src/services/mod.rs`

The Rust wiring owner does not yet build executable production dependencies.
Instead, it owns the full dependency graph and cutover-readiness contract that
the Python wiring file currently hides:

- stages: generation, word-budget repair, targeted final repair, finalize,
  executor
- Rust-owned dependencies: output collection, candidate record build,
  runtime-state labels/sync, candidate generation/repair/finalize workflows,
  and executor orchestration
- external formula blockers: rerank-heavy functions that still live in
  `chapter_candidate_rerank_service.py`
- validation: required stage coverage, owner-file presence, and non-empty
  dependency lists
- readiness: Rust-owned dependency count, external formula dependency count,
  and explicit cutover blocker names

This is real Rust migration progress because the candidate package now has a
Rust-owned wiring contract and no longer relies on ad-hoc notes to explain why
the staged executor cannot cut over yet. It is still not production cutover:
Python `chapter_candidate_executor_wiring_service.py` remains active until a
Rust default dependency builder or production adapter consumes the plan and
replaces or explicitly bridges the formula blockers.

Focused validation passed with:

- `cargo fmt --manifest-path "backend-rs/Cargo.toml"`
- `cargo test chapter_candidate_executor_wiring_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/mumunovel-candidate-executor-wiring-owner" -- --nocapture`
  -> 5 passed
- `cargo test chapter_candidate_executor_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/mumunovel-candidate-executor-wiring-owner" -- --nocapture`
  -> 3 passed
- `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/mumunovel-candidate-executor-wiring-owner"`
  -> passed with existing unused/dead-code warnings
- `git diff --check -- "backend-rs/src/services/chapter_candidate_executor_wiring_service.rs" "backend-rs/src/services/mod.rs"`
  -> passed

Next acceleration target:

1. migrate a Rust rerank formula owner for the formula blockers listed by the
   wiring readiness contract, or
2. build the production adapter/default dependency builder that consumes the
   wiring plan and makes any remaining Python formula bridge explicit.

Do not count the Python wiring file as retired until the active generation
path consumes the Rust executor package.

### 2026-06-08 chapter-candidate-rerank Rust staged owner checkpoint

This round continued the same Rust-first candidate executor package and ported
the rerank-heavy formula source map from Python into a Rust-owned, tested
function group. The Python source map is:

- `backend/app/services/chapter_candidate_rerank_service.py`
- `backend/app/services/chapter_candidate_executor_wiring_service.py`
- `backend/app/services/chapter_candidate_executor_service.py`

New Rust staged owner:

- `backend-rs/src/services/chapter_candidate_rerank_service.rs`
- `backend-rs/src/services/chapter_candidate_executor_wiring_service.rs`
- `backend-rs/src/services/mod.rs`

The migrated function group now owns the formula names that previously blocked
candidate executor cutover readiness:

- candidate retry and ranking:
  `should_generate_additional_candidate`,
  `build_candidate_retry_prompt_suffix`,
  `build_candidate_retry_strategy_suffix`,
  `resolve_candidate_retry_temperature`,
  `select_best_generation_candidate`
- candidate selection/finalize metadata:
  `build_candidate_selection_metadata`,
  `attach_candidate_selection_metadata`,
  `normalize_candidate_quality_gate_plan`,
  `build_candidate_pool_summary`
- word-budget repair:
  `should_apply_word_budget_repair`,
  `build_word_budget_repair_suffix`,
  `should_relax_word_budget_repair_limits`,
  `resolve_word_budget_repair_temperature`,
  `resolve_word_budget_repair_max_tokens`,
  `resolve_word_budget_repair_char_limit`,
  `should_keep_word_budget_repair_candidate`,
  `should_prefer_word_budget_repair_candidate`
- targeted final repair:
  `should_apply_targeted_final_repair`,
  `should_apply_followup_targeted_final_repair`,
  `build_targeted_final_repair_suffix`,
  `resolve_targeted_final_repair_temperature`,
  `resolve_targeted_final_repair_max_tokens`,
  `resolve_targeted_final_repair_char_limit`,
  `should_keep_targeted_final_repair_candidate`,
  `should_adopt_targeted_final_repair_candidate`,
  `should_prefer_targeted_final_repair_candidate`,
  `select_targeted_final_repair_seed_candidate`

Behavior contract preserved / staged:

- target word bounds, severe word-budget pressure, and
  `allow_save -> auto_repair` quality-gate normalization now live in Rust
- best-candidate ranking preserves the Python priority order:
  gate priority, selection score, overall score, word-count fit, then lower
  candidate index
- word-budget keep/prefer formulas preserve target-window, quality-drop,
  failed-metric-count, substantial-improvement, and severe-over-budget gates
- targeted final repair formulas preserve manual-review, target-window,
  continuity-warning, score-floor, focus-area, follow-up, and seed-selection
  gates
- prompt suffix owners preserve the key repair labels, target-window
  instructions, and focus-specific instruction lines; they are intentionally
  tested by behavior/key lines rather than byte-for-byte suffix parity

Wiring readiness update:

- `chapter_candidate_executor_wiring_service.rs` now points rerank formula
  dependencies to
  `backend-rs/src/services/chapter_candidate_rerank_service.rs`
- default readiness now reports zero external formula dependencies and no
  formula cutover blockers
- this is still staged readiness work, not active-path retirement:
  `backend/app/services/chapter_candidate_rerank_service.py` remains active
  until Rust default executor dependencies or a production adapter consumes
  the Rust formula owner

Focused validation passed with:

- `cargo fmt --manifest-path "backend-rs/Cargo.toml"`
- `cargo test chapter_candidate_rerank_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/mumunovel-candidate-rerank-owner" -- --nocapture`
  -> 10 passed
- `cargo test chapter_candidate_executor_wiring_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/mumunovel-candidate-rerank-owner" -- --nocapture`
  -> 5 passed
- `cargo test chapter_candidate_executor_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/mumunovel-candidate-rerank-owner" -- --nocapture`
  -> 3 passed
- `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/mumunovel-candidate-rerank-owner"`
  -> passed with existing unused/dead-code warnings

Next acceleration target:

1. Build executable Rust default dependency wiring for the candidate executor
   so the staged executor can call the Rust rerank owner directly.
2. Or build the production adapter that lets the active generation path
   consume the Rust candidate executor package while preserving explicit
   rollback to the Python path.

Do not count the Python rerank, executor, or wiring files as retired until the
active generation path consumes the Rust executor package.

### 2026-06-08 chapter-candidate-executor-default-dependency Rust staged owner checkpoint

This round continued the same Rust-first candidate executor package and built
an executable Rust default dependency owner for the Python wiring source map.
The Python source map is:

- `backend/app/services/chapter_candidate_executor_wiring_service.py`
- `backend/app/services/chapter_candidate_executor_service.py`
- `backend/app/services/chapter_candidate_rerank_service.py`

New Rust staged owner:

- `backend-rs/src/services/chapter_candidate_executor_default_dependency_service.rs`
- `backend-rs/src/services/chapter_candidate_executor_wiring_service.rs`
- `backend-rs/src/services/mod.rs`

The new owner composes the existing staged Rust candidate package into one
callable default dependency flow:

- generation owner:
  `generate_candidate_pool_workflow`
- word-budget repair owner:
  `maybe_apply_word_budget_repair_workflow`
- targeted final repair owner:
  `execute_targeted_final_repair_pass_workflow`
- finalize owner:
  `resolve_final_candidate_state`,
  `maybe_promote_best_word_budget_repair_candidate`, and
  `finalize_selected_candidate_result`
- rerank owner:
  retry, word-budget, targeted-final-repair, selection metadata, pool summary,
  and best-candidate formulas from
  `chapter_candidate_rerank_service.rs`

Behavior contract preserved / staged:

- base prompt and temperature are resolved from the executor request like the
  staged executor owner
- runtime state is handed through generation, repair, targeted repair, and
  finalize request owners
- provider output collection and candidate record construction remain explicit
  injection boundaries
- quality gate plan construction remains explicit because the production
  quality adapter is still outside this staged package
- default dependency wiring is executable Rust progress, but not active-path
  retirement: Python candidate executor/wiring remain active until a production
  adapter or route path consumes this owner

Wiring readiness update:

- `chapter_candidate_executor_wiring_service.rs` now lists
  `backend-rs/src/services/chapter_candidate_executor_default_dependency_service.rs`
  in the Rust target map
- executor-stage dependencies include
  `generate_best_ranked_candidate_with_default_dependency_wiring`
- formula blockers remain zero because rerank dependencies point to the Rust
  rerank owner

Focused validation passed with:

- `cargo fmt --manifest-path "backend-rs/Cargo.toml"`
- `cargo test chapter_candidate_executor_default_dependency_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/mumunovel-candidate-default-wiring-owner" -- --nocapture`
  -> 2 passed
- `cargo test chapter_candidate_executor_wiring_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/mumunovel-candidate-default-wiring-owner" -- --nocapture`
  -> 5 passed
- `cargo test chapter_candidate_executor_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/mumunovel-candidate-default-wiring-owner" -- --nocapture`
  -> 3 passed
- `cargo test chapter_candidate_rerank_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/mumunovel-candidate-default-wiring-owner" -- --nocapture`
  -> 10 passed
- `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/mumunovel-candidate-default-wiring-owner"`
  -> passed with existing unused/dead-code warnings

Next acceleration target:

1. Build the production adapter that lets the active generation path consume
   `generate_best_ranked_candidate_with_default_dependency_wiring(...)` while
   preserving explicit rollback to Python.
2. Or, if route cutover is still too risky, migrate the provider output /
   record / quality-gate adapter boundaries as one whole module package so the
   remaining injections shrink before production consumption.

Do not count the Python candidate executor, rerank, or wiring files as retired
until the active generation path consumes the Rust executor package.

### 2026-06-08 chapter-candidate-executor-runtime-adapter Rust staged owner checkpoint

This round continued the same Rust-first candidate executor package and
shrunk the remaining Python-style injection surface after the executable
default dependency owner landed. The Python source map is:

- `backend/app/api/chapters.py`
- `backend/app/services/compat/chapter_generation_route_compat_service.py`
- `backend/app/services/chapter_candidate_executor_service.py`
- `backend/app/services/chapter_candidate_executor_wiring_service.py`
- `backend/app/services/chapter_candidate_output_service.py`
- `backend/app/services/chapter_candidate_record_service.py`

New / tightened Rust staged owners:

- `backend-rs/src/services/chapter_candidate_executor_runtime_adapter_service.rs`
- `backend-rs/src/services/chapter_candidate_executor_default_dependency_service.rs`
- `backend-rs/src/services/chapter_candidate_generation_service.rs`
- `backend-rs/src/services/chapter_candidate_word_budget_repair_service.rs`
- `backend-rs/src/services/chapter_candidate_targeted_final_repair_service.rs`
- `backend-rs/src/services/chapter_candidate_record_service.rs`
- `backend-rs/src/services/chapter_candidate_executor_wiring_service.rs`
- `backend-rs/src/services/mod.rs`

The new runtime adapter owner adds:

- `generate_best_ranked_candidate_with_runtime_adapters(...)`, which consumes
  the existing default dependency owner rather than rebuilding the executor
  dependency graph
- `resolve_default_candidate_provider_stream_request(...)`, which maps
  candidate `generate_kwargs` into Rust provider stream inputs and safe
  `AIConfig` overrides for temperature / max tokens
- `build_default_generation_candidate_record(...)`, which consumes the Rust
  candidate record owner

Contract tightened this round:

- generation, word-budget repair, targeted final repair, and default dependency
  owners now accept record callbacks returning `Result<Value, String>`
- record owner errors such as empty sanitized content can now propagate through
  the staged executor package instead of being hidden behind infallible test
  callbacks
- provider output and record build are now Rust-owned adapter boundaries
- quality evaluator and quality gate plan builder remain explicit injection
  boundaries until the production quality adapter or active route owner consumes
  the Rust package

Wiring readiness update:

- `chapter_candidate_executor_wiring_service.rs` now lists
  `backend-rs/src/services/chapter_candidate_executor_runtime_adapter_service.rs`
  in the Rust target map
- executor-stage dependencies include
  `generate_best_ranked_candidate_with_runtime_adapters`

Focused validation passed with:

- `cargo fmt --manifest-path "backend-rs/Cargo.toml"`
- `cargo test chapter_candidate_executor_runtime_adapter_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/mumunovel-candidate-runtime-adapter-owner" -- --nocapture`
  -> 3 passed
- `cargo test chapter_candidate_generation_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/mumunovel-candidate-runtime-adapter-owner" -- --nocapture`
  -> 5 passed
- `cargo test chapter_candidate_word_budget_repair_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/mumunovel-candidate-runtime-adapter-owner" -- --nocapture`
  -> 3 passed
- `cargo test chapter_candidate_targeted_final_repair_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/mumunovel-candidate-runtime-adapter-owner" -- --nocapture`
  -> 3 passed
- `cargo test chapter_candidate_executor_default_dependency_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/mumunovel-candidate-runtime-adapter-owner" -- --nocapture`
  -> 2 passed
- `cargo test chapter_candidate_executor_wiring_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/mumunovel-candidate-runtime-adapter-owner" -- --nocapture`
  -> 5 passed
- `cargo test chapter_candidate_record_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/mumunovel-candidate-runtime-adapter-owner" -- --nocapture`
  -> 3 passed
- `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/mumunovel-candidate-runtime-adapter-owner"`
  -> passed with existing unused/dead-code warnings

Next acceleration target:

1. Build the rollback-aware production adapter or route-level consumption that
   calls `generate_best_ranked_candidate_with_runtime_adapters(...)`.
2. Or migrate the remaining quality adapter as a whole module if active route
   consumption still needs one more explicit Rust boundary.

Do not count the Python candidate executor or route callback assembly as
retired until the active generation path consumes this runtime adapter.

### 2026-06-08 chapter-candidate-quality-adapter Rust staged owner checkpoint

This round continued the same Rust-first candidate executor package and moved
the next remaining production-consumption blocker as a whole adapter block:
candidate quality hook assembly. The Python source map is:

- `backend/app/services/chapter_generation/stream/candidate_service.py`
- `backend/app/services/batch_generation_candidate_service.py`
- `backend/app/services/chapter_candidate_record_service.py`
- `backend/app/services/chapter_candidate_finalize_service.py`

New / tightened Rust staged owners:

- `backend-rs/src/services/chapter_candidate_quality_adapter_service.rs`
- `backend-rs/src/services/chapter_candidate_executor_wiring_service.rs`
- `backend-rs/src/services/mod.rs`

The new quality adapter owner adds:

- `ChapterCandidateQualityAdapter`, an executable Rust adapter object that
  owns quality hook assembly while leaving the heavy quality rule callbacks
  explicit
- `build_chapter_candidate_quality_adapter(...)`
- `evaluate_quality(...)`, which projects story packet, project, chapter,
  chapter context, target word count, generation intent, generated content,
  chapter outline, world rules, and quality runtime context into stable Rust
  inputs
- `build_quality_gate_plan(...)`, which preserves retry count, max retries,
  current story repair payload, scope, and non-object metrics fallback

Wiring readiness update:

- `chapter_candidate_executor_wiring_service.rs` now includes
  `backend-rs/src/services/chapter_candidate_quality_adapter_service.rs` in the
  Rust target map
- candidate executor wiring now has an explicit `quality_adapter` stage before
  generation / repair / finalize / executor stages
- readiness still reports zero external formula blockers because this adapter
  owns hook assembly while keeping heavy quality rules as intentional injection
  boundaries

Focused validation passed with:

- `cargo fmt --manifest-path "backend-rs/Cargo.toml"`
- `cargo test chapter_candidate_quality_adapter_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/mumunovel-candidate-quality-adapter-owner" -- --nocapture`
  -> 3 passed
- `cargo test chapter_candidate_executor_wiring_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/mumunovel-candidate-quality-adapter-owner" -- --nocapture`
  -> 5 passed

Next acceleration target:

1. Wire the runtime adapter to consume `ChapterCandidateQualityAdapter`, or build
   the rollback-aware production adapter that calls
   `generate_best_ranked_candidate_with_runtime_adapters(...)`.
2. Keep quality-rule computation injectable until route parity and smoke
   coverage make a full quality-domain Rust migration safe.

Do not count Python quality hook assembly as active-path retired until the
active generation path consumes this Rust quality adapter.

### 2026-06-08 chapter-candidate-runtime-quality-adapter Rust staged owner checkpoint

This round continued the same candidate executor package and connected the
runtime adapter to the newly staged quality adapter owner. The Python source
map remains:

- `backend/app/services/chapter_generation/stream/candidate_service.py`
- `backend/app/services/batch_generation_candidate_service.py`
- `backend/app/services/chapter_candidate_executor_service.py`
- `backend/app/services/chapter_candidate_record_service.py`
- `backend/app/services/chapter_candidate_finalize_service.py`

Updated Rust staged owners:

- `backend-rs/src/services/chapter_candidate_executor_runtime_adapter_service.rs`
- `backend-rs/src/services/chapter_candidate_quality_adapter_service.rs`
- `backend-rs/src/services/chapter_candidate_executor_wiring_service.rs`

The runtime adapter now adds:

- `generate_best_ranked_candidate_with_runtime_quality_adapters(...)`, which
  accepts a `ChapterCandidateQualityAdapter` and then calls the existing
  runtime adapter path
- `build_runtime_quality_adapter_callbacks(...)`, which builds the evaluator
  and quality-gate-plan callbacks from the Rust quality adapter
- a bridge test proving generated content flows through `evaluate_quality(...)`
  and candidate metrics flow through `build_quality_gate_plan(...)`

Wiring readiness update:

- `chapter_candidate_executor_wiring_service.rs` now lists
  `generate_best_ranked_candidate_with_runtime_quality_adapters` as an
  executor-stage Rust-owned dependency
- candidate executor readiness still has zero external formula blockers

Focused validation passed with:

- `cargo fmt --manifest-path "backend-rs/Cargo.toml"`
- `cargo test chapter_candidate_executor_runtime_adapter_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/mumunovel-candidate-runtime-quality-adapter-owner" -- --nocapture`
  -> 4 passed
- `cargo test chapter_candidate_quality_adapter_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/mumunovel-candidate-runtime-quality-adapter-owner" -- --nocapture`
  -> 3 passed
- `cargo test chapter_candidate_executor_wiring_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/mumunovel-candidate-runtime-quality-adapter-owner" -- --nocapture`
  -> 5 passed
- `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/mumunovel-candidate-runtime-quality-adapter-owner"`
  -> passed with existing unused/dead-code warnings

Next acceleration target:

1. Build the rollback-aware production adapter or route-level consumption that
   constructs `ChapterCandidateQualityAdapter` from route/runtime values and
   calls `generate_best_ranked_candidate_with_runtime_quality_adapters(...)`.
2. Keep Python fallback frozen until route parity, smoke coverage, and rollback
   behavior are explicit.

Do not count Python candidate executor or quality hook assembly as active-path
retired until the active generation path consumes this runtime-quality adapter.

### 2026-06-08 chapter-candidate-production-adapter Rust staged owner checkpoint

This round continued the same candidate executor package and moved the next
whole cutover boundary into Rust: rollback-aware production adapter ownership.
This is not another internal seam; it is the route-facing decision owner that
future active generation consumption can call while preserving an explicit
Python fallback.

Python source map:

- `backend/app/api/chapters.py`
- `backend/app/services/compat/chapter_generation_route_compat_service.py`
- `backend/app/services/chapter_candidate_executor_service.py`
- `backend/app/services/chapter_candidate_executor_wiring_service.py`
- `backend/app/services/chapter_generation/stream/candidate_service.py`
- `backend/app/services/batch_generation_candidate_service.py`

New / updated Rust staged owners:

- `backend-rs/src/services/chapter_candidate_executor_production_adapter_service.rs`
- `backend-rs/src/services/chapter_candidate_executor_runtime_adapter_service.rs`
- `backend-rs/src/services/chapter_candidate_quality_adapter_service.rs`
- `backend-rs/src/services/chapter_candidate_executor_wiring_service.rs`
- `backend-rs/src/services/mod.rs`

The production adapter owner adds:

- `ChapterCandidateProductionAdapterConfig`, which carries Rust enablement,
  `fallback_on_rust_error`, disabled reason, and rollback boundary.
- `resolve_chapter_candidate_production_adapter_decision(...)`, which resolves
  Rust candidate executor vs Python fallback before execution.
- `execute_chapter_candidate_production_adapter(...)`, which defaults to
  `generate_best_ranked_candidate_with_runtime_quality_adapters(...)`.
- `execute_chapter_candidate_production_adapter_with_executor(...)`, a test
  hook that proves production cutover and rollback decisions without invoking
  real provider output.

Behavior contract preserved / staged:

- Rust disabled -> Python fallback runs and Rust executor is not called.
- Rust enabled + Rust success -> result is returned with
  `RustCandidateExecutor` decision and no fallback marker.
- Rust enabled + Rust failure + rollback enabled -> Python fallback runs with
  Rust error, fallback reason, and rollback boundary in the fallback context.
- Rust enabled + Rust failure + rollback disabled -> Rust error propagates.
- The owner records rollback metadata but does not remove Python fallback.

Wiring readiness update:

- `chapter_candidate_executor_wiring_service.rs` now includes a
  `production_adapter` stage before quality/generation/repair/finalize/executor
  stages.
- Rust target map now includes
  `backend-rs/src/services/chapter_candidate_executor_production_adapter_service.rs`.
- Candidate executor readiness remains at zero external formula blockers and
  now has seven staged owner stages.

Focused validation passed with:

- `cargo fmt --manifest-path "backend-rs/Cargo.toml"`
- `cargo test chapter_candidate_executor_production_adapter_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/mumunovel-candidate-production-adapter-owner" -- --nocapture`
  -> 5 passed
- `cargo test chapter_candidate_executor_wiring_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/mumunovel-candidate-production-adapter-owner" -- --nocapture`
  -> 5 passed

Next acceleration target:

1. Make a route/deployment gateway consume
   `execute_chapter_candidate_production_adapter(...)` with a real cutover flag
   and smoke probe.
2. Keep Python fallback intact until route parity and smoke evidence prove the
   Rust candidate executor can own the active path.

Do not count Python candidate executor, route callback assembly, or quality
hook assembly as active-path retired until the active generation route consumes
this production adapter.

### 2026-06-08 chapter-candidate-route-gateway Rust staged owner checkpoint

This round continued the same candidate executor package and moved the next
deployment-facing cutover boundary into Rust: route/deployment gateway config
ownership for the rollback-aware production adapter. This keeps the migration
on a whole-block Rust owner instead of adding another Python compatibility
cleanup. It is still staged because the active generation route has not yet
been repointed to this gateway.

Python source map:

- `backend/app/api/chapters.py`
- `backend/app/services/compat/chapter_generation_route_compat_service.py`
- `backend/app/services/chapter_candidate_executor_service.py`
- `backend/app/services/chapter_candidate_executor_wiring_service.py`

New / updated Rust staged owners:

- `backend-rs/src/services/chapter_candidate_route_gateway_service.rs`
- `backend-rs/src/services/chapter_candidate_executor_production_adapter_service.rs`
- `backend-rs/src/services/chapter_candidate_executor_wiring_service.rs`
- `backend-rs/src/services/mod.rs`
- `backend-rs/src/config.rs`
- `backend-rs/src/api/router.rs`

The route gateway owner adds:

- `ChapterCandidateRouteGatewayConfig`, which carries route/deployment
  enablement, rollback-on-error, disabled reason, and rollback boundary.
- `build_chapter_candidate_route_gateway_config_from_app_config(...)`, which
  maps `AppConfig` cutover fields into the gateway owner.
- `build_chapter_candidate_production_adapter_config_from_route_gateway(...)`,
  which prevents routes from rebuilding production-adapter config locally.
- `execute_chapter_candidate_route_gateway(...)`, which delegates to
  `execute_chapter_candidate_production_adapter(...)`.
- `execute_chapter_candidate_route_gateway_with_executor(...)`, a test hook
  that proves gateway cutover and fallback decisions without invoking a real
  provider.

Deployment config added:

- `CHAPTER_CANDIDATE_RUST_EXECUTOR_ENABLED`
- `CHAPTER_CANDIDATE_RUST_EXECUTOR_FALLBACK_ON_ERROR`
- `CHAPTER_CANDIDATE_RUST_EXECUTOR_DISABLED_REASON`
- `CHAPTER_CANDIDATE_RUST_EXECUTOR_ROLLBACK_BOUNDARY`

Behavior contract preserved / staged:

- Rust executor remains disabled by default.
- Fallback on Rust error remains enabled by default.
- Blank disabled reason normalizes to `None`.
- Blank rollback boundary normalizes to
  `python_candidate_executor_fallback`.
- Rust enabled + Rust success returns the Rust result without fallback.
- Rust disabled calls the Python fallback and does not invoke the Rust executor.
- The gateway delegates execution decisions to the production adapter; it does
  not duplicate the adapter's rollback logic.

Wiring readiness update:

- `chapter_candidate_executor_wiring_service.rs` now includes a
  `route_gateway` stage before `production_adapter`.
- Rust target map now includes
  `backend-rs/src/services/chapter_candidate_route_gateway_service.rs`.
- Candidate executor readiness remains at zero external formula blockers and
  now has eight staged owner stages.

Focused validation passed with:

- `cargo fmt --manifest-path "backend-rs/Cargo.toml"`
- `cargo test chapter_candidate_route_gateway_service --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-candidate-route-gateway-owner" -- --nocapture`
  -> 4 passed
- `cargo test chapter_candidate_executor_production_adapter_service --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-candidate-route-gateway-owner" -- --nocapture`
  -> 5 passed
- `cargo test chapter_candidate_executor_wiring_service --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-candidate-route-gateway-owner" -- --nocapture`
  -> 5 passed
- `cargo test chapter_candidate_executor_runtime_adapter_service --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-candidate-route-gateway-owner" -- --nocapture`
  -> 4 passed
- `cargo test chapter_candidate_quality_adapter_service --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-candidate-route-gateway-owner" -- --nocapture`
  -> 3 passed
- `cargo test config --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-candidate-route-gateway-owner" -- --nocapture`
  -> 21 passed
- `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-candidate-route-gateway-owner"`
  -> passed with existing unused/dead-code warnings

Validation note:

- The earlier `D:/codex-targets/...` validation path failed because D: had no
  free space (`no space on device`). This checkpoint used a dedicated C:
  target dir and did not delete any existing build artifacts.

Next acceleration target:

1. Add an active Rust route/deployment smoke consumption point that calls
   `execute_chapter_candidate_route_gateway(...)` and records Rust-vs-Python
   fallback evidence.
2. Keep Python fallback intact until route parity, smoke coverage, and rollback
   behavior are explicit.

Do not count Python candidate executor, route callback assembly, or quality
hook assembly as active-path retired until the active generation route or a
deployment gateway consumes this route gateway.

### 2026-06-08 chapter-candidate-route-gateway-smoke Rust staged owner checkpoint

This round continued the same candidate executor package and added a Rust
deployment-smoke owner that directly consumes the route gateway. This is a
stronger staged checkpoint than route-gateway config alone because the smoke
suite calls the gateway execution hook and proves both Rust and Python fallback
paths can be reached. It still does not retire the active Python generation
route because the active generation route still has not been repointed to this
gateway. The smoke suite is now observable through a Rust health endpoint and
the deployment manifest.

Python source map:

- `backend/app/api/chapters.py`
- `backend/app/services/compat/chapter_generation_route_compat_service.py`
- `backend/app/services/chapter_candidate_executor_service.py`
- `backend/app/services/chapter_candidate_executor_wiring_service.py`
- `backend/tools/run_strangler_gateway_smoke.py`
- `deploy/strangler-gateway-probes.json`

New / updated Rust staged owners:

- `backend-rs/src/services/chapter_candidate_route_gateway_smoke_service.rs`
- `backend-rs/src/services/chapter_candidate_route_gateway_service.rs`
- `backend-rs/src/services/chapter_candidate_executor_wiring_service.rs`
- `backend-rs/src/services/mod.rs`
- `backend-rs/src/api/health.rs`
- `backend-rs/src/middleware/auth.rs`
- `deploy/strangler-gateway-probes.json`

The smoke owner adds:

- `ChapterCandidateRouteGatewaySmokeProbe`, which carries probe name, owner,
  route group, and route-gateway config.
- `ChapterCandidateRouteGatewaySmokeResult`, which carries probe metadata,
  execution path, fallback flag, fallback reason, rollback boundary, optional
  Rust error, result payload, and runtime state.
- `build_default_chapter_candidate_route_gateway_smoke_probes(...)`, which
  creates one Rust-owner probe and one forced Python-fallback probe.
- `run_chapter_candidate_route_gateway_smoke_suite(...)`, which executes both
  default probes.
- `run_chapter_candidate_route_gateway_smoke_probe(...)`, which calls
  `execute_chapter_candidate_route_gateway_with_executor(...)` and therefore
  consumes the Rust route gateway instead of bypassing it.
- `GET /health/chapter-candidate-route-gateway-smoke`, which exposes the smoke
  suite as a Rust-owned deployment probe without invoking a real provider.

Behavior contract preserved / staged:

- Rust smoke probe enables the Rust candidate executor and returns
  `gateway_consumed = true`.
- Python fallback smoke probe disables the Rust candidate executor, preserves
  the fallback reason, and returns the rollback boundary.
- Runtime state records `gateway_smoke = rust` or
  `gateway_smoke = python-fallback`.
- Rollback boundary remains `python_candidate_executor_fallback`.
- The smoke endpoint is public like the existing health probes because it uses
  fake-provider smoke data and does not expose user data.
- The active Python generation route is still not retired; active-path
  retirement requires route parity and a route-level call to the Rust gateway.

Wiring readiness update:

- `chapter_candidate_executor_wiring_service.rs` now includes
  `route_gateway_smoke` before `route_gateway`.
- Rust target map now includes
  `backend-rs/src/services/chapter_candidate_route_gateway_smoke_service.rs`.
- `deploy/strangler-gateway-probes.json` now includes
  `chapter-candidate-route-gateway-smoke-rust` for the `chapters` route group.
- Candidate executor readiness remains at zero external formula blockers and
  now has nine staged owner stages.

Focused validation passed with:

- `cargo fmt --manifest-path "backend-rs/Cargo.toml"`
- `cargo test chapter_candidate_route_gateway_smoke_service --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-candidate-route-gateway-smoke-owner" -- --nocapture`
  -> 3 passed
- `cargo test chapter_candidate_route_gateway_service --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-candidate-route-gateway-smoke-owner" -- --nocapture`
  -> 4 passed
- `cargo test chapter_candidate_executor_wiring_service --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-candidate-route-gateway-smoke-owner" -- --nocapture`
  -> 5 passed
- `cargo test chapter_candidate_executor_production_adapter_service --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-candidate-route-gateway-smoke-owner" -- --nocapture`
  -> 5 passed
- `cargo test health --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-candidate-route-gateway-smoke-owner" -- --nocapture`
  -> 2 passed
- `cargo test auth::tests::exact_public_paths_remain_public --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-candidate-route-gateway-smoke-owner" -- --nocapture`
  -> 1 passed
- `python backend/tools/run_strangler_gateway_smoke.py --validate-manifest-only`
  -> passed; manifest probe count is 169 and `chapters` now includes
  `chapter-candidate-route-gateway-smoke-rust`
- `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-candidate-route-gateway-smoke-owner"`
  -> passed with existing unused/dead-code warnings

Next acceleration target:

1. Keep the new observable Rust deployment-smoke route in the deployment
   manifest and run it in deployment smoke jobs.
2. Only after that runtime smoke evidence stays green, consider repointing the
   active generation route from the Python candidate executor fallback toward
   the Rust route gateway.

Do not count Python candidate executor, route callback assembly, or quality
hook assembly as active-path retired until an active route or deployment runner
consumes this smoke owner and keeps Python fallback rollback evidence.

### 2026-06-08 chapter-candidate-executor-send-safe active-route-prep checkpoint

This round continued the candidate executor package as a whole function-group
active-route preparation pass. The package no longer keeps the runtime/default
dependency callback state in `Rc<RefCell<_>>`; the shared callback state now
uses `Arc<Mutex<_>>`, and the runtime, production adapter, and route gateway
generic boundaries require `Send` for quality callback owners that cross the
Rust executor boundary. This is real Rust cutover preparation because a direct
Axum active route cannot safely await the candidate executor while the executor
future is pinned to non-`Send` callback holders.

Python source / fallback map:

- `backend/app/api/chapters.py`
- `backend/app/services/compat/chapter_generation_route_compat_service.py`
- `backend/app/services/chapter_candidate_executor_service.py`
- `backend/app/services/chapter_candidate_executor_wiring_service.py`

Updated Rust owners:

- `backend-rs/src/services/chapter_candidate_executor_runtime_adapter_service.rs`
- `backend-rs/src/services/chapter_candidate_executor_default_dependency_service.rs`
- `backend-rs/src/services/chapter_candidate_executor_production_adapter_service.rs`
- `backend-rs/src/services/chapter_candidate_route_gateway_service.rs`

Behavior contract preserved:

- Candidate generation, retry, word-budget repair, targeted final repair, and
  finalization still compose through the existing Rust staged owners.
- Quality evaluation and quality-gate plan callbacks remain injectable.
- Locks are used only to invoke mutable callback owners; provider/collector
  futures are created inside the lock and awaited after the lock guard is
  dropped.
- Poisoned callback mutexes recover by taking the inner callback owner instead
  of panicking through an `unwrap()`.
- Python fallback remains the rollback boundary and may still be non-`Send`
  until the active route is repointed.

Cutover boundary:

- This checkpoint removes the main non-`Send` blocker from the Rust candidate
  executor runtime/default dependency path.
- It does not remove the temporary health smoke `spawn_blocking` quarantine.
- It does not retire the active Python generation route. Active-path retirement
  still requires an active route call to the Rust gateway, route parity, smoke
  evidence, and rollback confirmation.

Focused validation passed with:

- `cargo fmt --manifest-path "backend-rs/Cargo.toml"`
- `cargo test chapter_candidate_executor_runtime_adapter_service --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-candidate-route-gateway-smoke-owner" -- --nocapture`
  -> 4 passed
- `cargo test chapter_candidate_executor_default_dependency_service --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-candidate-route-gateway-smoke-owner" -- --nocapture`
  -> 2 passed
- `cargo test chapter_candidate_executor_production_adapter_service --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-candidate-route-gateway-smoke-owner" -- --nocapture`
  -> 5 passed
- `cargo test chapter_candidate_route_gateway_service --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-candidate-route-gateway-smoke-owner" -- --nocapture`
  -> 4 passed
- `cargo test chapter_candidate_route_gateway_smoke_service --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-candidate-route-gateway-smoke-owner" -- --nocapture`
  -> 3 passed
- `cargo test health --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-candidate-route-gateway-smoke-owner" -- --nocapture`
  -> 2 passed
- `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-candidate-route-gateway-smoke-owner"`
  -> passed with existing unused/dead-code warnings

Next acceleration target:

1. Add a direct Send-safe route/handler consumption path for the Rust candidate
   route gateway so the health smoke endpoint no longer needs the
   `spawn_blocking` quarantine.
2. Then repoint the active generation route behind an explicit rollback knob,
   preserving Python fallback until route parity and deployment smoke evidence
   stay green.

### 2026-06-08 chapter-candidate-route-gateway-smoke direct-async checkpoint

This round finished the next active-route preparation block for the candidate
executor package. The Rust health smoke route now directly awaits
`run_chapter_candidate_route_gateway_smoke_suite()` instead of hiding the
gateway smoke future behind `tokio::task::spawn_blocking`, a nested
current-thread runtime, and `block_on(...)`.

Python source / fallback map:

- `backend/app/api/chapters.py`
- `backend/app/services/compat/chapter_generation_route_compat_service.py`
- `backend/app/services/chapter_candidate_executor_service.py`
- `backend/app/services/chapter_candidate_executor_wiring_service.py`

Updated Rust owners:

- `backend-rs/src/api/health.rs`
- `backend-rs/src/services/chapter_candidate_route_gateway_smoke_service.rs`
- `backend-rs/src/services/chapter_candidate_route_gateway_service.rs`
- `backend-rs/src/services/chapter_candidate_executor_production_adapter_service.rs`

Behavior contract preserved:

- `GET /health/chapter-candidate-route-gateway-smoke` keeps the same response
  payload: status, owner, route group, probe count, rollback boundary, and Rust
  / Python-fallback probe results.
- The smoke suite still proves both gateway paths:
  `rust_candidate_executor` and `python_fallback`.
- Rollback boundary remains `python_candidate_executor_fallback`.
- The endpoint still uses fake-provider smoke data and does not call a real AI
  provider.

Send-safe boundary tightened:

- `chapter_candidate_executor_production_adapter_service.rs` now requires
  boxed Rust executor and Python fallback futures to be
  `Pin<Box<dyn Future<Output = Result<Value, String>> + Send + 'request>>`.
- `chapter_candidate_route_gateway_service.rs` mirrors the same `Send` boxed
  future contract for route-gateway hooks.
- `health.rs` no longer contains `spawn_blocking`, nested runtime creation, or
  `block_on(...)` for this smoke route.

Cutover boundary:

- This checkpoint removes the temporary health-smoke quarantine and proves the
  route gateway smoke owner can be consumed through a normal Axum async
  handler.
- It still does not retire the active Python generation route. Active-path
  retirement requires the production generation route to consume the Rust
  gateway behind an explicit rollback knob.

Focused validation passed with:

- `cargo fmt --manifest-path "backend-rs/Cargo.toml"`
- `cargo test health --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-candidate-route-gateway-smoke-owner" -- --nocapture`
  -> 2 passed
- `cargo test chapter_candidate_route_gateway_smoke_service --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-candidate-route-gateway-smoke-owner" -- --nocapture`
  -> 3 passed
- `cargo test chapter_candidate_route_gateway_service --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-candidate-route-gateway-smoke-owner" -- --nocapture`
  -> 4 passed
- `cargo test chapter_candidate_executor_production_adapter_service --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-candidate-route-gateway-smoke-owner" -- --nocapture`
  -> 5 passed
- `rg -n "spawn_blocking|current_thread|block_on\\(run_chapter_candidate_route_gateway_smoke_suite" "backend-rs/src/api/health.rs" "backend-rs/src/services/chapter_candidate_route_gateway_smoke_service.rs"`
  -> no matches
- `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-candidate-route-gateway-smoke-owner"`
  -> passed with existing unused/dead-code warnings

Next acceleration target:

1. Repoint an active generation-route test hook or thin route adapter to call
   `execute_chapter_candidate_route_gateway(...)` behind the existing rollback
   config.
2. Keep the Python fallback shell frozen until route parity, smoke, and
   rollback evidence stay green.

### 2026-06-08 chapter-single-generation active-route gateway consumption checkpoint

This round moved from smoke-only gateway evidence into active Rust
single-generation route consumption. Both Rust stream and background
generation routes now extract `AppConfig`, derive
`ChapterCandidateRouteGatewayConfig`, and pass it through the
single-generation service chain to the runtime owner. The runtime owner calls
`execute_chapter_candidate_route_gateway(...)` before persistence, while the
default disabled-gateway path preserves the existing direct AI generation
fallback.

Python source / fallback map:

- `backend/app/api/chapter_generation_routes.py`
- `backend/app/api/chapters.py`
- `backend/app/services/compat/chapter_generation_route_compat_service.py`
- `backend/app/services/chapter_generation/stream/entry_service.py`
- `backend/app/services/chapter_candidate_executor_service.py`

Updated Rust owners:

- `backend-rs/src/api/chapter_generation_routes.rs`
- `backend-rs/src/services/chapter_generation_runtime_service.rs`
- `backend-rs/src/services/chapter_single_generation_runtime_state_service.rs`
- `backend-rs/src/services/chapter_single_generation_stream_entry_service.rs`
- `backend-rs/src/services/chapter_single_generation_stream_workflow_service.rs`
- `backend-rs/src/services/chapter_single_generation_write_workflow_service.rs`

Behavior contract preserved:

- HTTP route paths and request payload fields are unchanged.
- SSE success/error projection remains owned by
  `chapter_single_generation_stream_workflow_service.rs`.
- Background task creation, startup snapshot persistence, and lifecycle
  checkpoint semantics remain unchanged.
- Default runtime behavior remains direct single-generation AI fallback while
  `CHAPTER_CANDIDATE_RUST_EXECUTOR_ENABLED=false`.
- When the Rust candidate executor is enabled, the active Rust
  single-generation runtime consumes the route gateway before converting the
  selected candidate content back into the existing `GeneratedChapterResult`
  persistence path.
- Rust executor errors still follow the route gateway fallback policy.

Cutover boundary:

- This is active Rust route consumption, not just a health smoke. It does not
  retire the Python FastAPI generation route or Python candidate executor
  fallback shell.
- The fallback branch inside the Rust route remains direct single-generation
  generation, so Python source files remain frozen source map until route
  parity and deployment smoke evidence justify shrinking them.
- Candidate quality metrics in this active route gateway pass are still a
  minimal Rust adapter boundary; full quality-rule parity remains a separate
  package before default-enabling the Rust candidate executor.

Focused validation passed with:

- `cargo fmt --manifest-path "backend-rs/Cargo.toml"`
- `cargo test chapter_generation_runtime_service --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-candidate-route-gateway-smoke-owner" -- --nocapture`
  -> 16 passed
- `cargo test chapter_single_generation_runtime_state_service --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-candidate-route-gateway-smoke-owner" -- --nocapture`
  -> 14 passed
- `cargo test chapter_single_generation_stream_entry_service --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-candidate-route-gateway-smoke-owner" -- --nocapture`
  -> 7 passed
- `cargo test chapter_single_generation_write_workflow_service --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-candidate-route-gateway-smoke-owner" -- --nocapture`
  -> 4 passed
- `cargo test chapter_generation_routes --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-candidate-route-gateway-smoke-owner" -- --nocapture`
  -> 3 passed
- `cargo test chapter_single_generation_stream_workflow_service --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-candidate-route-gateway-smoke-owner" -- --nocapture`
  -> 20 passed
- `cargo test chapter_candidate_route_gateway_service --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-candidate-route-gateway-smoke-owner" -- --nocapture`
  -> 4 passed
- `cargo test health --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-candidate-route-gateway-smoke-owner" -- --nocapture`
  -> 2 passed
- `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-candidate-route-gateway-smoke-owner"`
  -> passed with existing unused/dead-code warnings

Next acceleration target:

1. Add a route-level / smoke-level parity probe that exercises
   `CHAPTER_CANDIDATE_RUST_EXECUTOR_ENABLED=true` for the active
   single-generation gateway path without real provider calls.
2. Promote the candidate quality-rule adapter from minimal metrics to a
   parity-backed owner before default-enabling Rust candidate execution.
3. Only after those two are stable, shrink or repoint the Python
   `chapter_generation_route_compat_service.py` / candidate executor fallback
   shell.

### 2026-06-08 chapter-single-generation active gateway no-provider parity checkpoint

This round finished the next active-route evidence block for the
`chapter_single_generation` package. A new Rust smoke owner now exercises the
active single-generation candidate gateway boundary with
`CHAPTER_CANDIDATE_RUST_EXECUTOR_ENABLED=true` semantics and a direct
single-generation fallback path, while using fake executor outputs so no real
provider call is made.

Python source / fallback map:

- `backend/app/api/chapter_generation_routes.py`
- `backend/app/api/chapters.py`
- `backend/app/services/compat/chapter_generation_route_compat_service.py`
- `backend/app/services/chapter_generation/stream/entry_service.py`
- `backend/app/services/chapter_candidate_executor_service.py`

Updated Rust owners:

- `backend-rs/src/services/chapter_single_generation_active_gateway_smoke_service.rs`
- `backend-rs/src/api/health.rs`
- `backend-rs/src/middleware/auth.rs`
- `backend-rs/src/services/chapter_generation_runtime_service.rs`
- `backend-rs/src/services/chapter_candidate_executor_production_adapter_service.rs`
- `backend-rs/src/services/chapter_candidate_route_gateway_smoke_service.rs`
- `backend-rs/src/services/mod.rs`
- `deploy/strangler-gateway-probes.json`

Behavior contract preserved:

- The production single-generation routes remain behind the existing
  `AppConfig`-derived gateway config and do not default-enable the Rust
  candidate executor.
- `GET /health/chapter-single-generation-active-gateway-smoke` proves the
  active single-generation request/content-normalization gateway boundary
  without calling a real AI provider.
- The enabled probe runs through the Rust candidate executor path and extracts
  candidate `full_content`.
- The disabled probe runs through the active Rust route's direct-generation
  fallback shape and extracts fallback `content`.
- The smoke payload exposes route group, execution path, fallback flag,
  rollback boundary, content, result payload, and runtime state.
- The new public health path is explicitly allowed by auth middleware.
- The strangler gateway manifest now includes
  `chapter-single-generation-active-gateway-smoke-rust` under deploy,
  route-groups, and phase5-p1 profiles.
- Python FastAPI route and Python candidate executor fallback shell remain
  frozen source maps; they are not retired by this checkpoint.

Cutover boundary:

- This closes the gap between active-route gateway consumption and enabled-path
  observability. It is stronger than a generic gateway health smoke because it
  uses the single-generation request builder and content extractor.
- The rollback boundary remains `legacy_single_generation_direct_ai` for this
  active Rust route path.
- Full candidate quality-rule parity is still not complete; do not
  default-enable Rust candidate execution until the quality adapter becomes
  parity-backed.

Focused validation passed with:

- `cargo fmt --manifest-path "backend-rs/Cargo.toml"`
- `cargo test chapter_single_generation_active_gateway_smoke_service --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-candidate-route-gateway-smoke-owner" -- --nocapture`
  -> 3 passed
- `cargo test health --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-candidate-route-gateway-smoke-owner" -- --nocapture`
  -> 3 passed
- `cargo test auth --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-candidate-route-gateway-smoke-owner" -- --nocapture`
  -> 10 passed
- `cargo test chapter_candidate_route_gateway_smoke_service --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-candidate-route-gateway-smoke-owner" -- --nocapture`
  -> 3 passed
- `cargo test chapter_generation_runtime_service --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-candidate-route-gateway-smoke-owner" -- --nocapture`
  -> 16 passed
- `cargo test chapter_candidate_executor_production_adapter_service --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-candidate-route-gateway-smoke-owner" -- --nocapture`
  -> 5 passed
- `cargo test chapter_candidate_route_gateway_service --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-candidate-route-gateway-smoke-owner" -- --nocapture`
  -> 4 passed
- `python backend/tools/run_strangler_gateway_smoke.py --validate-manifest-only`
  -> passed; manifest probe count is 170 and `chapter_single_generation`
  contains `chapter-single-generation-active-gateway-smoke-rust`

Next acceleration target:

1. Promote the minimal single-generation candidate quality adapter into a
   parity-backed Rust quality-rule owner.
2. Only after enabled-path smoke plus quality parity are stable, shrink or
   repoint `chapter_generation_route_compat_service.py` and the Python
   candidate executor fallback shell.

### 2026-06-08 chapter-single-generation candidate quality rule owner checkpoint

This round stayed on Package B, `chapter_single_generation`, and migrated the
next whole function group from the Python quality source map into Rust. The
active Rust single-generation candidate gateway no longer uses an inline fake
`overall_score: 80.0` / unconditional pass gate. A new Rust owner now computes
single-generation candidate quality metrics, reuses the existing Rust
story-repair quality context owner for gate/repair derivation, and exposes a
retry-aware gate plan back through the existing candidate quality adapter
contract.

Python source / fallback map:

- `backend/app/services/quality_domain/story_quality_feedback_service.py`
  - source map for `compute_story_quality_metrics(...)`
  - source map for `build_quality_gate_decision(...)`
- `backend/app/services/chapter_candidate_executor_service.py`
- `backend/app/services/compat/chapter_generation_route_compat_service.py`

Updated Rust owners:

- `backend-rs/src/services/chapter_single_generation_candidate_quality_service.rs`
- `backend-rs/src/services/chapter_generation_runtime_service.rs`
- `backend-rs/src/services/mod.rs`
- `backend-rs/src/services/chapter_story_repair_quality_context_service.rs`
  remains the reused quality gate / repair-guidance derivation owner.

Behavior contract preserved:

- Active route HTTP/SSE/task payload shapes are unchanged.
- The candidate quality adapter still receives the same runtime context,
  metrics input, and gate-plan input shapes.
- Metrics now include real single-generation heuristic fields:
  `overall_score`, `word_count`, seven quality rates, `details`, optional
  `quality_runtime_context`, `repair_guidance`, and `quality_gate`.
- Gate plan output keeps `action`, `quality_gate`, `quality_metrics`,
  retry budget, scope, and current story-repair payload.
- `auto_repair` only maps to `retry` while retry budget remains; exhausted
  budget keeps `continue` but preserves the non-pass gate for downstream
  metadata and review visibility.
- The Python FastAPI route and Python candidate executor fallback shell remain
  frozen source maps. This checkpoint narrows the Rust active route quality
  gap but does not claim full Python quality-domain retirement.

Cutover boundary:

- This is stronger than hook assembly migration because the active Rust runtime
  now consumes a Rust quality-rule owner through the production adapter
  boundary.
- It is still not enough to default-enable the Rust candidate executor:
  deployment smoke and deeper quality-domain parity should remain explicit
  follow-up gates.
- Rollback remains the active Rust route's
  `legacy_single_generation_direct_ai` fallback plus the still-frozen Python
  source maps.

Focused validation passed with:

- `cargo fmt --manifest-path "backend-rs/Cargo.toml"`
- `cargo test chapter_single_generation_candidate_quality_service --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-candidate-route-gateway-smoke-owner" -- --nocapture`
  -> 5 passed
- `cargo test chapter_candidate_quality_adapter_service --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-candidate-route-gateway-smoke-owner" -- --nocapture`
  -> 3 passed
- `cargo test chapter_generation_runtime_service --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-candidate-route-gateway-smoke-owner" -- --nocapture`
  -> 16 passed
- `cargo test chapter_single_generation_active_gateway_smoke_service --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-candidate-route-gateway-smoke-owner" -- --nocapture`
  -> 3 passed
- `cargo test chapter_candidate_executor_runtime_adapter_service --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-candidate-route-gateway-smoke-owner" -- --nocapture`
  -> 4 passed
- `python backend/tools/run_strangler_gateway_smoke.py --validate-manifest-only`
  -> passed; manifest probe count remains 170 and
  `chapter_single_generation` still contains
  `chapter-single-generation-active-gateway-smoke-rust`
- `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-candidate-route-gateway-smoke-owner"`
  -> passed with existing unused/dead-code warnings

Next acceleration target:

1. Continue the quality migration as a larger owner package only if the next
   block retires another Python quality-domain function group; otherwise move
   to the next `chapter_single_generation` whole-file owner gap.
2. Keep Python fallback shrink as a follow-up after Rust quality owner,
   enabled-path smoke, rollback, and deployment checks are all stable.

### 2026-06-08 chapter-single-generation continuity preflight quality owner checkpoint

This round continued the same Package B quality owner instead of starting a
new seam. The next Python quality-domain function group has been moved into
Rust: `build_story_continuity_preflight(...)` plus its continuity ledger specs
and anchor extraction behavior now live inside
`chapter_single_generation_candidate_quality_service.rs`.

Python source / fallback map:

- `backend/app/services/quality_domain/story_quality_feedback_service.py`
  - source map for `_CONTINUITY_LEDGER_SPECS`
  - source map for `_extract_continuity_anchor_candidates(...)`
  - source map for `build_story_continuity_preflight(...)`
- Python FastAPI route and Python candidate executor fallback shell remain
  frozen; no fallback shrink is counted in this checkpoint.

Updated Rust owners:

- `backend-rs/src/services/chapter_single_generation_candidate_quality_service.rs`
- `.trellis/spec/backend/quality-guidelines.md`
- `.trellis/tasks/05-18-backend-chapter-generation-refactor-followup/implement.md`
- `docs/architecture/rust-strangler-refactor-plan-2026-05-17.zh-CN.md`

Behavior contract preserved:

- Existing candidate quality adapter input/output shapes are unchanged.
- When `quality_runtime_context` contains continuity ledgers, metrics now
  include `continuity_preflight`.
- Missing character/relationship/foreshadow/organization/career handoff
  anchors produce up to four warning records with `ledger_key`, `ledger_label`,
  `focus_area`, `item`, anchors, matched count, and required count.
- The quality gate payload is enriched with `continuity_warning_count`,
  `continuity_preflight`, continuity focus areas, and continuity repair
  targets, matching the Python `build_quality_gate_decision(...)` data shape
  more closely.
- When all anchors are present, `continuity_preflight.status` is `ok` with
  `warning_count: 0`, so downstream metadata can distinguish clean continuity
  from missing runtime context.

Cutover boundary:

- This retires another Python quality-domain function group from the active
  Rust single-generation candidate path.
- It still does not retire the whole Python quality domain or default-enable
  the Rust candidate executor.
- Rollback remains the active Rust route's direct AI fallback plus frozen
  Python source maps.

Focused validation passed with:

- `cargo fmt --manifest-path "backend-rs/Cargo.toml"`
- `cargo test chapter_single_generation_candidate_quality_service --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-candidate-route-gateway-smoke-owner" -- --nocapture`
  -> 7 passed
- `cargo test chapter_generation_runtime_service --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-candidate-route-gateway-smoke-owner" -- --nocapture`
  -> 16 passed
- `cargo test chapter_candidate_executor_runtime_adapter_service --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-candidate-route-gateway-smoke-owner" -- --nocapture`
  -> 4 passed
- `cargo test chapter_single_generation_active_gateway_smoke_service --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-candidate-route-gateway-smoke-owner" -- --nocapture`
  -> 3 passed
- `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-candidate-route-gateway-smoke-owner"`
  -> passed with existing unused/dead-code warnings
- `python backend/tools/run_strangler_gateway_smoke.py --validate-manifest-only`
  -> passed; manifest probe count remains 170 and
  `chapter_single_generation` still contains
  `chapter-single-generation-active-gateway-smoke-rust`

Next acceleration target:

1. Continue quality only by moving the next whole Python quality-domain
   function group; otherwise move to the next `chapter_single_generation`
   whole-file owner gap.
2. Do not shrink Python fallback shells until the Rust owner, enabled-path
   smoke, rollback, and deployment checks remain stable.

### 2026-06-08 chapter-single-generation runtime pressure quality owner checkpoint

This round continued the same Package B quality owner and moved the next
Python quality-domain function group into Rust. The shared story-repair quality
context owner now carries the Python `_build_runtime_pressure(...)` continuity
ledger contract for character, relationship, foreshadow, organization, and
career ledgers, and consumes the pressure-driven branch of
`_resolve_metric_threshold_adjustments(...)` during quality gate derivation.

Python source / fallback map:

- `backend/app/services/quality_domain/story_quality_feedback_service.py`
  - source map for `_build_runtime_pressure(...)`
  - source map for the runtime-pressure branch of
    `_resolve_metric_threshold_adjustments(...)`
- Python FastAPI route and Python candidate executor fallback shell remain
  frozen; no fallback shrink is counted in this checkpoint.

Updated Rust owners:

- `backend-rs/src/services/chapter_story_repair_quality_context_service.rs`
- `.trellis/spec/backend/quality-guidelines.md`
- `.trellis/tasks/05-18-backend-chapter-generation-refactor-followup/implement.md`
- `docs/architecture/rust-strangler-refactor-plan-2026-05-17.zh-CN.md`

Behavior contract preserved:

- Existing candidate quality adapter input/output shapes are unchanged.
- `quality_runtime_pressure` now exposes character, relationship, foreshadow,
  organization, and career ledger counts plus normalized item samples.
- Runtime context normalization accepts arrays, strings, objects, and scalar
  values instead of silently ignoring non-array pressure inputs.
- Quality gate failed metrics now consume pressure-adjusted weak thresholds for
  organization, career, relationship, character, and foreshadow ledger pressure.
- This is a real Rust quality owner migration, but it is not the full adaptive
  quality profile migration; preset/style/genre profile parity remains a
  follow-up quality-domain package.

Cutover boundary:

- This narrows the active Rust single-generation quality gate gap by making
  runtime continuity pressure affect Rust gate decisions.
- It still does not retire the whole Python quality domain or default-enable
  the Rust candidate executor.
- Rollback remains the active Rust route's direct AI fallback plus frozen
  Python source maps.

Focused validation passed with:

- `cargo fmt --manifest-path "backend-rs/Cargo.toml"`
- `cargo test chapter_story_repair_quality_context_service --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-candidate-route-gateway-smoke-owner" -- --nocapture`
  -> 9 passed
- `cargo test chapter_single_generation_candidate_quality_service --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-candidate-route-gateway-smoke-owner" -- --nocapture`
  -> 7 passed
- `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-candidate-route-gateway-smoke-owner"`
  -> passed with existing unused/dead-code warnings

Next acceleration target:

1. Continue quality only if the next block moves the adaptive preset/style/genre
   profile function group into Rust; otherwise move to the next
   `chapter_single_generation` whole-file owner gap.
2. Keep Python fallback shrink as a follow-up after Rust quality owner,
   enabled-path smoke, rollback, and deployment checks remain stable.

### 2026-06-08 chapter-single-generation adaptive quality profile owner checkpoint

This round continued the same Package B quality owner and moved the adaptive
quality profile function group into Rust as a coherent owner package. The new
Rust profile owner now carries preset, style, genre, focus-weight, label, and
threshold-adjustment semantics that were previously source-mapped to Python.

Python source / fallback map:

- `backend/app/services/quality_domain/novel_quality_profile_service.py`
  - source map for `QUALITY_FOCUS_LABELS`
  - source map for `QUALITY_PROFILE_STYLE_LABELS`
  - source map for `QUALITY_PROFILE_GENRE_LABELS`
  - source map for `QUALITY_PROFILE_PRESET_LABELS`
  - source map for `_normalize_profile_token(...)`
  - source map for `_normalize_profile_token_sequence(...)`
  - source map for `resolve_runtime_quality_profile(...)`
  - source map for `_apply_focus_weight(...)`
  - source map for `resolve_quality_weight_profile(...)`
- `backend/app/services/quality_domain/novel_quality_rules.py`
  - source map for `detect_style_profile(...)`
  - source map for `detect_genre_profiles(...)`
  - source map for the style/genre trigger tables
- Python FastAPI route and Python candidate executor fallback shell remain
  frozen; no fallback shrink is counted in this checkpoint.

Updated Rust owners:

- `backend-rs/src/services/novel_quality_profile_service.rs`
- `backend-rs/src/services/chapter_story_repair_quality_context_service.rs`
- `backend-rs/src/services/mod.rs`
- `.trellis/spec/backend/quality-guidelines.md`
- `.trellis/tasks/05-18-backend-chapter-generation-refactor-followup/implement.md`
- `docs/architecture/rust-strangler-refactor-plan-2026-05-17.zh-CN.md`

Behavior contract preserved:

- Existing candidate quality adapter input/output shapes are unchanged.
- Rust now resolves runtime quality profiles from explicit profile tokens or
  detected genre/style text.
- Rust now resolves stage-aware quality weight profiles with focus areas,
  focus labels, Chinese summary text, style profile, genre profiles, and
  quality preset.
- Story-repair guidance and quality-gate payloads now emit
  `adaptive_quality_profile`.
- Quality gate failed metrics now consume profile/stage/intent threshold
  adjustments before the existing runtime-pressure adjustments are applied.
- `build_volume_goal_completion_summary(...)` now consumes the same Rust
  `quality_weight_profile` owner and no longer maps `style_profile` from
  `story_focus` or `genre_profiles` from `character_focus`.

Cutover boundary:

- This retires another Python quality-domain function group from the active
  Rust single-generation quality path.
- This still does not retire all of `novel_quality_profile_service.py`;
  prompt-block and external-asset profile construction remain Python
  source-map material for a future larger package.
- It still does not retire the whole Python quality domain or default-enable
  the Rust candidate executor.
- Rollback remains the active Rust route's direct AI fallback plus frozen
  Python source maps.

Focused validation passed with:

- `cargo fmt --manifest-path "backend-rs/Cargo.toml"`
- `cargo test novel_quality_profile_service --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-candidate-route-gateway-smoke-owner" -- --nocapture`
  -> 3 passed
- `cargo test chapter_story_repair_quality_context_service --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-candidate-route-gateway-smoke-owner" -- --nocapture`
  -> 9 passed
- `cargo test chapter_single_generation_candidate_quality_service --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-candidate-route-gateway-smoke-owner" -- --nocapture`
  -> 7 passed
- `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-candidate-route-gateway-smoke-owner"`
  -> passed with existing unused/dead-code warnings
- `python backend/tools/run_strangler_gateway_smoke.py --validate-manifest-only`
  -> passed; manifest probe count remains 170

Next acceleration target:

1. Continue quality only if the next block moves the remaining prompt-block /
   external-asset profile package from `novel_quality_profile_service.py`;
   otherwise switch to the next `chapter_single_generation` whole-file owner
   gap.
2. Keep Python fallback shrink as a follow-up after Rust quality owner,
   enabled-path smoke, rollback, and deployment checks remain stable.

### 2026-06-08 chapter-single-generation quality prompt-block owner checkpoint

This round continued Package B quality ownership and moved the remaining
prompt-block / external-asset profile package from
`novel_quality_profile_service.py` into Rust. Unlike the prior adaptive profile
checkpoint, this one also connects the Rust owner to active Rust chapter
generation prompt params, so Rust templates now receive quality profile blocks
from a Rust service owner.

Python source / fallback map:

- `backend/app/services/quality_domain/novel_quality_profile_service.py`
  - source map for `NovelQualityAssetInput`
  - source map for `NovelQualityAssetSummary`
  - source map for `NovelQualityIgnoredAsset`
  - source map for `NovelQualityProfileBlock`
  - source map for `NovelQualityPromptBlocks`
  - source map for `NovelQualityRelaxationSnapshot`
  - source map for `NovelQualityProfile`
  - source map for `NovelQualityProfileService.build_profile(...)`
  - source map for `_sanitize_external_assets(...)`
  - source map for generation/checker/reviser/MCP/external-asset block builders
  - source map for `_build_policy(...)`
- `backend/app/services/prompt_service.py`
  - source map for `_build_quality_profile_context(...)`
  - source map for core `prompt_blocks` consumption during quality runtime
    block construction
- Python FastAPI route and Python candidate executor fallback shell remain
  frozen; no fallback shrink is counted in this checkpoint.

Updated Rust owners:

- `backend-rs/src/services/novel_quality_profile_service.rs`
- `backend-rs/src/services/chapter_generation_prompt_service.rs`
- `.trellis/spec/backend/quality-guidelines.md`
- `.trellis/tasks/05-18-backend-chapter-generation-refactor-followup/implement.md`
- `docs/architecture/rust-strangler-refactor-plan-2026-05-17.zh-CN.md`

Behavior contract preserved:

- Rust now builds a full quality profile JSON owner with `version`,
  `baseline_id`, `genre_profiles`, `style_profile`, `quality_dimensions`,
  `active_relaxations`, `external_assets`, `ignored_external_assets`, block
  objects, `prompt_blocks`, and `policy`.
- Rust external asset handling preserves summary-only behavior, title/source/
  usage/type clipping, raw-only/no-summary/limit/duplicate ignore reasons, and
  JSON string asset parsing.
- Rust prompt blocks now cover generation, checker, reviser, MCP guard, and
  external asset blocks.
- `chapter_generation_prompt_service.rs` now fills
  `quality_generation_block`, `quality_analysis_block`,
  `quality_checker_block`, `quality_reviser_block`,
  `quality_regeneration_block`, `quality_generation_protocol_block`,
  `quality_json_protocol_block`, `quality_mcp_guard_block`, `mcp_guard`, and
  `quality_external_assets_block` from the Rust quality profile owner.
- The previous simple raw external-asset rendering remains available as
  `quality_raw_external_assets_block` for diagnostics / future fallback audit,
  but templates now consume the owner-rendered summary-only block.

Cutover boundary:

- This is real active Rust prompt-path migration because Rust generation prompt
  params now consume the Rust quality profile prompt-block owner.
- It still does not retire Python `prompt_service.py`, the Python FastAPI
  route, the Python candidate executor fallback shell, or the full Python
  quality domain.
- Rollback remains the active Rust route's direct AI fallback plus frozen
  Python source maps.

Focused validation passed with:

- `cargo fmt --manifest-path "backend-rs/Cargo.toml"`
- `cargo test novel_quality_profile_service --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-candidate-route-gateway-smoke-owner" -- --nocapture`
  -> 5 passed
- `cargo test chapter_generation_prompt_service --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-candidate-route-gateway-smoke-owner" -- --nocapture`
  -> 12 passed
- `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-candidate-route-gateway-smoke-owner"`
  -> passed with existing unused/dead-code warnings
- `python backend/tools/run_strangler_gateway_smoke.py --validate-manifest-only`
  -> passed; manifest probe count remains 170

Next acceleration target:

1. Stay in Package B only if the next block moves another active
   `chapter_single_generation` whole-file owner, such as prepare/write/stream
   prompt-runtime materialization or quality-status cutover evidence.
2. Do not shrink Python fallback shells until the Rust prompt/quality owners,
   enabled-path smoke, rollback, and deployment checks remain stable.

### 2026-06-08 chapter-single-generation quality runtime contract owner checkpoint

This round stayed in Package B quality ownership and moved the chapter
generation branch of Python `PromptService._build_quality_runtime_blocks(...)`
and `_inject_quality_contract(...)` into the active Rust prompt path. The
previous checkpoint made Rust prompt params consume profile blocks; this
checkpoint makes Rust assemble and inject the runtime `<quality_contract>`
section itself.

Python source / fallback map:

- `backend/app/services/prompt_service.py`
  - source map for `QUALITY_RUNTIME_TRACKING_TAG`
  - source map for `QUALITY_PREFERENCE_SPECS`
  - source map for `QUALITY_PREFERENCE_ALIASES`
  - source map for `normalize_quality_preset(...)`
  - source map for `_split_quality_preference_note_items(...)`
  - source map for `build_quality_preference_block(...)`
  - source map for the chapter-generation branch of
    `_build_quality_runtime_blocks(...)`
  - source map for `_inject_quality_contract(...)`
- `backend/app/services/quality_domain/novel_quality_profile_service.py`
  remains the frozen source map for profile schema parity that is not yet
  retired from Python compatibility paths.
- Python FastAPI route, Python `prompt_service.py` compatibility behavior, and
  Python candidate executor fallback shell remain frozen; no fallback shrink is
  counted in this checkpoint.

Updated Rust owners:

- `backend-rs/src/services/chapter_generation_prompt_service.rs`
- `.trellis/spec/backend/quality-guidelines.md`
- `.trellis/tasks/05-18-backend-chapter-generation-refactor-followup/implement.md`
- `docs/architecture/rust-strangler-refactor-plan-2026-05-17.zh-CN.md`

Behavior contract preserved:

- Rust now builds `quality_preference_block` from quality preset aliases and
  up to four de-duplicated note items.
- Rust now owns the chapter-generation unified quality protocol block and JSON
  protocol block text instead of temporarily mapping those placeholders back to
  generation/checker prompt blocks.
- Rust now builds `quality_contract_block` from the active chapter generation
  quality block order: generation profile, creative/story focus, creation
  brief, quality preference, repair targets/diagnostics, protocol guard, MCP
  guard, and summary-only external assets.
- `build_prompt_with_provider_payload(...)` now injects the Rust-built
  `<quality_contract>` immediately after `</fusion_contract>` and skips
  duplicate injection if a custom rendered prompt already contains
  `<quality_contract>`.
- This is a real active Rust prompt-path migration because final rendered
  chapter prompts now receive the runtime quality contract from Rust, not only
  individual quality placeholders.

Cutover boundary:

- This narrows the Python `prompt_service.py` runtime-quality dependency for
  active Rust chapter generation.
- It still does not retire Python `prompt_service.py`, the Python FastAPI
  route, the Python candidate executor fallback shell, or the full Python
  quality domain.
- Rollback remains the active Rust route's direct AI fallback plus frozen
  Python source maps.

Focused validation passed with:

- `cargo fmt --manifest-path "backend-rs/Cargo.toml"`
- `cargo test chapter_generation_prompt_service --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-candidate-route-gateway-smoke-owner" -- --nocapture`
  -> 14 passed
- `cargo test novel_quality_profile_service --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-candidate-route-gateway-smoke-owner" -- --nocapture`
  -> 5 passed
- `cargo test chapter_single_generation_candidate_quality_service --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-candidate-route-gateway-smoke-owner" -- --nocapture`
  -> 7 passed
- `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-candidate-route-gateway-smoke-owner"`
  -> passed with existing unused/dead-code warnings
- `python backend/tools/run_strangler_gateway_smoke.py --validate-manifest-only`
  -> passed; manifest probe count remains 170

Next acceleration target:

1. Package B can now move from prompt-quality helper migration to a larger
   whole-file owner gap: single-generation prepare/write/stream runtime
   materialization, or quality-status cutover evidence.
2. Python fallback shrink should wait until the Rust prompt/quality owners,
   enabled-path smoke, rollback, and deployment checks remain stable.

### 2026-06-08 chapter-single-generation creative/story runtime block owner checkpoint

This round continued Package B prompt-runtime ownership as a whole function
group. It moved the active chapter-generation branch of Python
`prompt_service.py` creative mode, story focus, plot stage normalization, and
chapter `build_narrative_blueprint_block(...)` semantics into Rust. The
previous checkpoint made Rust assemble the runtime quality contract; this one
makes the creative/story runtime blocks inside that contract Rust-owned instead
of simple pass-through placeholders.

Python source / fallback map:

- `backend/app/services/prompt_service.py`
  - source map for `CREATIVE_MODE_SPECS`
  - source map for `CREATIVE_MODE_ALIASES`
  - source map for `STORY_FOCUS_SPECS`
  - source map for `STORY_FOCUS_ALIASES`
  - source map for `PLOT_STAGE_LABELS`
  - source map for `PLOT_STAGE_ALIASES`
  - source map for `normalize_creative_mode(...)`
  - source map for `normalize_story_focus(...)`
  - source map for `normalize_plot_stage(...)`
  - source map for `build_creative_mode_block(...)`
  - source map for `build_story_focus_block(...)`
  - source map for the chapter scene branch of
    `build_narrative_blueprint_block(...)`
- Python `prompt_service.py`, Python FastAPI routes, and Python candidate
  executor fallback shell remain frozen; no fallback deletion or route cutover
  is claimed in this checkpoint.

Updated Rust owner:

- `backend-rs/src/services/chapter_generation_prompt_service.rs`

Behavior contract now owned by Rust:

- Rust normalizes English and Chinese aliases for creative mode, story focus,
  and plot stage before building chapter prompt blocks.
- Rust builds rich chapter-scene `creative_mode_block` and `story_focus_block`
  from Rust-owned labels and bullet tables, replacing the old
  `build_optional_instruction_block(...)` pass-through behavior.
- Rust builds `narrative_blueprint_block` from creative mode, story focus, and
  plot stage priority beats / risks plus the chapter base beat budget.
- `QUALITY_CONTRACT_BLOCK_ORDER` now includes `narrative_blueprint_block`
  immediately after `story_focus_block`, so the final injected
  `<quality_contract>` carries the migrated story-runtime structure.
- This is active prompt-path migration: `build_prompt_params_with_provider_payload(...)`
  now materializes these blocks in Rust before the rendered prompt is injected.

Focused validation passed with:

- `cargo fmt --manifest-path "backend-rs/Cargo.toml"`
- `cargo test chapter_generation_prompt_service --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-candidate-route-gateway-smoke-owner" -- --nocapture`
  -> 15 passed
- `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-candidate-route-gateway-smoke-owner"`
  -> passed with existing unused/dead-code warnings
- `python backend/tools/run_strangler_gateway_smoke.py --validate-manifest-only`
  -> passed; readiness summary shows `rust = 127`, `python-fallback = 43`;
  deploy manifest remains 7 probes
- `git diff --check -- backend-rs/src/services/chapter_generation_prompt_service.rs .trellis/spec/backend/quality-guidelines.md .trellis/tasks/05-18-backend-chapter-generation-refactor-followup/implement.md docs/architecture/rust-strangler-refactor-plan-2026-05-17.zh-CN.md`
  -> passed except the existing CRLF-to-LF warning for the Chinese architecture
  document

Next acceleration target:

1. Do not continue by adding another single prompt helper seam. The next
   package should be a whole-file or whole-module migration such as
   single-generation prepare/write/stream runtime materialization.
2. If prompt migration continues, move a larger remaining card group from
   Python `prompt_service.py` and wire every new block into the Rust active
   contract in the same checkpoint.
3. Python fallback shrink still requires route parity, enabled-path smoke, and
   rollback evidence; this checkpoint intentionally keeps those boundaries
   unchanged.

### 2026-06-08 chapter-single-generation foundational story card owner checkpoint

This round continued the same Package B prompt-runtime owner, but moved a
larger card group instead of a single placeholder. It ports the chapter-scene
branches of Python `prompt_service.py` foundational story cards into the active
Rust prompt path:

- `build_story_objective_card_block(...)`
- `build_story_result_card_block(...)`
- `build_story_payoff_chain_card_block(...)`
- `build_story_rule_grounding_card_block(...)`

Python source / fallback map:

- `backend/app/services/prompt_service.py`
  - source map for the four chapter story card builders above
  - source map for Python's assignment order: creative mode overrides first,
    story focus second, plot stage last
- Python `prompt_service.py`, Python FastAPI routes, and Python candidate
  executor fallback shell remain frozen; no route or fallback deletion is
  claimed in this checkpoint.

Updated Rust owner:

- `backend-rs/src/services/chapter_generation_prompt_service.rs`

Behavior contract now owned by Rust:

- Rust now builds `story_objective_card_block`, `story_result_card_block`,
  `story_payoff_chain_card_block`, and `story_rule_grounding_card_block`
  directly from the active `creative_mode`, `story_focus`, and `plot_stage`.
- The four card builders preserve the Python override order, including cases
  where `plot_stage = climax` overwrites a prior conflict-focus line.
- `QUALITY_CONTRACT_BLOCK_ORDER` now includes this foundational card group
  after `quality_preference_block`, matching the generation contract ordering
  before repair and protocol guards.
- Blank or unknown story runtime inputs keep these four card blocks empty,
  avoiding misleading raw pass-through prompts.
- This is active prompt-path migration because
  `build_prompt_params_with_provider_payload(...)` materializes the cards
  before `quality_contract_block` is assembled and injected.

Focused validation passed with:

- `cargo fmt --manifest-path "backend-rs/Cargo.toml"`
- `cargo test chapter_generation_prompt_service --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-candidate-route-gateway-smoke-owner" -- --nocapture`
  -> 15 passed
- `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-candidate-route-gateway-smoke-owner"`
  -> passed with existing unused/dead-code warnings
- `python backend/tools/run_strangler_gateway_smoke.py --validate-manifest-only`
  -> passed; readiness summary remains `rust = 127`, `python-fallback = 43`;
  deploy manifest remains 7 probes
- `git diff --check -- backend-rs/src/services/chapter_generation_prompt_service.rs .trellis/spec/backend/quality-guidelines.md .trellis/tasks/05-18-backend-chapter-generation-refactor-followup/implement.md docs/architecture/rust-strangler-refactor-plan-2026-05-17.zh-CN.md`
  -> passed except the existing CRLF-to-LF warning for the Chinese architecture
  document

Next acceleration target:

1. Continue prompt runtime only by moving another full card group, such as
   information/emotion/action/rendering/control cards, and wire it into the
   active Rust contract in the same round.
2. Higher-impact follow-up remains a whole-file `chapter_single_generation`
   prepare/write/stream runtime materialization package.
3. Do not shrink Python fallback shells until route parity, enabled-path smoke,
   and rollback evidence are explicit.

### 2026-06-08 chapter-single-generation information/emotion/action card owner checkpoint

This round continued Package B prompt-runtime ownership as another whole
function group. It ports the chapter-scene branches of Python
`prompt_service.py` follow-up story cards into the active Rust prompt path:

- `build_story_information_release_card_block(...)`
- `build_story_emotion_landing_card_block(...)`
- `build_story_action_rendering_card_block(...)`
- `build_story_summary_tone_control_card_block(...)`

Python source / fallback map:

- `backend/app/services/prompt_service.py`
  - source map for the four chapter follow-up story card builders above
  - source map for Python's assignment order: creative mode overrides first,
    story focus second, plot stage last
- Python `prompt_service.py`, Python FastAPI routes, and Python candidate
  executor fallback shell remain frozen; no route or fallback deletion is
  claimed in this checkpoint.

Updated Rust owner:

- `backend-rs/src/services/chapter_generation_prompt_service.rs`

Behavior contract now owned by Rust:

- Rust now builds `story_information_release_card_block`,
  `story_emotion_landing_card_block`, `story_action_rendering_card_block`,
  and `story_summary_tone_control_card_block` directly from active
  `creative_mode`, `story_focus`, and `plot_stage`.
- The four card builders preserve Python's override order, including
  plot-stage climax overrides for explanation density, emotion landing, action
  visibility, and summary-tone suppression.
- `QUALITY_CONTRACT_BLOCK_ORDER` now places these cards immediately after
  `story_rule_grounding_card_block`, matching the generation contract order
  before repair and protocol guards.
- Blank or unknown story runtime inputs keep these four card blocks empty,
  avoiding misleading raw pass-through prompt text.
- This is active prompt-path migration because
  `build_prompt_params_with_provider_payload(...)` materializes the cards
  before `quality_contract_block` is assembled and injected.

Focused validation passed with:

- `cargo fmt --manifest-path "backend-rs/Cargo.toml"`
- `cargo test chapter_generation_prompt_service --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-candidate-route-gateway-smoke-owner" -- --nocapture`
  -> 15 passed
- `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-candidate-route-gateway-smoke-owner"`
  -> passed with existing unused/dead-code warnings
- `python backend/tools/run_strangler_gateway_smoke.py --validate-manifest-only`
  -> passed; readiness summary remains `rust = 127`, `python-fallback = 43`;
  deploy manifest remains 7 probes

Next acceleration target:

1. Prompt runtime can continue only by moving the remaining card/control
   groups as a whole and wiring them into the active Rust contract in the same
   round.
2. Higher-impact follow-up remains a whole-file `chapter_single_generation`
   prepare/write/stream runtime materialization package.
3. Python fallback shrink still requires route parity, enabled-path smoke, and
   rollback evidence.

### 2026-06-08 chapter-single-generation control/voice/opening card owner checkpoint

This round continued Package B prompt-runtime ownership as a contiguous
function group in the Python generation contract order. It ports the
chapter-scene branches of Python `prompt_service.py` control, viewpoint,
dialogue, and opening-hook cards into the active Rust prompt path:

- `build_story_repetition_control_card_block(...)`
- `build_story_viewpoint_discipline_card_block(...)`
- `build_story_dialogue_advancement_card_block(...)`
- `build_story_opening_hook_card_block(...)`

Python source / fallback map:

- `backend/app/services/prompt_service.py`
  - source map for the four chapter control/voice/opening card builders above
  - source map for Python's assignment order: creative mode overrides first,
    story focus second, plot stage last
- Python `prompt_service.py`, Python FastAPI routes, and Python candidate
  executor fallback shell remain frozen; no route or fallback deletion is
  claimed in this checkpoint.

Updated Rust owner:

- `backend-rs/src/services/chapter_generation_prompt_service.rs`

Behavior contract now owned by Rust:

- Rust now builds `story_repetition_control_card_block`,
  `story_viewpoint_discipline_card_block`,
  `story_dialogue_advancement_card_block`, and
  `story_opening_hook_card_block` directly from active `creative_mode`,
  `story_focus`, and `plot_stage`.
- The four card builders preserve Python's override order, including
  plot-stage climax overrides for repetition compression, viewpoint stability,
  dialogue sharpness, and opening pressure.
- `QUALITY_CONTRACT_BLOCK_ORDER` now places these cards immediately after
  `story_summary_tone_control_card_block`, matching the generation contract
  order before repair and protocol guards.
- Blank or unknown story runtime inputs keep these four card blocks empty,
  avoiding misleading raw pass-through prompt text.
- This is active prompt-path migration because
  `build_prompt_params_with_provider_payload(...)` materializes the cards
  before `quality_contract_block` is assembled and injected.

Focused validation passed with:

- `cargo fmt --manifest-path "backend-rs/Cargo.toml"`
- `cargo test chapter_generation_prompt_service --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-candidate-route-gateway-smoke-owner" -- --nocapture`
  -> 15 passed
- `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-candidate-route-gateway-smoke-owner"`
  -> passed with existing unused/dead-code warnings
- `python backend/tools/run_strangler_gateway_smoke.py --validate-manifest-only`
  -> passed; readiness summary remains `rust = 127`, `python-fallback = 43`;
  deploy manifest remains 7 probes

Next acceleration target:

1. Continue prompt runtime only by moving another contiguous remaining card
   group, likely opening-adjacent scene/acceptance/cliffhanger/character cards,
   and wire it into the active Rust contract in the same round.
2. Higher-impact follow-up remains a whole-file `chapter_single_generation`
   prepare/write/stream runtime materialization package.
3. Python fallback shrink still requires route parity, enabled-path smoke, and
   rollback evidence.

### 2026-06-08 chapter-single-generation scene/acceptance/cliffhanger/character card owner checkpoint

This round continued Package B prompt-runtime ownership as the next contiguous
tail-card function group in Python's generation contract order. It ports the
chapter-scene branches of Python `prompt_service.py` execution, scene,
repetition-risk, acceptance, cliffhanger, and character-arc cards into the
active Rust prompt path:

- `build_story_execution_checklist_block(...)`
- `build_story_scene_anchor_card_block(...)`
- `build_story_scene_density_card_block(...)`
- `build_story_repetition_risk_block(...)`
- `build_story_acceptance_card_block(...)`
- `build_story_cliffhanger_card_block(...)`
- `build_story_character_arc_card_block(...)`

Python source / fallback map:

- `backend/app/services/prompt_service.py`
  - source map for the seven chapter tail-card builders above
  - source map for Python's assignment order: creative mode overrides first,
    story focus second, plot stage last
- Python `prompt_service.py`, Python FastAPI routes, and Python candidate
  executor fallback shell remain compatibility/source-map boundaries; no route
  or fallback deletion is claimed in this checkpoint.

Updated Rust owner:

- `backend-rs/src/services/chapter_generation_prompt_service.rs`

Behavior contract now owned by Rust:

- Rust now builds `story_execution_checklist_block`,
  `story_scene_anchor_card_block`, `story_scene_density_card_block`,
  `story_repetition_risk_block`, `story_acceptance_card_block`,
  `story_cliffhanger_card_block`, and `story_character_arc_card_block`
  directly from active `creative_mode`, `story_focus`, and `plot_stage`.
- The seven card builders preserve Python's override order, including
  plot-stage climax overrides for fast collision entry, scene density,
  repetition risk, acceptance checks, cliffhanger aftershock, and character
  bottom-line pressure.
- `QUALITY_CONTRACT_BLOCK_ORDER` now places these cards immediately after
  `story_repair_diagnostic_block`, matching the Python generation contract
  tail order before generation protocol / creative / anti-AI guards.
- Blank or unknown story runtime inputs keep these seven card blocks empty,
  avoiding misleading raw pass-through prompt text.
- `build_prompt_params_with_provider_payload(...)` materializes the cards
  before `quality_contract_block` is assembled, so this is active prompt-path
  migration rather than unused Rust helper code.

Focused validation passed with:

- `cargo fmt --manifest-path "backend-rs/Cargo.toml"`
- `cargo test chapter_generation_prompt_service --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-candidate-route-gateway-smoke-owner" -- --nocapture`
  -> 15 passed
- `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-candidate-route-gateway-smoke-owner"`
  -> passed with existing unused/dead-code warnings
- `python backend/tools/run_strangler_gateway_smoke.py --validate-manifest-only`
  -> passed; readiness summary remains `rust = 127`, `python-fallback = 43`;
  deploy manifest remains 7 probes

Next acceleration target:

1. Prompt runtime has now moved most chapter story-runtime card builders into
   Rust active prompt ownership. Continue here only if long-term-goal, pacing,
   ledger, or remaining protocol blocks are still missing from active Rust
   params.
2. Higher-impact follow-up should shift to a whole-file / whole-module
   `chapter_single_generation` prepare/write/stream runtime materialization
   package, because that produces visible Python fallback shrinkage instead of
   more prompt-only owner lift.
3. Python fallback shrink still requires route parity, enabled-path smoke, and
   rollback evidence.

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

### 2026-06-08 chapter_single_generation quality-gate follow-up persistence owner checkpoint

This round moved a real `chapter_single_generation` runtime/materialization
slice instead of another prompt-only seam. The Rust owner now projects
quality-gate follow-up state through generated-result persistence, history
metadata, and stream result payloads so the active Rust path no longer hard
codes the single-generation success case as always-applied chapter content.

Python source / parity map:

- `backend/app/services/chapter_generation/stream/candidate_service.py`
- `backend/app/services/chapter_generation/stream/finalize_service.py`
- `backend/app/services/chapter_generation/stream/wiring_service.py`
- `backend/app/services/batch_generation_chapter_persistence_service.py`
- `backend/app/schemas/generation_payload.py`

Updated Rust owners:

- `backend-rs/src/services/chapter_generation_runtime_service.rs`
- `backend-rs/src/services/chapter_single_generation_stream_workflow_service.rs`

Behavior now owned by Rust:

1. `GeneratedChapterResult` now carries Rust-owned
   `saved_word_count`, `chapter_status`, `content_applied`,
   `provisional_draft_saved`, `attempt_state`, `quality_metrics`,
   `quality_gate_action`, `quality_gate_message`, and `candidate_draft`
   fields instead of leaving the stream layer to infer everything from a
   post-hoc analysis payload.
2. Candidate-route active generation now derives quality-gate follow-up
   semantics directly from Rust candidate output:
   `continue -> applied/completed`,
   `retry -> draft/provisional draft saved`,
   `manual_review -> follow-up draft without content application`.
3. Rust runtime history payloads now preserve
   `content_applied` and `attempt_state` as real owner data rather than
   always writing `content_applied=true` and
   `attempt_state=generated_from_runtime`.
4. Rust single-generation persistence now creates and stores a
   `chapter_draft_attempts` record for non-applied quality-gate follow-up
   results, and the stream result payload now exposes `candidate_draft`
   when Rust owns a retry/manual-review candidate outcome.
5. Stream success projection now consumes the runtime result fields instead of
   hard-coding `chapter_status="draft"` and `content_applied=true`.
6. Rust still does not claim Python fallback retirement in this checkpoint.
   Python route/fallback shells remain the rollback boundary and source map.

Focused validation passed with:

- `cargo fmt --manifest-path "backend-rs/Cargo.toml"`
- `cargo test chapter_single_generation_stream_workflow_service --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-candidate-route-gateway-smoke-owner" -- --nocapture`
  -> 21 passed
- `cargo test chapter_generation_runtime_service --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-candidate-route-gateway-smoke-owner" -- --nocapture`
  -> 16 passed
- `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-candidate-route-gateway-smoke-owner"`
  -> passed with existing unused/dead-code warnings
- `python backend/tools/run_strangler_gateway_smoke.py --validate-manifest-only`
  -> passed; readiness summary remains `rust = 127`, `python-fallback = 43`;
  deploy manifest remains 7 probes

Next acceleration target:

1. Continue `chapter_single_generation` as a whole-file / whole-module package,
   not another prompt seam. Highest-value remaining gaps are prepare/write
   runtime materialization and the remaining quality-follow-up lifecycle around
   background scheduling / checkpoint persistence.
2. If the next round stays in this package, move the whole file/function group
   that still depends on Python-shaped fallback assumptions, rather than adding
   more message-only wrappers.
3. Python fallback shrink still requires route parity, enabled-path smoke, and
   rollback evidence before any retirement claim.

### 2026-06-08 chapter_single_generation active candidate gateway observability checkpoint

This round continued the same `chapter_single_generation` runtime /
materialization package and tightened the active candidate-route gateway
contract. The Rust active path already calls the candidate route gateway; this
checkpoint makes that execution decision observable through generated-result
persistence, history metadata, and stream result payloads.

Python source / parity map:

- `backend/app/services/chapter_generation/stream/candidate_service.py`
- `backend/app/services/chapter_generation/stream/finalize_service.py`
- `backend/app/services/chapter_generation/stream/wiring_service.py`
- `backend/app/services/chapter_candidate_executor_service.py`
- `backend/app/services/chapter_candidate_executor_wiring_service.py`

Updated Rust owners:

- `backend-rs/src/services/chapter_generation_runtime_service.rs`
- `backend-rs/src/services/chapter_single_generation_stream_workflow_service.rs`

Behavior now owned by Rust:

1. `GeneratedChapterResult` now carries
   `candidate_gateway_metadata`, preserving active candidate gateway
   execution evidence beside content, quality-gate, and draft state.
2. `generate_and_persist_with_candidate_route_gateway(...)` now records the
   route-gateway production adapter decision after the candidate output is
   materialized:
   `execution_path`, `fallback_applied`, `fallback_reason`,
   `rollback_boundary`, and `rust_error`.
3. Generated chapter history payloads now persist `candidate_gateway` metadata
   directly from the runtime result, and
   `update_latest_generated_chapter_history_quality_metrics(...)` preserves
   that existing metadata when later analysis rewrites quality metrics.
4. Single-generation stream result payloads now expose the same
   `candidate_gateway` object so enabled Rust executor vs direct fallback can
   be inspected from the active SSE result without scraping logs.
5. This is active-path materialization, not fallback retirement. Python route
   and candidate fallback shells remain the rollback boundary until route
   parity, enabled-path business smoke, and removal/repointing evidence exist.

Focused validation passed with:

- `cargo fmt --manifest-path "backend-rs/Cargo.toml"`
- `cargo test chapter_generation_runtime_service --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-candidate-route-gateway-smoke-owner" -- --nocapture`
  -> 16 passed
- `cargo test chapter_single_generation_stream_workflow_service --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-candidate-route-gateway-smoke-owner" -- --nocapture`
  -> 21 passed
- `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-candidate-route-gateway-smoke-owner"`
  -> passed with existing unused/dead-code warnings
- `python backend/tools/run_strangler_gateway_smoke.py --validate-manifest-only`
  -> passed; readiness summary remains `rust = 127`, `python-fallback = 43`;
  deploy manifest remains 7 probes

Next acceleration target:

1. Continue this package by moving the remaining prepare/write/runtime-state
   file groups that still carry Python-shaped fallback assumptions.
2. The next high-value cutover evidence is a stronger enabled-path business
   smoke for `chapter_single_generation`, because the active result payload
   now exposes whether the Rust executor or fallback path was used.
3. Do not report Python fallback retirement until the compatibility shell is
   removed or repointed and rollback evidence is updated.

### 2026-06-08 chapter_single_generation background terminal-state owner checkpoint

This round stayed on the same `chapter_single_generation` whole-module package
and continued the real Rust owner chain instead of returning to Python shell
cleanup. The focus moved into the background lifecycle boundary: prepare-time
task seed ownership plus runtime terminal-state persistence for manual-review,
retry, and hard-failure outcomes.

Python source / parity map:

- `backend/app/services/chapter_generation/stream/finalize_service.py`
- `backend/app/services/chapter_generation/stream/wiring_service.py`
- `backend/app/services/batch_generation_chapter_persistence_service.py`
- `backend/app/schemas/generation_payload.py`
- `backend/app/services/compat/chapter_generation_route_compat_service.py`

Updated Rust owners:

- `backend-rs/src/services/chapter_single_generation_prepare_service.rs`
- `backend-rs/src/services/chapter_single_generation_runtime_state_service.rs`
- validated against neighboring production consumers:
  - `backend-rs/src/services/chapter_single_generation_write_workflow_service.rs`
  - `backend-rs/src/services/chapter_single_generation_stream_workflow_service.rs`

Behavior now owned by Rust:

1. Single-chapter background task persistence seed now preserves the real
   runtime owner contract instead of writing a fake background default:
   `enable_analysis` is derived from
   `runtime_input.execution_input.compat_options.enable_analysis()`, and
   `max_retries` is now seeded as the Rust-owned single-generation background
   retry budget instead of `0`.
2. Runtime terminal persistence for background single-generation tasks now
   resolves three explicit Rust owner outcomes:
   `manual_review`, `retry`, and `error`, instead of collapsing everything
   into an undifferentiated failed task shell.
3. Failed single-generation tasks now persist `failed_chapters` entries from
   the Rust runtime owner, including chapter identity, retry count, error
   label, and quality-gate terminal fields when the failure is a follow-up
   review or repair decision.
4. Quality-gate terminal checkpoint payload and failed checkpoint payload are
   now merged on the Rust side before persistence, so a failed lifecycle write
   no longer overwrites the richer quality-gate snapshot fields that were
   already materialized.
5. Manual-review label and retry label resolution now reuse the shared
   quality-context decision path instead of keeping a second single-file
   ad hoc interpretation branch in the single-generation runtime file.
6. This is still active owner tightening, not Python fallback retirement.
   Python route/compat shells remain the rollback boundary until route parity,
   enabled-path smoke, and fallback repoint/removal evidence are updated.

Focused validation passed with:

- `cargo test chapter_single_generation_prepare_service --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-candidate-route-gateway-smoke-owner" -- --nocapture`
  -> 28 passed
- `cargo test chapter_single_generation_runtime_state_service --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-candidate-route-gateway-smoke-owner" -- --nocapture`
  -> 14 passed
- `cargo test chapter_single_generation_write_workflow_service --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-candidate-route-gateway-smoke-owner" -- --nocapture`
  -> 4 passed
- `cargo test chapter_single_generation_stream_workflow_service --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-candidate-route-gateway-smoke-owner" -- --nocapture`
  -> 21 passed
- `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-candidate-route-gateway-smoke-owner"`
  -> passed with existing unused/dead-code warnings
- `python backend/tools/run_strangler_gateway_smoke.py --validate-manifest-only`
  -> passed; readiness summary remains `rust = 127`, `python-fallback = 43`;
  deploy manifest remains 7 probes

Next acceleration target:

1. Keep moving `chapter_single_generation` as a whole-file / whole-module
   package. The next highest-value gap is still the remaining prepare/write/
   runtime-state file group that carries Python-shaped fallback assumptions in
   restore, existing-background read state, or route-parity edges.
2. Do not count further helper relocation as primary migration progress unless
   it tightens the active Rust owner, removes a Python dependency, or improves
   smoke/rollback evidence for this package.
3. The next non-code milestone for this package is stronger enabled-path
   business smoke, because single-generation active payloads and background
   terminal states now expose enough Rust-owned lifecycle evidence to probe
   the path meaningfully.

### 2026-06-08 chapter_single_generation stream-entry whole-file collapse checkpoint

This round stayed on the same `chapter_single_generation` whole-module package
and removed one remaining fake owner hop on the active stream lane. The former
`chapter_single_generation_stream_entry_service.rs` no longer exists as a
separate module; its prepare-and-spawn public entry now belongs directly to
the stream workflow owner.

Python source / parity map:

- `backend/app/api/chapter_generation_routes.py`
- `backend/app/services/chapter_generation/stream/wiring_service.py`
- `backend/app/services/compat/chapter_generation_route_compat_service.py`

Updated Rust owners:

- `backend-rs/src/services/chapter_single_generation_stream_workflow_service.rs`
- `backend-rs/src/api/chapter_generation_routes.rs`
- deleted:
  `backend-rs/src/services/chapter_single_generation_stream_entry_service.rs`

Behavior now owned by Rust:

1. The single-generation stream public entry is now a direct public owner
   inside `chapter_single_generation_stream_workflow_service.rs`:
   route payload -> restored runtime launch preparation -> stream lifecycle
   spawn all belong to the same file-level owner chain.
2. `chapter_generation_routes.rs` now imports
   `create_owned_single_generation_stream(...)` directly from the workflow
   owner instead of routing through a separate entry-shell module.
3. The former stream-entry file was removed because it no longer added
   validation, transport shaping, branch selection, or error translation; it
   only replayed prepare -> lifecycle spawn.
4. The route-level error boundary test for missing chapters moved with the
   production owner, so stream request preparation and spawn behavior remain
   verifiable after the file collapse.
5. This is active Rust owner tightening, not Python fallback retirement.
   Python route/compat shells remain the rollback boundary until parity and
   enabled-path smoke move forward.

Focused validation passed with:

- `cargo test chapter_single_generation_stream_workflow_service --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-candidate-route-gateway-smoke-owner" -- --nocapture`
  -> 22 passed
- `cargo test chapter_generation_routes --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-candidate-route-gateway-smoke-owner" -- --nocapture`
  -> 3 passed
- `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-candidate-route-gateway-smoke-owner"`
  -> passed with existing unused/dead-code warnings

Next acceleration target:

1. Keep moving `chapter_single_generation` as a whole-file / whole-module
   package. The next highest-value gaps are now
   `existing_background_query + write + prepare/runtime-state` interactions
   that still reopen snapshot/recovery/read-state semantics across neighboring
   files.
2. Do not recreate another stream entry shell; future progress on this lane
   should remove real owner duplication, not reintroduce indirection.

### 2026-06-08 chapter_single_generation existing-background whole-file collapse checkpoint

This round stayed on the same `chapter_single_generation` whole-module package
and removed one more single-consumer file shell on the background lane. The
former `chapter_single_generation_existing_background_query_service.rs` no
longer exists as a separate module; its task query, recovery, snapshot, read
state, and quality-context assembly now belong directly to the write workflow
owner.

Python source / parity map:

- `backend/app/api/chapter_generation_routes.py`
- `backend/app/services/chapter_generation/stream/wiring_service.py`
- `backend/app/services/compat/chapter_generation_route_compat_service.py`
- `backend/app/schemas/generation_payload.py`

Updated Rust owners:

- `backend-rs/src/services/chapter_single_generation_write_workflow_service.rs`
- deleted:
  `backend-rs/src/services/chapter_single_generation_existing_background_query_service.rs`

Behavior now owned by Rust:

1. Single-generation existing-background detection now belongs directly to the
   write workflow owner:
   active task query -> task recovery -> snapshot map load -> read-state
   projection -> payload assembly all live in one file-level owner chain.
2. The former existing-background query file was removed because it no longer
   had an independent consumer or error boundary; it was only serving the
   write workflow lane.
3. Quality runtime context, quality history fields, and
   `active_story_repair_payload` projection for existing background payloads
   now stay beside the write-lane owner that decides whether to return an
   existing task payload or create a new launch.
4. The existing-background payload tests moved with the production owner, so
   task matching, read-state projection, and richer quality payload contracts
   remain verifiable after the file collapse.
5. This is active Rust owner tightening, not Python fallback retirement.
   Python route/compat shells remain the rollback boundary until route parity
   and enabled-path smoke advance.

Focused validation passed with:

- `cargo test chapter_single_generation_write_workflow_service --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-candidate-route-gateway-smoke-owner" -- --nocapture`
  -> 10 passed
- `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-candidate-route-gateway-smoke-owner"`
  -> passed with existing unused/dead-code warnings

Next acceleration target:

1. Keep moving `chapter_single_generation` as a whole-file / whole-module
   package. The next high-value gaps are now the remaining
   `prepare + runtime_state + write` neighboring assumptions that still reopen
   restored runtime, checkpoint, or route-parity semantics across file
   boundaries.
2. Do not recreate a standalone existing-background query shell; future
   progress should keep removing real duplication on the active owner path.

### 2026-06-08 chapter_single_generation background-launch owner tightening checkpoint

This round stayed on the same `chapter_single_generation` whole-module package
and kept moving real Rust ownership on the background lane instead of returning
to Python shell cleanup. The focus was the remaining background launch semantics
that still lived in `chapter_single_generation_prepare_service.rs` even though
the restored runtime / startup snapshot chain already belonged to
`chapter_single_generation_runtime_restore_service.rs`.

Python source / parity map:

- `backend/app/api/chapter_generation_routes.py`
- `backend/app/services/chapter_generation/stream/wiring_service.py`
- `backend/app/services/compat/chapter_generation_route_compat_service.py`
- `backend/app/schemas/generation_payload.py`

Updated Rust owners:

- `backend-rs/src/services/chapter_single_generation_runtime_restore_service.rs`
- `backend-rs/src/services/chapter_single_generation_prepare_service.rs`
- validated neighboring consumers:
  - `backend-rs/src/services/chapter_single_generation_write_workflow_service.rs`
  - `backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs`

Behavior now owned by Rust:

1. Single-generation background launch seed ownership now belongs to the
   restored-launch owner chain instead of the request/target preparation file:
   `SingleGenerationTaskPersistenceSeed`, pending checkpoint construction, and
   background task `ActiveModel` assembly moved into
   `chapter_single_generation_runtime_restore_service.rs`.
2. Background create-response compatibility payload construction now also
   belongs to the restored-launch owner, beside startup snapshot restoration
   and runtime launch materialization, instead of reopening that contract in
   `prepare_service`.
3. `chapter_single_generation_prepare_service.rs` is narrower again: it now
   keeps request normalization, route payload bounds, chapter target loading,
   and execution-config preparation, while background launch persistence
   semantics stay on the restored runtime owner path.
4. This is still active Rust owner tightening, not Python fallback retirement.
   Python route/compat shells remain the rollback boundary until route parity,
   enabled-path smoke, and fallback repoint/removal evidence advance.

Focused validation passed with:

- `cargo fmt --manifest-path "backend-rs/Cargo.toml"`
- `cargo test chapter_single_generation_runtime_restore_service --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-candidate-route-gateway-smoke-owner" -- --nocapture`
  -> 8 passed
- `cargo test chapter_single_generation_write_workflow_service --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-candidate-route-gateway-smoke-owner" -- --nocapture`
  -> 10 passed
- `cargo test chapter_batch_generation_runtime_state_service --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-candidate-route-gateway-smoke-owner" -- --nocapture`
  -> 109 passed
- `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "C:/codex-targets/mumunovel-candidate-route-gateway-smoke-owner"`
  -> passed with existing unused/dead-code warnings

Next acceleration target:

1. Keep moving `chapter_single_generation` as a whole-file / whole-module
   package. The next highest-value gap is no longer background launch seed
   ownership; it is the remaining `prepare + runtime_state + write` overlap
   where restored runtime / read-state assumptions still reopen neighboring
   contracts.
2. Prefer deleting or collapsing real single-consumer seams over relocating
   isolated helpers. A helper move only counts as progress when it tightens the
   active Rust owner chain, shrinks a fallback shell, or improves rollback /
   smoke evidence.
