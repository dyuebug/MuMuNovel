# Chapter Loop Durable Adapter Research

## Scope and non-negotiable constraints

`NovelAutopilotStepType` already reserves `ChapterGenerate`、`ChapterAnalyze` and
`ChapterRepair` (`backend-rs/src/services/novel_autopilot/types.rs:164-182`), but the
current chapter runtime was designed around a single-generation/background-task lifecycle.
The durable implementation must keep that lifecycle separate from the new
`novel_book_autopilot` child task:

- A chapter is processed in number order.  It reuses the existing Generation Contract,
  Story Packet, role model policy, candidate executor and quality formula; it does **not**
  create a second chapter store.
- Each durable step represents one bounded operation: generate one draft, analyse one
  accepted draft, or make one targeted repair.  The Router, not a `Future`, decides
  whether another attempt is allowed.
- The existing R7 `novel_autopilot` remains `NonResumable`; this design only targets the
  durable `novel_book_autopilot` task type.
- Never call an API route or localhost HTTP endpoint from the adapter.  Extract/re-export
  typed service functions from the current Rust owners instead.
- Run/step payloads persist only safe metadata: result/input digests, provider/model,
  actual-or-estimated usage, attempt number and a compact quality decision.  They must not
  persist prompts, provider reasoning/thinking, or raw provider responses.  The accepted
  normalized narrative itself belongs in `chapters.content`, not in a second durable blob.

The PRD requires a single existing chapter service/store, a per-chapter quality decision
of `accept` / `auto_repair` / `retry` / `manual_review`, bounded attempts, and derived
state updates only after acceptance (`prd.md:47-54`).  It also requires checks before a
model call and after a commit, with cancelled or expired results forbidden from committing
(`prd.md:56-68`).

## Existing reusable owners

### Generation contract, context and candidate execution

| Reusable owner | What it provides | Durable use |
| --- | --- | --- |
| `load_generation_context` | Ownership-checked chapter/project load, previous-chapter context, continuity ledger and a single-chapter Story Packet | Use for the initial business snapshot and Story Packet preparation. It is read-only at the chapter business layer. `runtime_execution_owner.rs:314-346` |
| `build_chapter_generation_intent_overrides` + Generation Contract helpers | Stable `ChapterGenerate` intent over the Story Packet | Preserve the contract digest for the attempt; do not store the prompt. `runtime_execution_owner.rs:490-608` |
| `execute_single_generation_candidate_runtime_tracked` | Prompt construction, candidate-route execution, candidate quality processing and an optional execution trace; it returns `(prompt, GeneratedChapterResult, trace)` without itself writing a chapter | This is the closest current generation-only execution boundary. Wrap it in a new typed service that discards the returned prompt before the adapter result is built. `candidate_runtime_owner.rs:425-444`, `:446-566` |
| `build_single_generation_runtime_generated_result_from_candidate` | Normalizes/sanitizes the winner and projects quality/lifecycle fields (`content_applied`, provisional draft, action, metrics) | Convert this to a safe typed `ChapterGeneratedDraft`; preserve normalized narrative only for the repository commit. `candidate_runtime_owner.rs:320-348` |
| `build_single_generation_quality_runtime_context`, `compute_single_generation_story_quality_metrics`, `resolve_single_generation_quality_gate_plan` | Existing writer-side quality metric and gate formula | Reuse unchanged; map only the final plan to the durable decision enum. `single_generation_candidate_quality_owner.rs:73-162`, `:252-385` |
| `build_chapter_story_packet_contract`, `build_chapter_review_contract`, `build_chapter_repair_contract` | Same Story Packet projected to generate/review/repair intents | Capture its digest at load time; rebuild from the authoritative business state for every new attempt. `chapter_story_packet_owner.rs:43-199` |

`GeneratedChapterResult` already includes chapter identity, normalized content, word count,
chapter status, lifecycle flags, quality metrics/action/message, candidate draft and safe
candidate gateway metadata (`chapter_generation_runtime_service.rs:53-69`).  It is a good
internal conversion source but not a durable DTO as-is: it carries candidate JSON and must
be minimized before persistence.

### Analysis and repair primitives

The analysis runtime already builds a ChapterReview Story Packet, formats the
`PLOT_ANALYSIS` template, resolves the reviewer role policy and calls
`AIService::generate_text_tracked` (`trigger_runtime_owner.rs:128-176`).  Its current
lowest model-call functions are private:

- `build_chapter_analysis_prompt(...)`;
- `execute_chapter_review_prompt(...) -> ExecutedChapterReview { content, audit }`;
- `clean_json_response(...)` + JSON parse in `execute_and_persist_chapter_review(...)`.

Extract a `chapter_analysis_generation_service` with a public-crate typed function that
performs exactly those first three operations and returns parsed analysis plus the safe
execution audit.  It must stop **before** `persist_chapter_analysis_result`.

For repair, the lowest suitable owner is
`execute_targeted_final_repair_pass_workflow(...)`, which takes an explicit request,
winner/candidate list and injectable model-output dependency, then produces the selected
candidate and an optional deferred repair seed.  It has no database parameter and is
therefore reusable after a thin typed facade builds its dependencies
(`chapter_candidate_targeted_final_repair_service.rs:161-250`, `:297-395`).  Reuse the
existing repair contract constructed from the same Story Packet; do not build an unrelated
"rewrite chapter" prompt.

## Non-reusable / side-effecting paths

The following functions are useful behavior references, but must **not** be called by a
Durable adapter as its commit operation:

1. `generate_and_persist_chapter_content_with_candidate_route_gateway(...)` and its batch /
   single variants load context **and persist** a generated result
   (`runtime_execution_owner.rs:626-648`, `:693-730`).  They bypass durable run/epoch/step
   fencing.
2. `persist_single_generation_generated_result_with_contract_and_audit(...)` starts a
   transaction and writes `chapters.content/status/word_count`, a `generation_history`
   row, and possibly candidate draft-attempt records.  It accepts and stores a `prompt`,
   so it violates the durable safe-payload boundary if reused directly
   (`chapter_generation_history_persistence_service/persistence_owner.rs:240-280`; the
   history model sets `prompt` at `:179-203`).
3. `trigger_chapter_analysis_write_workflow(...)` creates an analysis task and spawns a
   background future (`trigger_runtime_owner.rs:291-335`); it is inappropriate beneath the
   durable background task.
4. `analyze_chapter_now*` and `analyze_generated_chapter_follow_up` call
   `execute_and_persist_chapter_review` (`trigger_runtime_owner.rs:337-410`).  The latter
   inserts plot analysis, refreshes memories and foreshadows, synchronizes character and
   organization state, patches history quality data and completes the legacy analysis task
   (`persistence_owner.rs:19-25`, `:134-171`).  Those writes need one durable-owned,
   fenced transaction instead.
5. Batch runtime cancellation/checkpoint and SSE owners own `batch_generation_tasks` plus
   their snapshots.  They are not a substitute for a durable Run's child-task state; the
   batch cancellation code is only a cooperative-cancellation pattern reference.

## Minimal typed DTOs

The facade types should use owned typed fields rather than `serde_json::Value` at the
adapter/repository boundary.  JSON is permitted internally while calling existing owners.

```rust
struct ChapterBusinessSnapshot {
    project_id: String,
    chapter_id: String,
    chapter_number: i32,
    outline_id: Option<String>,
    title_digest: String,
    content_digest: String,
    summary_digest: String,
    status: String,
    word_count: i32,
    updated_at: Option<NaiveDateTime>,
    story_packet_digest: String,
}

struct ChapterGenerationInput {
    user_id: String,
    project_id: String,
    chapter_id: String,
    target_word_count: i32,
    story_packet: StoryPacketV1,
    generation_contract: GenerationContractSnapshotV1,
    prompt_overrides: ChapterGenerationPromptOverrides,
    role_policy: PreparedRoleModelPolicyContext,
    cancellation: DurableStepCancellation,
}

struct ChapterGeneratedDraft {
    normalized_content: String,
    word_count: i32,
    proposed_status: String,
    candidate_digest: String,
    quality: ChapterQualityDecision,
    provider: String,
    model: String,
    execution_digest: String,
    token_usage: UsageAccounting, // actual when supplied, otherwise estimated
}

struct ChapterAnalysisResult {
    decision: ChapterQualityDecision,
    score_summary: ChapterQualityScoreSummary,
    repair_focus: Vec<RepairFocus>,
    derived_effects: ChapterDerivedEffects,
    provider: String,
    model: String,
    execution_digest: String,
    token_usage: UsageAccounting,
}

struct ChapterRepairDraft {
    normalized_content: String,
    word_count: i32,
    candidate_digest: String,
    repaired_from_content_digest: String,
    quality: ChapterQualityDecision,
    provider: String,
    model: String,
    execution_digest: String,
    token_usage: UsageAccounting,
}
```

`ChapterDerivedEffects` is allowlisted data only: analysis report/score summary,
memories, foreshadow mutations and character/organization state projections accepted by
current business owners.  It excludes the raw review text, provider response and prompt.
The `chapters` model has `status`, content, summary, word count and `updated_at`, but **no
native revision column** (`models/chapter.rs:7-25`).  The snapshot must therefore compare
at least all listed chapter fields atomically.  A follow-up hardening migration adding a
monotonic `chapters.revision` (incremented by every human and service write) is preferable;
until it exists, do not pretend an in-memory read is a CAS token.

## ChapterGenerate adapter and atomic commit

### Generation-only facade

Create `chapter_generation_service::generate_single_chapter_candidate(...)` around
`load_generation_context` and `execute_single_generation_candidate_runtime_tracked`.
Before invocation it verifies user/project/chapter ownership and builds the snapshot and
contract.  It checks cancellation and budget first, executes exactly one candidate route,
checks cancellation again, then converts the result to `ChapterGeneratedDraft`:

- `content_applied == false` is not written as a chapter body.  Preserve only a digest and
  quality decision for durable routing; it is never an accepted draft.
- Candidate metadata is reduced to path/fallback/reason/error-code digest, never the raw
  candidate or prompt.
- The execution trace is converted to provider/model/fallback/usage metadata.  Provider
  usage missing from the trace is explicitly marked `estimated`, never zero.

### Repository operation

```rust
async fn commit_chapter_generate_step(
    db: &DatabaseConnection,
    step_id: &str,
    user_id: &str,
    expected_run_version: i64,
    expected_run_epoch: i64,
    expected_step_key: &str,
    expected_background_task_id: Option<&str>,
    expected_chapter: &ChapterBusinessSnapshot,
    draft: ChapterGeneratedDraft,
) -> Result<ClaimedNovelAutopilotStep, NovelAutopilotRepositoryError>
```

In **one transaction**, the method must:

1. validate owned Run; exact `version`, `epoch`, `Running` status and active child task;
2. validate the claimed step is `Running`, same epoch/key/task; this mirrors the existing
   organization commit fences (`repository.rs:1861-1893`);
3. load the chapter by `chapter_id`, require same project/number/outline/title digest and
   exact expected content/summary/status/word-count/update snapshot; a mismatch returns
   `BusinessDataChanged` without any business write;
4. re-check chapter eligibility and expected target outline; then write normalized content,
   word count/status and a new update token;
5. write only allowlisted generation metadata/digests to the durable step, terminalize the
   step and advance the Run once.

Do **not** call history persistence in this transaction while it requires a raw prompt.
If product compatibility later mandates a generation history row, introduce a separate
metadata-only business-owner API first; it must have no prompt/raw response column values.

`BusinessDataChanged` routes to `WaitingHuman` (not retry), as the existing Organization
adapter does (`organization_adapter.rs:145-171`).  Stale version/epoch/task or an invalid
transition is a late-result discard: reload/reconcile; it must not overwrite or
terminalize the newer attempt.

## ChapterAnalyze adapter and atomic commit

The adapter is started only after `ChapterGenerate` committed an accepted normalized body.
It loads a `ChapterBusinessSnapshot` whose `content_digest` is the exact accepted draft.
Extract this generation-only API from the analysis trigger owner:

```rust
async fn analyze_chapter_generation_only(
    db: &DatabaseConnection,
    user_id: &str,
    chapter: &chapter::Model,
    project: &project::Model,
    cancellation: &DurableStepCancellation,
) -> Result<ChapterAnalysisResult, ChapterAnalysisGenerationError>
```

Internally it reuses `prepare_chapter_analysis_story_packet`,
`build_chapter_review_contract`, role-aware configuration, `AIService::generate_text_tracked`
and the existing JSON cleaner/parser (`trigger_runtime_owner.rs:128-205`).  It returns only
validated/allowlisted score, decision, repair focuses and derived mutations; raw review
content is dropped after parsing.

```rust
async fn commit_chapter_analyze_step(
    /* same run/step/task fences */
    expected_chapter: &ChapterBusinessSnapshot,
    expected_generation_result_digest: &str,
    analysis: ChapterAnalysisResult,
) -> Result<ClaimedNovelAutopilotStep, NovelAutopilotRepositoryError>
```

The transaction repeats all Run/step fences and requires that the target chapter still has
the same accepted content snapshot.  It persists allowed plot-analysis/quality/derived
state mutations and terminalizes the durable analysis step together.  It must not create
or complete legacy `analysis_task` rows.  On a business mismatch it returns
`BusinessDataChanged` and Router enters `WaitingHuman`; on a normal `manual_review`
decision it commits the safe decision then transitions the durable Run to `WaitingHuman`.

## ChapterRepair adapter and atomic commit

Repair starts only for a committed analysis result with decision `auto_repair` or `retry`,
and it consumes the exact `content_digest` plus `analysis_digest` it was routed from.  The
facade invokes the targeted-final-repair owner with the existing repair Generation Contract
and quality focus.  One repair step makes one bounded attempt; retry accounting belongs to
the Router/run, not a while loop in the model executor.

```rust
async fn commit_chapter_repair_step(
    /* same run/step/task fences */
    expected_chapter: &ChapterBusinessSnapshot,
    expected_analysis_digest: &str,
    repaired: ChapterRepairDraft,
) -> Result<ClaimedNovelAutopilotStep, NovelAutopilotRepositoryError>
```

The transaction requires all of the following before replacing the chapter content:

- the source chapter snapshot is unchanged;
- its content digest equals `repaired_from_content_digest`;
- the most recently committed durable analysis digest equals `expected_analysis_digest`;
- the selected repaired result is normalized and valid for the expected chapter target;
- Run/epoch/version/step key/background-task fences still match.

It then writes the chapter, safe repair digest/usage metadata and completed step together.
A concurrent human edit, a newer analysis or a newer repair causes `BusinessDataChanged`;
the repair draft is discarded and the Run waits for a human rather than overwriting work.

## Quality decision mapping and Router ownership

Keep the current quality formula.  `resolve_single_generation_quality_gate_plan` maps
`allow_save` to `continue`, repairable `auto_repair`/`repair` to `retry` while retries
remain, `manual_review` to `manual_review`, and otherwise to `continue`
(`single_generation_candidate_quality_owner.rs:366-389`; its tests record the exhausted
repair-budget fallback at `:1775-1778`).  The durable projection is:

| Existing / analysis signal | Durable decision | Router action |
| --- | --- | --- |
| `continue` / `allow_save` | `accept` | Commit generated or repaired body, then schedule `ChapterAnalyze`. |
| `auto_repair` with repair budget | `auto_repair` | Commit only safe analysis decision; schedule one `ChapterRepair`. |
| retryable candidate lifecycle or retryable quality action | `retry` | Increment attempt exactly once and schedule a new `ChapterGenerate`/`Repair` step. |
| `manual_review`, exhausted attempt budget, high-risk policy | `manual_review` | Commit safe decision then `WaitingHuman`; no model loop. |
| malformed result, provider failure, cancellation or stale outcome | no decision / terminal reason | Cancel/stale results commit nothing; bounded provider failures route according to PRD limits. |

Existing single-generation terminal projection only treats retry follow-up as a failed
batch state and deliberately does not make manual review a failed terminal
(`terminal_state_owner.rs:75-107`).  Durable routing should preserve that semantic:
manual review is a non-failure human gate, not a `Failed` Run.

## Cancellation, late-result and CAS fencing

Current direct candidate execution APIs accept no cancellation token
(`candidate_runtime_owner.rs:381-444`).  The first implementation must therefore add a
`DurableStepCancellation` parameter at the new facade boundary and check it:

1. before loading/generating and before budget reservation;
2. immediately before the provider call;
3. immediately after a model result and before parsing/conversion;
4. immediately before repository commit; and
5. after commit before Router schedules the next child task.

Use the existing batch cooperative-cancellation registry only as a pattern; durable runs
need their own `novel_book_autopilot` scope/child task ID.  A `tokio::select!` wrapper may
stop waiting, but it does not make an underlying provider request safe by itself.  If
active in-flight abort is required, extend the shared AI/candidate stream path with
cooperative cancellation checks rather than leaking a background model future.

The repository transaction is the final late-result firewall.  It fences:

- owned project/run ID and user ID;
- `run.version` and `run.epoch`;
- Run/step `Running` state, expected step key and expected child task ID;
- step epoch and exactly one terminal transition;
- chapter ID/project/number/outline target and full expected business snapshot;
- for Analyze/Repair, the accepted content digest and prior durable result/analysis digest.

Pause, cancel, guidance changes and recovery increment/change state so an old model result
fails one of those predicates.  It must be discarded without writing business facts.  A
business snapshot mismatch is distinguishable from stale orchestration state and should
be surfaced as `WaitingHuman`; a stale orchestration fence is reconciled from persisted
facts.

## Rollout order, risks and tests

1. First add the generation-only facades and unit tests proving they do not insert chapter,
   history, analysis-task, plot-analysis or background-task records.
2. Add snapshots, typed commit DTOs and repository tests for success, content/outline
   mismatch, stale version, stale epoch, wrong task ID and exactly-one-terminal-CAS races.
3. Implement `ChapterGenerate`, then `ChapterAnalyze`, then `ChapterRepair`; each gets a
   focused adapter test with cancellation before call, after result and late commit.
4. Only after those pass, connect Router/recovery/SSE.  Recovery reconciles a step from
   the actual chapter content digest before reissuing any model call.

Main risks are the missing native chapter revision token, private analysis generation
functions that need a small extraction, and legacy history/analysis owners carrying
unacceptable prompt/task side effects.  The safe rollout is to extract minimally and keep
all new durable persistence in repository-owned transactions; do not retrofit the existing
single/batch runtime into a hidden durable loop.

## Concrete file/function references

- `backend-rs/src/services/chapter_generation_runtime_service/runtime_execution_owner.rs:314-346`, `:626-648`, `:693-730`
- `backend-rs/src/services/chapter_generation_runtime_service/candidate_runtime_owner.rs:264-348`, `:381-566`
- `backend-rs/src/services/chapter_generation_runtime_service/single_generation_candidate_quality_owner.rs:73-162`, `:252-385`
- `backend-rs/src/services/chapter_generation_contract_prepare_service/chapter_story_packet_owner.rs:43-199`
- `backend-rs/src/services/chapter_generation_history_persistence_service/persistence_owner.rs:179-280`
- `backend-rs/src/services/chapter_analysis_runtime_service/trigger_runtime_owner.rs:128-210`, `:291-335`, `:337-410`
- `backend-rs/src/services/chapter_analysis_runtime_service/persistence_owner.rs:19-171`
- `backend-rs/src/services/chapter_candidate_targeted_final_repair_service.rs:161-250`, `:297-395`
- `backend-rs/src/services/chapter_single_generation_runtime_state_service/terminal_state_owner.rs:62-107`
- `backend-rs/src/services/novel_autopilot/repository.rs:1861-1893`
- `backend-rs/src/services/novel_autopilot/organization_adapter.rs:145-171`
- `backend-rs/src/models/chapter.rs:7-25`