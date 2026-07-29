# Design: R7 Autopilot Integration Contract Gate

## Boundary

本任务不引入新的业务能力，而是在现有 owner 边界之间建立可执行的回归证据：

```text
ProjectWorkflowStatePanel
  -> projectApi.requestAutopilotAction(project_id, strict body)
  -> POST /projects/{project_id}/autopilot/actions
  -> canonical TaskRecord + payload (route project, Claims actor, confirmation)
  -> queued durable audit before task spawn
  -> generic task runner
  -> execute_novel_autopilot_task
  -> Tool Contract -> novel_workflow_service CAS
  -> succeeded/failed audit projection
  -> GET /projects/{project_id}/autopilot/invocations
  -> local readonly history Modal
```

测试可以在 HTTP/API、服务与 frontend mock 边界分层执行，但必须有至少一个验证相邻 owner handoff 的跨层组合；不能以彼此孤立的单元测试代替整个链路的契约证据。

## Owner and Safety Rules

- API owner 只负责 Claims/path/strict DTO/canonical payload，不能把 body `project_id` 或 `user_id` 传入 task owner。
- Generic task subsystem 继续拥有排队、运行、终态和取消；Coordinator 仅解释 validated `novel_autopilot` payload。
- Tool Contract 继续拥有 allowlist/schema/confirmation/project-scope validation；workflow Service 继续拥有权限与 CAS。
- Audit Service 继续拥有 queued/running/terminal 安全投影；成功 audit 与 workflow mutation 仍在同一事务中提交。
- Frontend history 继续使用显式 allowlist read model 和 component-local state；不缓存为新的 workflow/task owner。

## Test Strategy

1. 先定位当前 API route 的测试装配与 generic task executor 可调用点。
2. 优先添加最小 Rust integration-style regression：构造 owner action request/record，验证 canonical payload、queued audit、Coordinator 成功/失败结果和 history read model 的联合事实。
3. 如果现有 Axum test harness 难以驱动异步 registry，则使用真实 API helper + existing executor/Coordinator 的明确 handoff，而不是 mock Tool 或手工伪造成功 audit。
4. 保留并运行现有 Playwright workflow scenarios：创建后不乐观变更 workflow，history 面板不展示敏感字段或控制行为。
5. 对发生的真实缺口，在相邻正确 owner 修复；禁止创建 coordinator-local workflow update、audit replay 或 UI 控制捷径。

## Compatibility and Rollback

- 所有新断言和修复必须保持现有 `novel_autopilot` payload、generic task API 与 history response 向后兼容。
- 不修改数据库 schema/migrations。
- 若验证需要额外 test helper，只限测试模块；生产代码变更必须保留现有 service delegation。
- 回滚范围限于本任务新测试和由测试暴露的最小修复，不回退 R7 已验证纵切。
