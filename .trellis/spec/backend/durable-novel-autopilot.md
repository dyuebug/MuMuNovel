# Durable Novel Autopilot Contract

> Executable backend and cross-layer contract for the resumable whole-book
> orchestrator. This contract is intentionally separate from the legacy R7
> single-tool autopilot contract.

## Scenario: Run a resumable end-to-end novel workflow

### 1. Scope / Trigger

Use this contract when changing any of the following:

- `backend-rs/src/services/novel_autopilot/`;
- Novel Autopilot Run HTTP handlers or DTOs;
- `novel_book_autopilot` scheduling, recovery, SSE, or runtime metrics;
- the frontend Novel Autopilot Workbench;
- Run/Step migrations, completion gates, budgets, or export references;
- manual-review candidate persistence, human decisions, human-gate routing,
  queued-Run dispatch recovery, or public Run configuration validation.

The legacy task type `novel_autopilot` remains a confirmed, single-tool,
`NonResumable` R7 operation. Whole-book orchestration must use the separate
`novel_book_autopilot` task type with `CheckpointResumable` recovery.

### 2. Signatures

#### HTTP API

```text
POST /projects/{project_id}/novel-autopilot-runs
GET  /projects/{project_id}/novel-autopilot-runs
GET  /projects/{project_id}/novel-autopilot-runs/{run_id}
GET  /projects/{project_id}/novel-autopilot-runs/{run_id}/steps
POST /projects/{project_id}/novel-autopilot-runs/{run_id}/pause
POST /projects/{project_id}/novel-autopilot-runs/{run_id}/resume
POST /projects/{project_id}/novel-autopilot-runs/{run_id}/cancel
POST /projects/{project_id}/novel-autopilot-runs/{run_id}/guidance
POST /projects/{project_id}/novel-autopilot-runs/{run_id}/decision
```

```rust
CreateRunRequest {
    config: NovelAutopilotRunConfig,
    total_chapters: Option<u32>,
}

VersionedControlRequest { expected_version: i64 }
GuidanceRequest { expected_version: i64, guidance: String }
HumanDecisionRequest {
    expected_version: i64,
    decision: Accept | Retry | Repair | Stop,
    guidance: Option<String>,
}
```

#### Durable and task signatures

```text
Task type: novel_book_autopilot
Recovery policy: CheckpointResumable
Legacy task type: novel_autopilot
Legacy recovery policy: NonResumable
```

Run statuses are `queued`, `running`, `waiting_human`, `paused`, `completed`,
`failed`, and `cancelled`. Execution scopes are `planning_only`,
`next_n_chapters`, `continue_from_current`, and `complete_book`.

```text
Manual-review candidate table: chapter_draft_attempts
Candidate id: chapter_draft_attempts.id = novel_autopilot_step_runs.id
Candidate source: novel_book_autopilot
Candidate state before accept: waiting_human
Supported new-Run export format: txt
Supported new-Run regenerate_existing value: false
Supported new-Run human gates: every_chapter, every_n_chapters, high_risk_only
```

### 3. Contracts

- Every API operation is project-owner scoped. An inaccessible project or a Run
  outside the URL project scope returns the existing not-found response and must
  not reveal whether the resource exists.
- A project has at most one active Run. A duplicate create returns the current
  active Run instead of starting a competing book workflow.
- Pause, resume, cancel, guidance, and decision requests use `expected_version`.
  Business commits also fence on Run version, epoch, Step identity, attempt, and
  active background task identity.
- Run/Step rows own orchestration state, budgets, safe digests, references, and
  progress only. Chapters, outlines, characters, organizations, quality facts,
  and exported files remain owned by their existing business models/services.
- Run/Step rows and public DTOs must not contain chapter body text, raw prompts,
  credentials, complete provider responses, raw exceptions, or provider
  reasoning/thinking.
- Provider `content` and explicitly returned `reasoning` are separate runtime
  channels. `reasoning_chunk` may be shown only when the provider supplied it;
  it stays in SSE/browser memory and is not persisted or exported.
- The workflow order for `complete_book` is foundation, world building, career
  design, character design, organization design, outline, outline expansion,
  chapter generation, chapter analysis, chapter repair when required, book
  review, bounded book polish, completion gate, and export.
- A Run may become `completed` only after the completion gate confirms chapter
  completeness, no unresolved rewrite obligations, completed review/polish, and
  a valid export reference.
- Cost budgets fail closed when no authoritative provider pricing source exists.
  `used_tokens` is an observed content-plus-reasoning estimate unless an
  authoritative usage contract explicitly replaces it.
- A generated or repaired chapter that requires human review is stored only in
  `chapter_draft_attempts`. Its candidate id is the terminal Step id, its
  `source` is `novel_book_autopilot`, and Run/Step retain only safe metadata and
  a digest. Candidate body text must never be copied into durable snapshots.
- `Accept` consumes the latest terminal Generate/Repair Step and commits the
  candidate in one transaction with Run version/epoch/status/task fences, Step
  identity validation, candidate-state validation, and a chapter snapshot CAS.
  It also writes generation history, marks the candidate accepted, updates Run
  progress, and clears the active background task.
- Accepting a Generate candidate increments `completed_chapters` and adds the
  candidate word count. Accepting a Repair candidate does not increment the
  chapter count and changes `total_word_count` only by `new - old`.
- `Retry` may continue only when the facts router still selects the same Step.
  `Repair` explicitly rewrites ChapterAnalyze/ChapterRepair to ChapterRepair.
  Unsupported, stale, or unavailable decisions fail closed to `waiting_human`.
- Accept routing applies the configured human gate before automatic progress.
  Historical `every_volume` data fails closed when no reliable volume boundary
  exists; new Runs reject that mode rather than advertising unsupported behavior.
- A persisted `queued` Run without a live bound task is a dispatch-retry
  candidate. Create, resume, and startup reconciliation may prepare and bind a
  replacement task; a failed preparation/bind must not strand the Run.
- New Run configuration rejects `regenerate_existing=true`, `every_volume`, and
  export formats other than `txt` before persistence or provider invocation.
  Historical rows remain readable for compatibility.

### 4. Validation & Error Matrix

| Condition | Required result |
|---|---|
| Project is not owned by the caller | Return not-found; do not enumerate Runs |
| Run does not belong to URL project | Return not-found |
| Another active Run exists | Return the existing active Run |
| `expected_version` is stale | Reject with stable CAS/version error; do not mutate |
| Pause/resume/decision is illegal for current status | Reject with stable state error |
| Guidance exceeds 4,000 characters | Reject before persistence |
| Run epoch/status/task/Step fence changed during model call | Reject the late result and mark/supersede the stale attempt |
| Budget would be exceeded before a step | Do not call the provider; pause or enter the defined human gate |
| Cost budget is configured without authoritative pricing | Fail closed to `waiting_human` with `novel_autopilot_cost_estimation_unavailable` |
| Required chapters/review/polish/export are missing | Completion gate must reject `completed` |
| Provider did not return reasoning | Emit no synthetic reasoning |
| Audit actor id is a local `local_<uuid>` id | Persist it in an audit column sized to the user-id contract (`VARCHAR(100)`) |
| `Accept` has no candidate, a stale candidate, or a changed chapter snapshot | Fail closed with `human_decision_candidate_unavailable` or `human_decision_candidate_stale`; do not partially commit |
| `Retry` routes to a different Step | Return to `waiting_human` with `human_decision_retry_route_mismatch` |
| `Repair` targets a Step that cannot be repaired | Return to `waiting_human` with `human_decision_repair_not_supported` |
| Accepted chapter matches an enabled human gate | Commit the candidate, then keep the Run in `waiting_human` instead of auto-dispatching the next Step |
| Historical `every_volume` Run reaches an accepted chapter without a reliable volume boundary | Fail closed with `human_gate_every_volume_boundary_unavailable` |
| Persisted `queued` Run has no live registry task | Re-dispatch idempotently instead of returning the orphan unchanged |
| New Run requests `regenerate_existing=true`, `every_volume`, `markdown`, or `docx` | Reject validation before scheduling; historical rows remain readable |

### 5. Good / Base / Bad Cases

- **Good**: A `complete_book` Run generates three chapters, repairs a failed
  chapter, survives pause and service restart, completes review and polish, and
  stores only a verified export reference before becoming `completed`.
- **Base**: `continue_from_current` starts at the first unfinished outline
  chapter and quality-checks only chapters generated by the current Run. It does
  not rewrite historical manual chapters merely because old analysis is absent.
- **Bad**: Reusing legacy `novel_autopilot` as a resumable multi-step task breaks
  the R7 confirmation boundary and recovery policy.
- **Bad**: Treating frontend progress `100%` as completion without checking
  unresolved rewrites and the export descriptor can publish an incomplete book.
- **Good**: A ChapterRepair candidate waits in `chapter_draft_attempts`; an
  `Accept` decision atomically commits it, adds only the word-count delta, writes
  history, and leaves the chapter count unchanged.
- **Base**: A persisted `queued` Run whose task bind failed is returned by a
  duplicate create request and is safely dispatched again.
- **Bad**: Persisting generated content or reasoning in Run/Step creates a second
  source of truth and violates the privacy boundary.
- **Bad**: Persisting a decision in the API payload without consuming it in the
  coordinator makes Accept/Retry/Repair operationally equivalent to no action.
- **Bad**: Accepting `markdown`, `docx`, `regenerate_existing=true`, or
  `every_volume` for new Runs advertises execution paths the backend cannot
  complete safely.

### 6. Tests Required

Backend changes must retain focused coverage for:

- migration syntax, metadata catalog/head, indexes, foreign keys, and downgrade;
- concurrent active-Run creation;
- API owner/non-owner/project-scope and illegal-state behavior;
- pause/resume/cancel/guidance/decision CAS behavior;
- restart recovery, idempotent commit, and late-result rejection;
- all four execution scopes and all quality decisions;
- every budget type before provider invocation and after usage persistence;
- book review, bounded polish, completion rejection, and export integrity;
- DTO allowlists proving private snapshot fields, content, prompts, and reasoning
  are absent;
- manual candidate persistence outside Run/Step, Generate accept progress, Repair
  accept word-count delta, stale chapter snapshot rejection, and generation
  history creation;
- coordinator consumption of Accept/Retry/Repair, stable decision error codes,
  and every-chapter/every-N/high-risk human-gate routing;
- queued orphan dispatch retry across create, resume, and startup recovery;
- rejection of unsupported new-Run configuration and readability of historical
  configuration;
- PostgreSQL audit actor-id capacity and readiness fixtures pinned to the shared
  latest Alembic head constant.

Frontend changes must retain Workbench E2E coverage for create, refresh recovery,
controls, human guidance/decision, sticky runtime metrics, independent status /
reasoning / content toggles, budget messaging, and the final export descriptor.

Release evidence for the whole-book path must include a real HTTP + PostgreSQL
smoke whose result has `status=completed`, `execution_scope=complete_book`, no
failed chapters, no pending rewrites, all required Step types, and a downloaded
export whose digest matches `final_export_ref`.

On Windows, an MSVC `link.exe` `LNK1318` PDB limit is not sufficient evidence to
skip Rust assertions. Re-run focused tests with the Rust toolchain's
`rust-lld.exe`, disabled test debuginfo, and disabled incremental compilation;
record the executed pass/fail counts separately from `cargo check --tests`.

### 7. Wrong vs Correct

#### Wrong

```rust
// Do not turn the legacy confirmed single-tool task into the durable workflow.
register("novel_autopilot", TaskRecoveryPolicy::CheckpointResumable);

// Do not persist model output in orchestration rows.
step.result_snapshot = json!({ "content": content, "reasoning": reasoning });

// Do not commit a human candidate before re-checking all fences and ownership.
chapter.content = candidate.content;
run.completed_chapters += 1; // Wrong for ChapterRepair.
```

#### Correct

```rust
register("novel_autopilot", TaskRecoveryPolicy::NonResumable);
register(
    "novel_book_autopilot",
    TaskRecoveryPolicy::CheckpointResumable,
);

// Persist only safe facts and references. Business content remains in its owner.
step.result_ref = Some(chapter_id);
step.result_digest = Some(safe_digest);

// Candidate body text lives in chapter_draft_attempts and Accept uses one
// fenced transaction. Repair changes only the aggregate word-count delta.
candidate.id = step.id;
accept_candidate_with_chapter_snapshot_cas(candidate, run_fence).await?;
```

Before a business commit, re-read the Run and require the expected version,
epoch, status, Step attempt, and background task fence. After completion, expose
only allowlisted DTO fields and a verifiable export descriptor.

## Scenario: Persist and consume chapter-repair retry evidence

### 1. Scope / Trigger

Use this contract when a durable `ChapterRepair` Step produces a complete
candidate whose quality action is `retry` or `auto_repair`. It also applies when
the retry budget is exhausted and the last complete candidate must become a
manual-review candidate.

### 2. Signatures

```text
Retry candidate table: chapter_draft_attempts
Retry candidate source: novel_autopilot_chapter_repair
Retry candidate state: retry
Manual-review source: novel_book_autopilot
Manual-review state: waiting_human

repair_payload scope:
  run_id: string
  run_epoch: integer
  source_content_digest: string
  analysis_id: string
  candidate_content_digest: string
  step_attempt: integer
  candidate_full_content: string
  content_complete: true
  quality_gate_message?: string (maximum 1,000 Unicode characters)
```

```rust
NovelAutopilotChapterRepairFailureEvidence {
    expected_chapter: ChapterBusinessSnapshot,
    draft_attempt: chapter_draft_attempt::Model,
    result_digest: String,
}
```

### 3. Contracts

- A budgeted quality retry MUST persist the complete candidate before another
  attempt can consume it. The draft insert, Run update, Step terminal update,
  quality counters, and Step `result_digest` MUST commit in one transaction.
- A later attempt may consume only the newest retry draft matching project,
  chapter, Run ID, Run epoch, accepted-source content digest, and analysis ID.
  Historical drafts without the complete scope are not eligible.
- Candidate full content, persisted digest, and Unicode character count MUST be
  mutually consistent. A corrupt newest in-scope candidate causes fallback to
  the accepted chapter; do not silently select an older retry candidate.
- The accepted chapter and its target word count remain authoritative. Loading
  a retry candidate changes only the in-memory repair baseline and adds the
  latest safe quality message, failed metrics, and repair guidance to the next
  repair target.
- A retry candidate MUST NOT overwrite the accepted chapter. Only a newly
  evaluated candidate with an accepting quality action may use the normal
  fenced chapter commit path.
- When the Step-attempt or consecutive-quality-failure budget is exhausted,
  persist the last complete candidate through the existing manual-review
  contract. The candidate ID is the terminal Step ID, the Run becomes
  `waiting_human`, and no additional repair Step is scheduled.
- Run/Step rows retain only safe status, decision, error code, counters, and
  digest. Do not persist prompts, credentials, provider payloads, raw provider
  errors, endpoints, or reasoning in retry evidence or task results.

### 4. Validation & Error Matrix

| Condition | Required result |
|---|---|
| Retry evidence scope differs from the current Run/epoch/source digest/analysis | Reject the transaction as an invalid transition |
| Chapter snapshot changes before retry evidence is committed | Return `BusinessDataChanged`; persist neither candidate nor Run/Step updates |
| Candidate insert, Run CAS, or Step CAS fails | Roll back the complete quality-failure transaction |
| Newest in-scope retry draft has incomplete content or a digest mismatch | Warn with safe identifiers and use the accepted chapter as the baseline |
| Generated candidate equals the current repair baseline | Reject as `content_unchanged`; do not spend a retry on identical evidence |
| Provider failure has no complete candidate | Update provider-failure state without fabricating retry evidence |
| Quality retry remains within budget | Persist retry evidence, expose safe quality diagnostics, and return `retry_scheduled` |
| Quality retry exhausts its budget | Persist one manual-review candidate and return `waiting_human`; do not schedule attempt N+1 |

### 5. Good / Base / Bad Cases

- **Good**: Attempt 1 fails outline alignment, its complete candidate and quality
  feedback commit atomically, and attempt 2 repairs that candidate with the new
  failed metric included in its target.
- **Base**: No valid scoped retry draft exists, so repair starts from the
  accepted chapter and the committed matching analysis.
- **Base**: The newest scoped draft is corrupt, so the system falls back to the
  accepted chapter instead of using an older candidate with stale feedback.
- **Bad**: Mark the Step failed and increment counters while discarding the
  candidate; every retry then starts from the same accepted text and cannot
  learn from the previous quality result.
- **Bad**: Save a failed candidate into the accepted chapter or reuse a draft
  from another Run, epoch, source digest, or analysis.

### 6. Tests Required

- Assert retry drafts contain complete content, all scope fields, safe quality
  diagnostics, and a digest matching the full candidate.
- Assert the next repair uses the newest matching candidate and quality feedback
  but ignores newer drafts from another Run.
- Assert corrupt newest scoped evidence falls back to the accepted chapter and
  does not select an older retry draft.
- Assert candidate insert plus Run/Step state and Step digest commit atomically;
  force candidate-insert and CAS failures and assert every write rolls back.
- Assert a manual chapter edit rejects retry evidence through the chapter
  snapshot CAS.
- Assert the final allowed quality failure creates one manual-review candidate,
  enters `waiting_human`, and never schedules a fourth attempt when the maximum
  is three.
- Retain focused `chapter_repair` tests and the wider `novel_autopilot` suite.
  On Windows, use the documented `rust-lld` fallback when MSVC reaches the PDB
  limit.

### 7. Wrong vs Correct

#### Wrong

```rust
// Discards the only business evidence that could make the next retry converge.
finish_step_failed(error_code, quality_decision).await?;
schedule_same_repair_from_accepted_chapter().await?;
```

#### Correct

```rust
transaction(|txn| async move {
    validate_chapter_snapshot_and_retry_scope(txn, &evidence).await?;
    insert_retry_candidate(txn, &evidence.draft_attempt).await?;
    update_run_quality_failure_with_fences(txn, &run_fence).await?;
    update_failed_step_with_digest(txn, &step_fence, &evidence.result_digest).await?;
    Ok(())
})
.await?;

// The next attempt reads only the newest fully matching retry evidence.
let baseline = load_scoped_retry(run_id, epoch, source_digest, analysis_id).await?;
```
