# R7 Autopilot Control API and Human Gate

## Goal

Expose one authenticated, project-scoped API that creates a confirmed, single-call
`novel_autopilot` background task through the existing generic task lifecycle.

## Requirements

1. Add only `POST /api/projects/:project_id/autopilot/actions` to request a
   confirmed R7 action. Do not add pause, resume, steer, batch, or read/write
   control placeholders.
2. The route path is the sole canonical project scope. The request body must not
   accept `project_id` or `user_id`; unknown fields must be rejected.
3. The actor must come only from authenticated `Claims`. The endpoint must use
   existing project-access verification before a task is created.
4. The request must use a strict typed DTO and accept only the allowlisted
   `transition_project_workflow` tool with stable public workflow phases.
5. `confirmed_by_user` must be explicitly `true`. Missing or false confirmation
   must fail before task creation.
6. The API must construct the internal `novel_autopilot` payload from the route
   project ID, typed action fields, and confirmation, then delegate creation to
   the existing generic background-task lifecycle. It must not execute a tool
   synchronously or write workflow state directly.
7. Existing generic task APIs, task result format, terminal owner, Tool Contract,
   workflow service, database schema, and recovery policy remain authoritative.
8. Do not add a migration, durable audit/checkpoint table, provider/MCP call,
   prompt parsing, multi-step orchestration, automatic retry, or Autopilot UI.

## Acceptance Criteria

- [x] A permitted authenticated project member can create a confirmed
  `novel_autopilot` task for `transition_project_workflow` using the project ID
  from the route.
- [x] The created task uses the existing generic task response/lifecycle and is
  scoped to the authenticated actor and route project.
- [x] Missing/false confirmation, unknown fields, injected `project_id` or
  `user_id`, unknown tools, invalid phases, and unauthorized project access are
  rejected before task creation.
- [x] Task execution continues through the existing Coordinator -> Tool Contract
  -> `novel_workflow_service::transition` chain; the route introduces no second
  workflow mutation path.
- [x] Focused API tests and existing relevant task/workflow regressions pass.
- [x] No pause/resume/steer endpoint, schema migration, durable audit store, or
  provider loop is introduced.

### 验收证据（2026-07-16）

`POST /projects/{project_id}/autopilot/actions` 以认证 Claims 和 route project 为唯一 actor/scope 来源；
request DTO 启用 `deny_unknown_fields`，只允许已确认的 `transition_project_workflow`。实现与 focused
API 回归位于 `backend-rs/src/api/autopilot.rs`，执行仍复用 Coordinator → Tool Contract →
`novel_workflow_service::transition`。当前全量 Rust、前端 E2E、lint/build 均通过；未新增
Pause/Resume/Steer、Provider loop 或第二条 workflow 写路径。

## Constraints and Risks

- `novel_autopilot` remains explicitly non-resumable because the original
  invocation payload is intentionally not persisted for recovery replay.
- This slice is a human-confirmed one-shot control surface, not complete
  autonomous novel generation. R7 remains in progress until the later G2 work.
- A stale workflow phase must reach the existing workflow owner as a normal
  conflict; this route must not auto-retry or overwrite a concurrent human edit.
