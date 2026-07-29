# Implementation Plan: R7 Autopilot Integration Contract Gate

## Ordered Checklist

1. 阅读本任务 PRD/设计、R7 audit/tool/workflow 前端规范，以及已有 R7 任务的验证证据。
2. 定位 action route、generic task creation/execution、Coordinator、audit history read API 和现有测试装配点，明确最小可执行的跨层测试路径。
3. 添加或强化 Rust 集成回归，证明 confirmed canonical action → queued audit → Coordinator/Tool → workflow/audit terminal 的 handoff；再覆盖一个安全失败路径。
4. 如果测试暴露生产缺口，只在当前 owner 边界做最小修复，并增加对应回归。
5. 运行 Rust 格式化、编译、聚焦 R7 测试；运行前端 lint、build 和项目 workflow Playwright 回归。
6. 对照 PRD 验收项复核 payload/scope/actor、事务/audit、脱敏 read model 和无控制边界；同步 task evidence、适用 spec 与路线状态。

## Validation Commands

```powershell
cargo fmt --manifest-path backend-rs/Cargo.toml -- --check
cargo check --manifest-path backend-rs/Cargo.toml -j 1
$env:RUSTFLAGS='-C link-arg=/DEBUG:NONE'; cargo test --manifest-path backend-rs/Cargo.toml -j 1 autopilot -- --nocapture
npm --prefix frontend run lint
npm --prefix frontend run build
npm --prefix frontend run e2e -- e2e/project-workflow-state.spec.ts
```

如需缩小回归范围，先运行与 action route/Coordinator 对应的 module tests；任何缩小命令都不能替代最终 `autopilot` 聚焦集。

## Review Gates

- 路由 scope 与 Claims actor 是唯一可信来源；内部 payload 不允许 body 注入 project/user。
- 成功 workflow CAS 和 succeeded audit 保持单事务；失败只记录稳定错误码。
- 读取侧仍为 owner-scoped、安全摘要、只读 UI；无控制或恢复行为。
- 不产生 Schema migration、Provider/MCP 依赖、新状态 owner 或第二套后台任务系统。
- 所有可见文本为 UTF-8 无 BOM、LF、无尾随空格。

## Risky Files and Rollback Points

- `backend-rs/src/api/autopilot.rs` 与 `backend-rs/src/api/background_tasks.rs`：不可把测试便利性变成 route/task lifecycle ownership 改写。
- `backend-rs/src/services/autopilot_coordinator_service.rs`：不可在 coordinator 绕过 Tool Contract/workflow Service 或破坏 audit transaction。
- `backend-rs/src/services/autopilot_invocation_audit_service.rs`：不可扩大审计 read model 或存储敏感输入。
- `frontend/src/features/projects/workflow/ProjectWorkflowStatePanel.tsx`：不可把 history Modal 演变成 task-control UI。
- 回滚只撤回本任务新增测试/最小修复；保留先前 R7 纵切实现。

## Execution Evidence (2026-07-16)

- 已在 `backend-rs/src/api/autopilot.rs` 完成真实路由级回归：confirmed action 通过 generic task、queued audit、Coordinator/Tool 到 workflow 与 terminal audit；owner 可读 history，非 owner 保持 `404`。
- 集成审查发现 history route 不能直接序列化内部 `AutopilotInvocationAuditReadModel`，因为该模型保留 actor、project 与 digest 以满足 durable audit。已在 API owner 加入显式 UI allowlist 输出投影；回归断言不返回 `task_id`、`project_id`、`actor_user_id`、schema/provider/model/digest 内部字段，也不返回 reason 或 related task ID 原文。
- 已验证：`cargo fmt --check`、`cargo check -j 1`、`RUSTFLAGS='-C link-arg=/DEBUG:NONE' cargo test -j 1 autopilot -- --nocapture`（25 passed）、`npm --prefix frontend run lint`、`npm --prefix frontend run build`、`npm --prefix frontend run e2e -- e2e/project-workflow-state.spec.ts`（5 passed）。
- 已知无关告警：Rust chapter-generation 既有 unused-code warnings；frontend 既有 Hook dependency warnings 与 Vite circular-chunk warning。均未由本任务新增。
- 范围确认：未新增 migration、Provider/MCP、Pause/Resume/Steer、checkpoint/recovery/replay、自动重试、多步骤自治或第二套 task/workflow owner。R7 与 G2 状态均不因本任务自动改为完成。

## Acceptance Review (2026-07-16)

- [x] **可信 payload / owner**：严格 DTO 拒绝请求体 scope/actor 注入；payload builder 只注入 route `project_id`；`TaskRecord.user_id` 与 `TaskRecord.project_id` 继续作为执行 authority。证据：`backend-rs/src/api/autopilot.rs` 的 builder、route 与 API tests。
- [x] **真实执行和事务链**：confirmed route 已通过 generic task、queued audit、Coordinator/Tool、workflow CAS 与 terminal audit；成功 history 的 phase summary 与 workflow 结果一致。证据：`confirmed_project_owner_creates_scoped_generic_autopilot_task` 与 `autopilot` 聚焦集。
- [x] **安全失败链**：未确认、payload scope mismatch、stale expected phase 均 fail closed；workflow 不被越权修改，failed audit 仅投影稳定 error code。证据：Coordinator/Tool/audit 既有回归（随 `autopilot` 集运行）。
- [x] **history 输出与 UI 契约**：API 显式 allowlist 只输出前端类型声明字段；不输出 actor、project、task ID、provider/model、digest、reason 或 raw arguments。前端 Modal 保持 owner-scoped、只读且无控制按钮。证据：API route regression、`frontend/src/services/modules/projects.ts` 与 Playwright 5/5。
- [x] **质量门禁**：Rust fmt/check、25 个 `autopilot` 聚焦测试、frontend lint/build、workflow Playwright 5/5 通过；编码与空白检查通过。
- [x] **范围冻结**：未修改 schema/migration，未新增 Provider/MCP、recovery/replay、Pause/Resume/Steer、自动重试或多步骤自治。

该验收映射证明本集成门禁通过；它不替代 R7 的跨任务最终收口评审，也不授权进入 G2。

## R7 Final Closure Review (2026-07-16)

- [x] 已逐项复核全部 R7 纵切：受控 Tool Contract、最小 `novel_autopilot` task/Coordinator、
  authenticated project-scoped action API/human gate、durable invocation audit、owner-scoped
  readonly history panel，以及本任务的跨层 integration contract gate。
- [x] 路线范围内的 acceptance matrix 已写入
  `docs/20-r7-controlled-autopilot-mvp-acceptance-review.zh-CN.md`，并在主路线文档标记
  `R7 = PASS（受控 MVP）`；此结论不改变 G2 状态，也不授权无人值守自治。
- [x] 2026-07-16 再次执行本任务 validation commands：Rust fmt/check、`autopilot` 25/25、
  frontend lint/build、workflow Playwright 5/5 全部通过。既有 Rust unused-code、frontend
  Hook dependency 与 Vite circular-chunk warnings 未由 R7 引入，且不阻断该范围内收口。
- [x] 范围复核：未在本任务新增 migration、Provider/MCP、Pause/Resume/Steer、checkpoint/
  recovery/replay、自动重试、多步骤自治或第二套 owner；下一项工作只能建立 G2 固定样本与
  failure-injection 安全门禁任务。
