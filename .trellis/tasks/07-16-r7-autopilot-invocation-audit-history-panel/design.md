# Design: R7 Autopilot Invocation Audit History Panel

## Boundary

本任务只连接现有的 durable invocation audit read API 与项目 workflow UI。前端
消费严格限定的安全读取投影，而不把完整后端记录或创建 DTO 传递到展示层。

```text
ProjectWorkflowStatePanel
  -> projectApi.getAutopilotInvocationHistory(projectId)
  -> GET /projects/{project_id}/autopilot/invocations
  -> { items: safe audit projection[] }
  -> local Modal state (loading / error / items)
```

后端仍负责 owner-scoped 访问校验、50 条上限、排序和脱敏。前端不缓存历史，
不修改全局 store，也不基于 audit history 恢复任何 task state。

## Read Contract

在 `frontend/src/services/modules/projects.ts` 的 project-scoped API 中定义：

- `AutopilotInvocationAuditStatus`: `queued | running | succeeded | failed | cancelled`
- `AutopilotInvocationAuditInputSummary`: `expected_phase`、`target_phase`、
  `reason_provided`、`related_task_id_provided`
- `AutopilotInvocationAuditResultSummary`: `changed`、`previous_phase`、
  `current_phase`
- `AutopilotInvocationAuditHistoryItem`: 仅含 UI 被允许读取的字段
- `AutopilotInvocationAuditHistoryResponse`: `{ items: ...[] }`

`HistoryItem` 故意不声明 provider/model/prompt/digest/raw arguments/reason/actor 等
字段。TypeScript 的结构化赋值允许 API 返回额外字段，但组件层无法把这些字段作为
正规模型消费。

## UI Behavior

`ProjectWorkflowStatePanel` 新增“受控调用记录”按钮与一个 `Modal`：

1. 用户点击按钮时打开 Modal，并调用读取 API。
2. 正在加载时展示 `Spin`；空数组展示 `Empty`；请求失败展示 `Alert` 与
   “重新加载”按钮；成功时按服务端顺序显示每项记录。
3. 每项显示安全字段：工具名/契约版本、确认 Tag、执行模式、状态 Tag、请求阶段、
   执行结果阶段（如有）、稳定错误码（如有）以及创建/开始/完成时间。
4. 读取失败仅展示后端 `detail` 或通用中文错误，不展示或解析未知 error body。
5. 关闭 Modal 不修改 workflow state。打开/重新加载都不触发
   `requestBackgroundTaskCenterOpen()`。

不新增全局 Zustand 状态、轮询、SSE 订阅或任何 task mutation。列表第一次打开及
用户显式重试时读取；这保持实现最小且与“审计历史”定位一致。

## Presentation Decisions

- 复用既有 workflow phase presentation，把 machine phase 映射为中文 label/Tag；
  对未知/旧字段以原值安全回退，避免历史阅读因未来 phase 演进崩溃。
- 使用 Ant Design `Modal`、`Alert`、`Empty`、`Tag`、`Spin`、`Space` 和 `Typography`，
  匹配现有工作流面板，不引入新 UI 框架或通用抽象。
- 时间戳用浏览器本地时间格式化；缺失值显示“未开始”或“未完成”。

## Compatibility and Rollback

- API、后端数据库和任务状态机均不变，属于纯前端增量。
- 现有受控切换创建的 payload、成功提示和任务中心打开事件不变。
- 若 UI 发现读取契约不兼容，可仅回退本任务涉及的前端类型、API 方法、按钮、Modal
  与 E2E；不会影响 durable audit 写入或任务执行。

## Explicit Non-Goals

Modal 绝不成为 pause/resume/steer/retry/replay 的入口；审计行不能触发 task
恢复、重新执行或 workflow transition。R7 完成前仍坚持 `NonResumable` 边界。
