# Design: R7 Autopilot Task and Minimal Coordinator

## Boundary

```text
Authenticated POST /api/background-tasks (existing transport)
  -> TaskRecord(user_id, project_id) + existing generic lifecycle
  -> execute_task("novel_autopilot")
  -> AutopilotCoordinatorService (typed payload only)
  -> autopilot-tool-contract/v1 (allowlist + args + confirmation + task project scope)
  -> novel_workflow_service::transition (ownership + legal transition + CAS)
  -> TaskRecord.result / existing SSE projection
```

The coordinator owns only command orchestration. The Tool Contract owns Tool safety, and the workflow service owns
business authorization, transition legality, and persistence. No layer may directly update `projects.status`.

## Payload and Confirmation

```json
{
  "tool_name": "transition_project_workflow",
  "arguments": "{\"project_id\":\"project-1\",\"expected_phase\":\"inspiration\",\"target_phase\":\"foundation\"}",
  "confirmed_by_user": true
}
```

`arguments` remains a JSON string because it is passed to the existing contract dispatcher. It is parsed only there by
its strict DTO. The coordinator never reads arbitrary fields from it. The authenticated request that creates the task
is the explicit human invocation for this no-UI slice; model/provider output is not accepted as an invocation source.

The internal execution context gains an optional project scope. For `novel_autopilot`, it is always set from
`TaskRecord.project_id`; the contract rejects a Tool argument project ID that differs from that scope before the
workflow service is called. This prevents a task displayed under one project from mutating another project.

## Coordinator Result and Errors

The coordinator returns a serializable versioned Tool receipt on success. Its stable error mapping is intentionally
small:

| Category | External task error |
|---|---|
| invalid task payload or scope | `invalid novel autopilot task payload` |
| missing confirmation | existing safe Tool error text |
| invalid Tool/arguments | existing safe Tool error text |
| access, stale phase, invalid transition | existing safe Tool error text |
| unexpected failure | `autopilot task execution failed` |

No raw arguments, provider body, prompt, URL, token, or database error text appears in task error/result logging.

## Lifecycle and Recovery

`execute_task` gets a single `novel_autopilot` arm which invokes the coordinator, then delegates terminal handling to
existing `complete_task` / `fail_task`. The existing outer `tokio::select!` remains the cancellation owner. The runner
must not emit a second terminal state or create its own task store/SSE event type.

The recovery registry gets an explicit `NonResumable` entry. Since task payload and confirmation are not persisted in
`TaskRecord`, startup recovery treats an orphaned task as failed and requires a new user action. This is a deliberate
safety property, not an incomplete resume implementation.

## Compatibility

- Existing background task endpoint, status values, SSE transport, and JSON field names remain unchanged.
- Frontend adds only the `novel_autopilot` type/label required by source contract tests; it creates no control button.
- No migration, database table, background registry, durable confirmation, checkpoint namespace, or API route is added.

## Rollback

Remove the `novel_autopilot` executor arm, recovery entry, coordinator module, Tool Contract project-scope extension,
and frontend display type/label. No data migration or persisted business schema rollback is required. Any in-flight
non-resumable task is already protected by terminal ownership and will fail safely after a process restart.
