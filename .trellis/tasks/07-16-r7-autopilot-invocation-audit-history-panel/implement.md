# Implementation Plan: R7 Autopilot Invocation Audit History Panel

## Ordered Checklist

1. 阅读当前 PRD/设计、前端规范和跨层思考指南，确认 project-scoped API 与现有
   workflow panel / Playwright mock 约定。
2. 在 `frontend/src/services/modules/projects.ts` 增加最小、脱敏的 audit history
   读取类型和 `getAutopilotInvocationHistory` 方法；保持 `modularApi.ts` 导出不变。
3. 在 `ProjectWorkflowStatePanel.tsx` 增加局部历史 Modal 状态、按需加载函数、
   状态/阶段/时间的纯展示辅助逻辑和只读入口。
4. 更新 `frontend/e2e/project-workflow-state.spec.ts` 的路由 mock，并添加审计
   成功、空态、失败态、脱敏边界和非控制行为的断言。
5. 搜索并复核新可见文案与 API 路由引用，执行前端 lint、build 和目标 E2E。
6. 更新 `check.jsonl`、`implement.jsonl`、路线文档和本任务 evidence；如验证发现
   跨层契约问题，回到设计阶段修正，但不扩大至 R7 以外的能力。

## Validation Commands

```powershell
npm --prefix frontend run lint
npm --prefix frontend run build
npm --prefix frontend run e2e -- e2e/project-workflow-state.spec.ts
```

若本任务只消费既有读取接口，通常不需要 Rust 测试；如果发现必须修改后端 read
contract，再追加：

```powershell
$env:RUSTFLAGS='-C link-arg=/DEBUG:NONE'
cargo test --manifest-path backend-rs/Cargo.toml -j 1 autopilot -- --nocapture
```

## Review Gates

- 类型模型中不出现 `reason`、raw `arguments`、Prompt、provider/model、digest 或
  actor user id。
- 模态框没有 resume、retry、pause、steer、replay、checkpoint 或 recovery 控件。
- 历史读取不调用 workflow transition、不更新 Zustand 项目状态、不打开任务中心。
- 现有受控切换 E2E 仍验证 no optimistic workflow mutation。

## Risky Files and Rollback Points

- `frontend/src/features/projects/workflow/ProjectWorkflowStatePanel.tsx`：与现有同步及
  后台受控切换共享状态区域；保留它们的 payload 和 success path，避免重构。
- `frontend/e2e/project-workflow-state.spec.ts`：Mock 必须覆盖新增 GET 请求，否则
  启动时的 console/network failure 会掩盖实际 UI 回归。
- 回滚仅撤回本任务新增的前端 API、Modal、测试和文档；不触碰后端 durable audit。

## Completion Status

- [x] 阅读当前 PRD、设计和适用规范，确认 project-scoped 读取与只读 UI 边界。
- [x] 增加最小、脱敏的 `projectApi.getAutopilotInvocationHistory` 类型化读取模型。
- [x] 在 `ProjectWorkflowStatePanel` 实现按需加载的组件局部审计历史 Modal。
- [x] 补齐成功、空态、错误态、脱敏边界和无控制行为的 Playwright E2E。
- [x] 完成前端 lint、build、目标 E2E 与针对性安全字段/行为复核。
- [x] 同步 audit executable spec、优化路线文档和任务 evidence。

## Verification Evidence

- `npm --prefix frontend run lint` — passed on 2026-07-16; service facade and visible-text validation passed, with 33 pre-existing warnings in unrelated files and no lint errors.
- `npm --prefix frontend run build` — passed; service facade validation, visible-text validation, `tsc -b`, and Vite build succeeded.
- `npm --prefix frontend run e2e -- e2e/project-workflow-state.spec.ts` — passed; 5 tests covered existing workflow behavior plus safe success, empty, and error history states.
- Targeted review confirmed history rendering consumes only the explicit allowlisted summary fields and does not call workflow transition, update global workflow state, open the background task center, or expose control/recovery actions.
