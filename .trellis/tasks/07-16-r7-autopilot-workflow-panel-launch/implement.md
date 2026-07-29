# Implementation Plan: R7 Autopilot Workflow Panel Launch

## Preconditions

- [x] R7 authenticated control API and one-shot human gate are implemented and
  verified through the generic task lifecycle.
- [x] Existing project workflow panel, task store sync and task-center open event
  have been inspected.
- [x] User authorization covers continued direct development.

## Phase A — Typed Client

- [x] Add an explicit typed confirmed-transition request shape in the existing
  background-task service/type boundary.
- [x] Add `backgroundTaskApi` method posting to the existing project-scoped
  Autopilot endpoint and reuse `syncBackgroundTaskToStore`.
- [x] Keep project ID in the URL and never add a body `project_id` or `user_id`.

## Phase B — Workflow Panel Launch UI

- [x] Add separate allowed-target selection for `后台受控切换`; retain the current
  direct `切换阶段` dropdown unchanged.
- [x] Add an always-confirmed modal for the background path, with current/target
  context, optional reason, busy protection and retained error-retry state.
- [x] On success, show feedback and dispatch the existing task-center open event;
  do not update canonical workflow phase optimistically.

## Phase C — Verification

- [x] Add or extend focused frontend helper/component coverage if compatible test
  infrastructure is present; otherwise rely on strict TypeScript/lint/build
  verification and document the repository's test limitation.
- [x] Run `npm --prefix frontend run lint`.
- [x] Run `npm --prefix frontend run build`.
- [x] Run `cargo test --manifest-path backend-rs/Cargo.toml -j 1 api::autopilot`.
- [x] Check modified text is UTF-8 without BOM, LF-only and trailing-whitespace
  free; update the optimization roadmap without marking R7 complete.

## Risky Files

```text
frontend/src/features/projects/workflow/ProjectWorkflowStatePanel.tsx
frontend/src/services/modules/backgroundTasks.ts
frontend/src/services/modules/backgroundTaskTypes.ts
frontend/src/constants/backgroundTaskEvents.ts
```

## Explicit Non-Goals

Do not add pause/resume/steer actions, direct workflow mutation from the task
center, durable audit/checkpoint schema, a second background-task store, a new
workflow phase owner, provider/MCP calls, automatic retries or an unattended
mode.


## Completion Evidence（2026-07-16）

- 已在既有 background-task 类型/服务边界增加严格的已确认工作流转换请求；项目 ID
  只位于 URL，响应继续通过既有 task store 同步。
- Workflow 面板保留原有同步“切换阶段”入口，另设“后台受控切换”；目标仅取自
  `allowed_transitions`，确认弹窗携带当前/目标阶段与可选 reason，失败时保留输入，
  成功时不乐观改写 canonical workflow phase。
- 已为延迟挂载的 Background Task Center 增加一次性、仅内存的 open-request 消费机制，
  避免首屏快速创建任务时丢失打开抽屉的视图切换事件；它不持久化任何任务、工作流、
  checkpoint 或恢复状态。
- 已新增 Playwright 覆盖，验证确认 payload、scope/actor 字段不注入、无 workflow
  乐观更新、`novel_autopilot` 入既有任务中心与抽屉实际打开。
- 质量门禁通过：`npm --prefix frontend run lint`（0 error；33 条既有 warning）、
  `npm --prefix frontend run build`、`npm --prefix frontend run e2e --
  e2e/project-workflow-state.spec.ts`（2/2）、`cargo fmt --check`、`cargo check --manifest-path
  backend-rs/Cargo.toml -j 1`、`cargo test --manifest-path backend-rs/Cargo.toml -j 1 api::autopilot`
  （4/4）以及本任务文件 UTF-8 无 BOM、LF、无尾随空白检查。
- R7 仍为 **IN PROGRESS**：不包含 Pause/Resume/Steer、durable audit、durable invocation
  recovery、更多 allowlisted Tool、多步骤自治或 G2；`novel_autopilot` 为 `NonResumable`，
  UI 不得暗示其可 resume。G2 前禁止无人值守整书或多卷生成。
