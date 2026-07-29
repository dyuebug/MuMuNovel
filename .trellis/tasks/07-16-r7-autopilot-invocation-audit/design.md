# Design: R7 Autopilot Durable Invocation Audit

## Decision

新增独立的 PostgreSQL 表 `autopilot_invocation_audits`，由
`autopilot_invocation_audit_service` 唯一拥有持久化、脱敏投影、状态变更和读取模型。
它与 `TaskRegistry` 的关系仅是 `task_id` 关联：TaskRegistry 继续管理执行态、SSE、取消和
短期恢复；audit 仅保存不可变调用事实与终态，不提供恢复入口。

## Data flow

```text
confirmed HTTP request
  -> route validates request + project ownership
  -> generic task factory allocates TaskRecord/task_id
  -> audit service creates queued record (before registry insert/spawn)
  -> TaskRegistry executes the task
  -> coordinator marks audit running
  -> DB transaction { workflow CAS transition + audit succeeded projection }
  -> generic task terminal presentation

failure before/inside Tool
  -> audit service writes failed + allowlisted error code
  -> generic task records its normal failed presentation

project owner GET audit history
  -> ProjectService::ensure_owned_access
  -> audit service reads only matching project_id
```

## Durable schema (v1)

`autopilot_invocation_audits` fields:

| Field | Meaning |
| --- | --- |
| `id` | UUID audit identity |
| `task_id` | unique generic background-task relation |
| `project_id` | project scope; FK uses project lifecycle deletion semantics |
| `actor_user_id` | authenticated actor captured from TaskRecord |
| `schema_version` | `autopilot-invocation-audit/v1` |
| `tool_name` / `tool_schema_version` | allowlisted Tool identity and contract version |
| `confirmed_by_user` | confirmation fact (must be true for v1 Tool) |
| `execution_mode` | `direct_business_tool` |
| `provider_name` / `model_name` / `prompt_digest` | nullable; direct Tool writes null explicitly |
| `input_digest` | SHA-256 of canonical strict internal Tool payload |
| `input_summary` | allowlisted JSON text: phases + boolean presence indicators only |
| `status` | `queued`, `running`, `succeeded`, `failed`, `cancelled` |
| `result_summary` | allowlisted JSON text, never raw Tool result |
| `error_code` | stable redacted code, never raw error/detail |
| timestamps | `created_at`, `started_at`, `completed_at` |

Indexes: unique `task_id`; project history index `(project_id, created_at)`.

## Consistency boundary

- The queued audit write happens before the task is exposed/spawned. If it fails, the
  task is not inserted or spawned.
- For a successful workflow Tool, `novel_workflow_service` receives an execution
  transaction; its CAS update and the audit succeeded projection commit atomically.
  No task status is used as the business source of truth.
- Failed Tool attempts are recorded after rollback using a stable error-code mapper.
  A database outage that prevents this secondary write is logged without raw payload;
  the existing generic task failure remains visible. This is the residual failure mode
  until a transactional outbox is introduced; no false `succeeded` audit is emitted.
- Generic cancellation remains a task-lifecycle operation. The audit records the actual
  Tool outcome, not an inferred UI status; this task does not add pause/resume semantics.

## Privacy and authorization

The audit service parses the strict internal DTO and creates summaries itself. No caller
can pass a summary, actor, project, status, provider, model or prompt field. The read route
first invokes `ProjectService::ensure_owned_access`, returns the established 404 shape for
not-found-or-denied access, and then filters by the route project ID.

## Migration ownership

Production schema migration is not auto-generated at startup. The change must add one
ASCII-named PostgreSQL Alembic revision and one matching revision entry/DDL list/head update
in `schema_migration_metadata_service`. The SeaORM entity is a runtime mapping, while SQLite
in-memory entity creation is test-only.

## Explicit non-goals

Audit records are not checkpoints, do not contain the raw invocation payload, cannot be
used to replay a Tool, and must not make `novel_autopilot` resumable.