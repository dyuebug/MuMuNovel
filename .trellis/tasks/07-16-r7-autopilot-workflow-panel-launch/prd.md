# R7 Autopilot Workflow Panel Launch

## Goal

Expose the already-authoritative R7 confirmed control API through the existing
project workflow panel, so a project member can intentionally queue one
human-confirmed workflow transition as a `novel_autopilot` background task and
immediately observe it in the existing Background Task Center.

## Confirmed Facts

- `POST /api/projects/:project_id/autopilot/actions` already validates Claims,
  route-only project scope, strict request fields, canonical phases, and explicit
  `confirmed_by_user: true` before it creates a generic `novel_autopilot` task.
- `ProjectWorkflowStatePanel` already reads canonical workflow state and exposes
  allowed transitions, but its existing action remains a direct synchronous
  workflow mutation.
- `backgroundTaskApi` already syncs generic task-create responses to the single
  persisted background-task store, and the task center already recognizes
  `novel_autopilot` and polls/cancels generic tasks.
- `OPEN_BACKGROUND_TASK_CENTER_EVENT` is the established low-frequency event for
  opening the existing task center; it does not own durable task state.
- The user has explicitly authorized continued direct development. This task is
  limited to an explicit, user-triggered frontend entry point and must not create
  a second workflow/task/checkpoint/recovery owner.

## Requirements

1. Add a typed frontend client for the existing project-scoped Autopilot action
   endpoint. It must accept only `transition_project_workflow`, canonical
   `NovelWorkflowPhase` values, and `confirmed_by_user: true`.
2. The client must use existing background-task response synchronization so the
   newly created task appears in the current task store without a new tracker.
3. Add an explicit "后台受控切换" workflow-panel action. It must be separate
   from, and must not replace, the existing direct "切换阶段" behavior.
4. The background action must always show an explicit confirmation modal before
   creating the task, including the current and target phase and an optional
   reason. The modal must not permit progress until there is a selected allowed
   target phase.
5. On successful task creation, show a clear success message and open the
   existing Background Task Center. Do not optimistically mutate project workflow
   state; refresh the canonical workflow state only after the task reaches a
   visible terminal result or on user refresh.
6. Preserve current direct transition, rollback/complete confirmation behavior,
   task polling, generic cancellation, and all non-Autopilot task UI behavior.

## Acceptance Criteria

- [x] A user can select an allowed workflow target through a clearly labelled
  background-controlled action and must explicitly confirm it.
- [x] The UI posts the strict existing Autopilot request, with route project ID
  and no user/project IDs in the body.
- [x] A successful create response is synchronized into the existing background
  task store and opens the existing task center.
- [x] The direct workflow transition path is unchanged and remains available.
- [x] API errors leave the modal open, show the existing error shape, and do not
  mutate workflow state locally.
- [x] `npm --prefix frontend run lint` and `npm --prefix frontend run build` pass.
- [x] Existing relevant Rust API tests continue to pass; no backend protocol,
  migration, durable audit table, pause/resume/steer endpoint, provider loop, or
  new task/workflow/checkpoint/recovery owner is introduced.

### 验收证据（2026-07-16）

Workflow Panel 保留原有直接转换入口，并增加独立的后台受控切换：只从 server-returned
`allowed_transitions` 选择目标，确认后只向 route project URL 发严格请求；成功后同步既有 task store
并打开 Task Center，失败时不乐观修改 workflow。实现见
`frontend/src/features/projects/workflow/ProjectWorkflowStatePanel.tsx` 与
`frontend/src/services/modules/backgroundTasks.ts`。当前目标 Playwright 场景、完整前端 E2E、
lint/build 以及全量 Rust 测试均通过；未新增 pause/resume/steer、Provider loop、schema 或新的
workflow/task/checkpoint/recovery owner。

## Out of Scope

- Pause, Resume, Steer, durable audit/event storage, arbitrary Tool selection,
  Provider/MCP calls, multi-step orchestration, durable invocation replay,
  Autopilot task resume, and unattended novel/multi-volume generation.
- Replacing the existing direct project workflow transition path.
- Any database migration or task-center redesign.
