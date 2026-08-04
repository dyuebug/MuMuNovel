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

## Scenario: Distinguish manual candidates, provider failures, and UTC run times

### 1. Scope / Trigger

Use this contract when changing chapter generation, chapter analysis, chapter
repair, background task projection, Workbench status text, or Run/Step time
DTOs in the durable whole-book workflow.

### 2. Signatures

Provider or parsing failures without a complete candidate expose an allowlisted
diagnostic object in task results:

```text
failure_diagnostic:
  schema_version = novel-autopilot-failure-diagnostic/v1
  source_code    = invalid_input | context_error | analysis_not_found |
                   execution_config_failed | generation_error | invalid_result
  category       = timeout | rate_limited | upstream_unavailable |
                   authentication_or_configuration | response_invalid |
                   context_invalid | unknown
  provider?      = bounded sanitized provider identifier
  model?         = bounded sanitized model identifier
  http_status?   = integer 400..599
  retryable      = boolean
```

Failure accounting is explicit rather than inferred from `waiting_human`:

```rust
enum NovelAutopilotFailureCounterKind {
    Provider,
    Quality,
    None,
}
```

Task/SSE quality projection is an allowlist:

```text
quality_diagnostics:
  overall_score?
  quality_decision?
  quality_gate_action?
  failed_metrics[0..8].{key,label,value,threshold,gap}
  repair_targets[0..8] (each <= 160 characters)
  focus_areas[0..8]    (each <= 160 characters)
  result_digest?
```

Run and Step time fields are serialized as RFC 3339 UTC strings with a `Z`
suffix:

```text
Run:  created_at, updated_at, started_at, paused_at, completed_at
Step: created_at, updated_at, started_at, completed_at
```

Candidate acceptance uses the existing decision endpoint and a stable conflict
response when no acceptable candidate exists:

```text
POST /api/projects/{project_id}/novel-autopilot-runs/{run_id}/decision
request:  { decision: "accept", expected_version, guidance? }
response: 409 { code: "human_decision_candidate_unavailable", detail: ... }
```

### 3. Contracts

- `waiting_human` means a user action is needed, but it does not by itself mean
  a candidate exists. A candidate exists only when `candidate_id` is present and
  points to a `chapter_draft_attempts` row.
- Quality retry exhaustion after a complete generated or repaired chapter MUST
  persist the final candidate before setting Run `waiting_human`.
- Provider failures, invalid context, and invalid/parse-failed responses MUST
  not fabricate a candidate. They use the stable reason code and
  `failure_diagnostic` to explain why no Accept action is available. A
  non-retryable failure or exhausted transient-provider budget keeps the Run
  in no-candidate `waiting_human`, with Retry/Repair/Stop only.
- Failure counters are mutually exclusive. `Provider` increments the provider
  counter and clears the quality counter; `Quality` does the inverse; `None`
  increments neither. Context and response-invalid failures use `None` and
  MUST NOT consume the Provider budget.
- Typed HTTP status has priority over message fallback. Status 401/403 maps to
  authentication/configuration, 429 to rate limited, 408 to timeout, and 5xx
  to upstream unavailable. Message fallback may recognize a status number only
  next to an explicit `HTTP`, `status`, `status_code`, or `状态码` boundary;
  ports, request IDs, and model build numbers are not statuses.
- Stable no-candidate reason codes include
  `chapter_analysis_provider_timeout`, `chapter_analysis_provider_rate_limited`,
  `chapter_analysis_provider_upstream_unavailable`,
  `chapter_analysis_provider_authentication_or_configuration`,
  `chapter_analysis_result_invalid`, `chapter_analysis_context_invalid`, and
  the matching `chapter_repair_*` codes. Unknown provider failures keep the
  compatible aggregate code `chapter_*_provider_failed`.
- Run/Step/task/SSE/log payloads MUST NOT contain raw provider errors, API keys,
  full prompts, chapter body text, raw response bodies, URLs with query strings,
  provider reasoning, complete quality metrics, or unbounded quality messages.
  Task/SSE may expose only `quality_diagnostics` from the allowlist above.
- Persisting a manual candidate requires
  `candidate.result_digest == digest(candidate.content)`. Accept MUST compare
  the recomputed body digest, private `candidate_content_digest`, and terminal
  Step `result_digest`; any mismatch fails closed before the accepted chapter
  is written.
- Accept capability MUST be enforced by the server, not inferred from UI
  visibility. The API reads the latest Step and waiting candidate before any
  guidance write, Run version increment, or background-task creation. With no
  candidate, only a periodic human gate whose Run and latest Step both have no
  error may continue; no latest Step or any Run/Step error returns
  `409 human_decision_candidate_unavailable`. The coordinator repeats the
  error-bearing Step check as a final fail-closed boundary.
- `Repair` on a `ChapterGenerate` manual candidate keeps the candidate as audit
  evidence, stores bounded user guidance in private Run guidance, and creates a
  new attempt of the same `chapter_generate` Step. It is guided regeneration,
  not candidate acceptance. `ChapterAnalyze` and `ChapterRepair` Repair still
  route to `chapter_repair`.
- PostgreSQL `timestamp without time zone` values are UTC by convention. The API
  adds `Z`; the frontend uses standard `Date` parsing and browser local-time
  rendering. Do not manually add eight hours in the frontend.

### 4. Validation & Error Matrix

| Condition | Required result |
|---|---|
| Complete candidate fails final quality attempt | Insert manual candidate, set `candidate_id`, Run `waiting_human`, reason `chapter_generation_attempts_exhausted` or `chapter_repair_manual_review` |
| Provider timeout without candidate | No candidate row; reason `chapter_*_provider_timeout`; `retryable=true` |
| HTTP 429 without candidate | No candidate row; reason `chapter_*_provider_rate_limited`; `http_status=429` |
| HTTP 5xx without candidate | No candidate row; reason `chapter_*_provider_upstream_unavailable`; `http_status` retained if known |
| Authentication/configuration failure | No candidate row; `retryable=false`; enter no-candidate `waiting_human` immediately |
| Response cannot be parsed or validated | No candidate row; counter kind `None`; enter no-candidate `waiting_human` immediately |
| Step facts or business context are invalid | No candidate row; counter kind `None`; enter no-candidate `waiting_human` immediately |
| Provider transient retry budget is exhausted | No candidate row; enter no-candidate `waiting_human`; expose Retry/Repair/Stop only |
| Crafted Accept for no-candidate failure | Return `409 human_decision_candidate_unavailable`; keep Run `waiting_human`; do not change guidance/version or create a task |
| Persisted or recomputed candidate digest differs | Reject persistence or Accept before writing the accepted chapter |
| User chooses Repair for a generated candidate | Preserve candidate, save private guidance, schedule the next `chapter_generate` attempt |
| Stored UTC value `2026-08-01T05:34:58` | API returns `2026-08-01T05:34:58Z`; Workbench in Asia/Shanghai displays `2026/8/1 13:34:58` |

### 5. Good / Base / Bad Cases

- **Good**: Final quality exhaustion returns task result
  `dispatch_status=waiting_human`, `candidate_id=<step_id>`, and a reason code
  that matches Run `last_error_code` and Step `error_code`.
- **Good**: A Provider 503 returns a stable upstream-unavailable reason and a
  sanitized diagnostic with provider/model/status, but no chapter body or raw
  error message.
- **Good**: A response-invalid failure enters no-candidate `waiting_human`
  without incrementing Provider or Quality counters and without exposing
  Accept.
- **Base**: If the provider error cannot be safely classified, keep
  `chapter_*_provider_failed` and set category `unknown`.
- **Bad**: Showing "候选已保存，等待人工复核" when `candidate_id` is absent.
- **Bad**: Hiding Accept in the Workbench while the decision API still accepts a
  crafted no-candidate request.
- **Bad**: Returning a UTC database timestamp without `Z`, causing the browser
  to interpret it as local time and display an eight-hour offset.

### 6. Tests Required

- Unit tests for timeout, 429, 5xx, result invalid, context invalid, and unknown
  diagnostic mapping.
- Redaction tests proving API keys, prompts, chapter content, URLs with query
  strings, and raw response bodies are absent from serialized diagnostics.
- Repository/adapter tests proving final quality exhaustion creates exactly one
  manual candidate and Accept consumes it with existing fences.
- Counter tests proving Provider, Quality, and None update only their intended
  budgets; typed-status tests proving ports and bare numeric identifiers are not
  misclassified.
- Candidate integrity tests proving persistence and Accept reject all digest
  mismatches, plus coordinator tests proving ChapterGenerate Repair schedules a
  guided generation attempt.
- API tests that craft Accept for Provider/context/result-invalid
  no-candidate failures and assert `409 human_decision_candidate_unavailable`,
  unchanged Run version/guidance, and no background task; coordinator tests
  must cover the same error-bearing Step categories.
- Task/SSE projection tests proving only bounded `quality_diagnostics` fields
  survive and complete metrics, prompt, body, raw response, and API keys do not.
- API tests proving non-null Run/Step times end in `Z` and null times remain
  `null`.
- Workbench E2E with fixed `Asia/Shanghai` timezone proving UTC `Z` values show
  as local Beijing time and provider no-candidate failures do not expose Accept.

### 7. Wrong vs Correct

#### Wrong

```rust
finish_failure("chapter_repair_provider_failed", waiting_human = true);
json!({ "message": raw_provider_error, "candidate_id": null });
// The UI hides Accept, but the API accepts any waiting_human Run.
```

#### Correct

```rust
let diagnostic = error.failure_diagnostic();
let reason_code = diagnostic.reason_code(NovelAutopilotFailureDomain::ChapterRepair);
let counter_kind = if diagnostic.counts_as_provider_failure() {
    NovelAutopilotFailureCounterKind::Provider
} else {
    NovelAutopilotFailureCounterKind::None
};
finish_failure(reason_code, counter_kind, waiting_human, Some(diagnostic));
ensure_accept_decision_available(&db, &path, &user_id, &run).await?;
```
