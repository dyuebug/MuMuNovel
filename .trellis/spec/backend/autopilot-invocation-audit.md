# R7 Durable Autopilot Invocation Audit Contract (2026-07-16)

## 1. Scope / Trigger

- Trigger: a confirmed, project-scoped `novel_autopilot` task is created or reaches a terminal Tool outcome.
- Scope: the Rust audit entity/service, the one-shot Autopilot coordinator, generic task cancellation, the owner-scoped history route, and the PostgreSQL migration catalog.
- This audit is durable invocation history only. It is not the workflow owner, task owner, checkpoint owner, recovery owner, replay mechanism, or a Provider/MCP trace.

## 2. Signatures

```rust
pub async fn create_queued_autopilot_invocation_audit(
    db: &DatabaseConnection,
    record: &TaskRecord,
    payload: &Value,
) -> Result<(), AutopilotInvocationAuditError>;

pub async fn mark_autopilot_invocation_succeeded<C: ConnectionTrait>(
    db: &C,
    task_id: &str,
    result: &AutopilotToolExecutionResultV1,
) -> Result<(), AutopilotInvocationAuditError>;

pub async fn list_project_autopilot_invocation_audits(
    db: &DatabaseConnection,
    project_id: &str,
    limit: u64,
) -> Result<Vec<AutopilotInvocationAuditReadModel>, AutopilotInvocationAuditError>;
```

```text
GET /projects/{project_id}/autopilot/invocations
```

The durable PostgreSQL table is `autopilot_invocation_audits`. It has a unique
`task_id` relation and a `(project_id, created_at)` history index. The schema
is introduced by Alembic revision `20260716_autopilot_invocation_audit` and the
matching Rust migration-executor catalog entry.

## 3. Contracts

- Create the `queued` audit before a `novel_autopilot` task is inserted into the registry or spawned. If this write fails, do not start the task.
- The v1 execution mode is exactly `direct_business_tool`. `provider_name`, `model_name`, and `prompt_digest` are explicitly `NULL`; do not fabricate an AI-provider trace.
- Persist a SHA-256 digest of the strict internal Tool payload, but never persist raw payloads, `reason`, prompts, credentials, or raw errors.
- `input_summary` and `result_summary` are allowlist projections. The input projection may expose workflow phases and boolean presence facts only; the result projection may expose changed/previous/current phase only.
- The coordinator must commit the workflow CAS transition and `succeeded` audit projection in the same transaction. A Tool error rolls back the workflow transaction, then records only a stable failed error code.
- The history route must call `ProjectService::ensure_owned_access` before querying by its route `project_id`; a non-owner receives the existing invisible-project `404` boundary.
- Generic cancellation may set an active audit to `cancelled` with `cancelled_by_user`; it must not overwrite a succeeded or failed terminal audit.
- `novel_autopilot` remains explicitly `NonResumable`. No pause, resume, steer, checkpoint, retry, replay, or unattended multi-step loop is implied.
- The history API must project `AutopilotInvocationAuditReadModel` into an explicit output allowlist before serialization; it must not return the internal record directly. The allowed history fields are audit ID, tool/schema, confirmation, execution mode, input/result phase summaries, stable error code, status, and timestamps. A frontend consumer must use the same allowlisted model and must not render or reconstruct raw arguments, reason, prompts, credentials, provider/model, digest, actor identity, or raw errors, and it must not expose recovery or control actions.

## 4. Validation & Error Matrix

| Condition | Audit behavior | Caller-facing behavior |
| --- | --- | --- |
| Unknown fields, malformed arguments, unknown Tool | Reject before persistence | Stable invalid-task presentation; do not expose arguments |
| `confirmed_by_user == false` | Reject before persistence | Confirmation-required presentation |
| Argument `project_id` differs from `TaskRecord.project_id` | Reject before persistence | Stable invalid-task presentation |
| Audit queued write fails | Do not insert/spawn task | Task creation fails safely |
| Tool workflow CAS succeeds | Commit `succeeded` in the same transaction | Existing versioned Tool receipt |
| Tool validation/CAS fails | Roll back workflow; persist `failed` stable code | Existing safe task failure presentation |
| Active generic cancellation succeeds | Persist `cancelled_by_user` when possible | Existing cancellation response/SSE |
| Audit terminal update cannot persist | Log only `error_code` | Do not reverse generic task terminal ownership |
| Non-owner history request | Do not query audit data | `404 Project not found` |

## 5. Good / Base / Bad Cases

### Good

A confirmed owner transitions `foundation` to `world_building`: a queued audit is written first; the workflow state and a safe `succeeded` summary commit together; the owner can later read the project history.

### Base

A valid task is cancelled while queued or running: generic task cancellation remains the lifecycle owner and the audit is marked `cancelled` without creating a recovery path.

### Bad

Never persist raw `arguments`, `reason`, Prompt text, credentials, a provider/model label for this direct business Tool, or a raw database/Tool error. Never derive a resume/replay action from an audit row.

## 6. Tests Required

- SQLite service tests create a queued record, assert redaction and `direct_business_tool` null provider fields, reject malformed/scope-mismatched payloads, and prove cancellation cannot overwrite a failed terminal record.
- Coordinator/API tests prove queued-before-execution ordering, successful workflow/audit transaction behavior, stale transition `failed` code, owner history access, non-owner `404`, and that the real `cancel_task` handler records `cancelled_by_user` for a running Autopilot audit.
- Migration metadata tests assert the Alembic chain head, Rust catalog entry, unique task index, project-history index, pending revision count, and DDL-step count.
- Verification must include `cargo fmt --check`, `cargo check -j 1`, focused `autopilot` tests, focused `schema_migration_metadata_service` tests, Python migrator syntax parsing, and `git diff --check`.
- Frontend E2E must cover successful, empty, and error history states; prove sensitive extra response fields and pause/resume/retry/replay controls are absent, and prove history reads do not enqueue or mutate workflow state.

## 7. Wrong vs Correct

```text
Wrong: TaskRegistry terminal data is treated as durable audit or is replayed after restart.
Correct: TaskRegistry owns short-lived execution presentation; autopilot_invocation_audits stores safe historical facts only.

Wrong: A direct business Tool receives a made-up provider/model/prompt trace.
Correct: execution_mode=direct_business_tool and provider/model/prompt fields are NULL.

Wrong: Workflow success is committed before the audit success write.
Correct: Workflow CAS and audit succeeded projection use one database transaction.
```
