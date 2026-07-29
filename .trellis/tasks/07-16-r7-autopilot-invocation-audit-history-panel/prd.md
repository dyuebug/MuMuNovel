# R7 Autopilot Invocation Audit History Panel

## Goal

在项目工作流 UI 中提供一个只读入口，让项目所有者查看既有的、脱敏的
Autopilot invocation audit history。该能力用于追踪已确认的受控工作流切换，
不改变任务执行、恢复或控制语义。

## Confirmed Facts

- 后端已提供 owner-scoped 接口：`GET /projects/{project_id}/autopilot/invocations`。
- 该接口在项目不属于当前用户时保持既有的不可见/无权限语义；成功响应为
  `{ "items": [...] }`，服务端按创建时间倒序返回最多 50 条记录。
- durable audit 是 invocation history，不是 checkpoint、recovery/replay owner、
  provider trace 或重试基础；`novel_autopilot` 仍为 `NonResumable`。
- 审计记录只保存安全投影：tool/schema、确认事实、阶段摘要、状态、稳定错误码、
  时间戳；不得把 raw arguments、reason、Prompt、凭证、provider/model 或未脱敏
  异常展示到 UI。
- 现有 `ProjectWorkflowStatePanel` 已使用 Ant Design，并能创建后台受控切换；
  新入口必须不影响其乐观状态与任务中心打开行为。

## Requirements

1. 为现有 invocation history 接口增加前端类型化读取模型和 `projectApi` 调用。
2. 在 `ProjectWorkflowStatePanel` 中增加独立、按需加载的只读审计历史 Modal。
3. 仅展示可安全投影的字段：工具/契约版本、确认状态、执行模式、审计状态、
   expected/target phase、changed/previous/current phase、稳定错误码及时间戳。
4. 提供 loading、empty、error、success 反馈；每次打开历史时重新读取，不把结果
   写入全局 store，也不改变 workflow state。
5. 通过明确的 UI 文案表明此处是审计历史；不新增暂停、恢复、引导、重试、重放、
   checkpoint 或任何控制按钮。
6. 保持已有后台受控切换创建、原有同步 workflow transition 和任务中心入口的行为。

## Acceptance Criteria

- [x] `projectApi` 能请求 `/projects/{project_id}/autopilot/invocations` 并得到类型化
      `{ items }` 响应。
- [x] 工作流面板有“受控调用记录”只读入口；打开后显示 loading、空态、失败态或
      历史列表。
- [x] 历史项可读地展示审计状态、确认事实、阶段摘要、稳定错误码和时间戳；
      `queued`、`running`、`succeeded`、`failed`、`cancelled` 均有明确展示。
- [x] UI 不渲染或拼接 raw arguments、reason、related task id、Prompt、provider、
      model、digest、actor user id 或未脱敏错误文本。
- [x] UI 不包含 resume、retry、pause、steer、replay 等控制行为，且不会触发
      workflow optimistic mutation。
- [x] 现有受控切换 E2E 场景继续通过；新增 E2E 覆盖审计历史的请求、成功、空态、
      失败态及脱敏展示边界。
- [x] 通过前端 lint、build 与目标 Playwright E2E 验证。

### 验收证据（2026-07-16）

前端 `projectApi.getAutopilotInvocationHistory` 仅读取 owner-scoped allowlist DTO；Workflow Panel 的
历史 Modal 覆盖 loading、空态、失败态、五种审计终态和脱敏边界，且不包含 control/recovery 行为。
实现见 `frontend/src/services/modules/projects.ts` 与
`frontend/src/features/projects/workflow/ProjectWorkflowStatePanel.tsx`；当前
`npm --prefix frontend run e2e -- e2e/project-workflow-state.spec.ts` **6/6 passed**，完整前端
E2E、lint 与 build 亦通过。

## Out of Scope

- 任何后端审计 schema、API 路由、鉴权语义或分页协议变更。
- 审计日志导出、筛选、搜索、无限滚动或全局审计中心。
- Pause、Resume、Steer、checkpoint、recovery、replay、自动重试，或无人值守的
  多步骤/全书生成。

## Planning Decision

用户已在本会话明确授权后续工作持续直接开发。本子任务以最小可验证范围实现：
复用已有读取接口，在现有项目 workflow 面板内加入按需加载的只读 Modal。
