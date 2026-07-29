# G1 统一契约门禁审查

## Goal

逐项审查 R3-R6、G1-Cancel 与兼容证据，形成可复核的 G1 GO/NO-GO 决议；本任务只做 Gate 审查，
不实施 R7 Autopilot、Coordinator、Tool、Pause/Resume UI 或新的任务基础设施。

## Confirmed Facts

- 正式路线顺序为 `G1 -> R7 -> G2 -> R8`，G1 未通过时 R7 保持阻塞。
- R3、R4、R5、R6 与 G1-Cancel 均已完成本地实现和各自质量门禁，并在路线文档记录证据。
- G1 的正式验收条件来自优化路线文档 `16.4 Phase 1`，共有六项统一契约要求。
- G1-Cancel 已补齐进程内 cooperative cancellation、terminal CAS、commit 后 signal、迟到结果拒绝和
  cancel-vs-completion 竞态证据；它不提供跨进程 cancellation 或任意 token 位置 durable resume。
- 本地工作区包含多任务历史改动，审查必须基于限定文件、既有测试结果和可定位证据，不使用 Git
  no-index diff 推断范围。

## Requirements

1. 核对项目级阶段是否只有一个权威来源，并确认后台任务状态、字数/向导进度不会覆盖该事实。
2. 核对核心生成入口是否统一生成/消费 Story Packet 或通过兼容门面归并到同一 owner。
3. 核对模型选择、实际 provider/model、fallback 原因和配置版本是否可追溯。
4. 核对至少一个长流程是否通过业务 checkpoint 完成真实 DB-backed 恢复验证。
5. 核对旧页面、旧 API、既有 JSON/SSE/status/schema 是否仍通过兼容门面工作。
6. 核对 workflow state、background task state、business checkpoint 与 cancellation 的职责边界是否同时
   具备文档合同和测试保护。
7. 逐项引用具体文件与行号，形成 G1 审查矩阵；任何一项证据不足均判定 NO-GO 并继续阻塞 R7。
8. 若全部满足，则在路线文档记录 G1 GO 日期、证据边界、剩余风险，并将唯一下一节点切换到 R7。

## Acceptance Criteria

- [x] 六项 G1 条件均有至少一处实现证据和一处测试/质量证据。
- [x] G1-Cancel 前置完成证据已纳入 terminal ownership 与职责边界审查。
- [x] 兼容审查确认未新增第二套 task store、项目阶段事实、checkpoint/resume owner 或 Coordinator。
- [x] 审查明确区分进程内 cancellation、durable task state 和 business checkpoint，不承诺跨进程取消或
      任意 token 断点恢复。
- [x] 形成逐项 GO/NO-GO 矩阵；结论可由文件行号复核。
- [x] 若结论为 GO，路线状态更新为 G1 已通过、R7 解锁；若为 NO-GO，明确唯一阻塞项且不开发 R7。
- [x] 文档保持 UTF-8 无 BOM、LF-only；不执行 migration、依赖升级、Git commit/push/reset 或任务归档。

## Evidence Sources

- `docs/15-ainovel-cli-comparison-and-mumunovel-optimization.zh-CN.md`
- `.trellis/tasks/07-14-novel-workflow-state-machine/`
- `.trellis/tasks/07-14-story-packet-generation-intent/`
- `.trellis/tasks/07-15-role-model-policy-generation-execution-audit/`
- `.trellis/tasks/07-16-business-checkpoint-recovery/`
- `.trellis/tasks/07-16-cooperative-cancellation/`
- `.trellis/spec/backend/quality-guidelines.md`
- 对应 Rust owner 与测试模块

## Out of Scope

- R7 Autopilot MVP 的任何生产实现。
- 新增 Coordinator、Tool API、前端 Autopilot 页面、数据库 schema/migration 或新任务类型。
- 重跑已通过且未受本次纯文档审查影响的完整构建；若证据冲突或代码发生新变化，才重新执行相关门禁。
- Git commit、push、reset、任务归档或生产环境操作。

## Review Result（2026-07-16）

- **Decision: GO**. All six G1 conditions have an implementation owner and test/quality evidence.
- The review found one blocking defect in replacement registration ownership: inserting a new cancellation
  registration did not cancel the previous token. The defect was fixed without changing public API, schema,
  migration, status strings, task-store ownership, workflow facts, or recovery protocol.
- Focused verification passed: cancellation registry 3/3, background tasks 26/26, batch runtime state 139/139.
- Static gates passed: `cargo fmt --check`, `cargo check`, and `cargo check --tests`.
- Full Rust suite passed: **1761 passed, 0 failed, 0 ignored** using the documented rust-lld workaround for the
  local MSVC PDB `LNK1318` environment limit.
- The auditable matrix, non-blocking risks, boundary decision, and final roadmap decision are recorded in
  `docs/19-g1-contract-gate-review.zh-CN.md`.
- R7 is unlocked but not implemented. The only next roadmap action is the controlled Tool Contract; G2 remains
  mandatory before any unattended multi-volume expansion.
