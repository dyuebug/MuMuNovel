# Technical Design: R7 Autopilot Control API and Human Gate

## Boundary

```text
POST /api/projects/:project_id/autopilot/actions
  -> authenticated Claims + project access verification
  -> strict typed request DTO
  -> create internal novel_autopilot task payload
  -> existing generic background-task creation/lifecycle
  -> existing Coordinator
  -> existing Tool Contract
  -> novel_workflow_service::transition
```

The new route is a control-plane entry point only. It never invokes the
workflow service directly and never becomes a second task terminal, recovery,
or persistence owner.

## Request Contract

The endpoint accepts a closed request object:

```json
{
  "tool_name": "transition_project_workflow",
  "arguments": {
    "expected_phase": "foundation",
    "target_phase": "world_building",
    "reason": "User confirmed workflow transition",
    "related_task_id": null
  },
  "confirmed_by_user": true
}
```

Design choices:

- `project_id` is supplied only by the route path and injected into serialized
  Tool Contract arguments by server code.
- `user_id` is supplied only by authenticated claims and passed to generic task
  creation as its owner.
- The request DTO uses unknown-field denial at every nested object boundary.
- The route validates the stable public phase names through the same existing
  parsing/validation path used by the workflow contract where possible; it does
  not recreate the transition table.
- `confirmed_by_user` is required and must be true before authorization or task
  creation can produce an action task.

## Reuse Strategy

Prefer a focused API-local/service helper that takes already-authenticated actor,
canonical route project ID, and typed request data, then constructs:

```json
{
  "tool_name": "transition_project_workflow",
  "arguments": "{\"project_id\":\"<route-project-id>\", ...}",
  "confirmed_by_user": true
}
```

It calls the existing generic task create owner instead of reimplementing Task
Record persistence, task IDs, queueing, SSE, or terminal handling. The generic
public `/api/background-tasks` contract remains backward compatible.

## Failure Matrix

| Input or state | Expected result | Task created? |
| --- | --- | --- |
| Valid confirmed request and project access | Existing task-create response, `novel_autopilot` | Yes |
| Missing or false confirmation | 4xx validation error | No |
| Unknown request/arguments field, injected scope/actor | 4xx parse/validation error | No |
| Unsupported Tool | 4xx validation error | No |
| Invalid public phase | 4xx validation error | No |
| No project access | Existing authorization 4xx/404 shape | No |
| Stale expected phase during later execution | Existing workflow conflict; no automatic retry | Task exists, execution fails safely |

## Testing Design

Use the existing API test harness and task-store inspection patterns. Cover:

1. Confirmed happy path creates a `novel_autopilot` task with route project and
   authenticated user, then can execute through the existing chain.
2. Missing and false confirmation reject before creating a task.
3. Unknown fields and injected `project_id`/`user_id` reject as strict DTO
   violations.
4. Unsupported Tool and invalid workflow phase reject before creating a task.
5. An unauthorized actor cannot create a project-scoped task.
6. Generic background-task and workflow tests remain green, proving no lifecycle
   or workflow-owner bypass.

## Rollback

The slice contains no migration or persistent protocol change. Roll back the
new route, DTO/helper, and its focused tests together. Do not roll back or
modify the pre-existing generic task lifecycle, Coordinator, Tool Contract,
workflow service, or recovery policy.
