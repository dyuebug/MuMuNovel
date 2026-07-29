# R8 Eval / 创作档案 / 运行指标

## Goal

在已通过 R7 受控单次 Autopilot MVP 与 G2 安全门禁的前提下，建立一个最小、
可重复、只读且脱敏的评测与观测闭环：

1. 使用静态 golden sample 评估既有 generation contract / execution audit 的安全摘要；
2. 基于现有项目导出链路提供创作档案的显式 allowlist，而不是复制工作流或审计事实；
3. 将现有 workflow、generic task、quality metrics 和 invocation audit 的安全摘要关联为运行指标。

R8 的目标是让后续质量变化可被证据驱动地比较和复盘；它不是新的 Autopilot 执行能力，
也不替代现有 task、workflow、generation contract、audit 或 checkpoint owner。

## Confirmed Facts

- R7 已于 2026-07-16 收口为 PASS：唯一 Tool 为 `transition_project_workflow`，
  `novel_autopilot` 保持单次、人工确认且 `NonResumable`。
- G2 已于 2026-07-16 判定为 GO：固定 fixture、CAS/Tool 拒绝、queued/terminal audit
  failure injection、history 故障和 readonly UI 回归均已通过。
- 现有 `generation_execution_audit_service` 和 generation contract history 已有版本化、
  可读的持久化摘要；chapter quality metrics 已有 history/summary 和项目级趋势 read model。
- 现有项目导出 API 已建立 owner-scoped export context 与显式 export options，可作为
  创作档案的复用入口，但不能把内部或敏感字段无筛选带出。
- 用户已授权持续开发；R8 仍需遵守路线冻结边界，不以该授权扩大 Autopilot 运行能力。

## Requirements

1. 创建仅测试可用、静态且不含 Prompt、credential、raw arguments/raw errors、Provider secret
   的 golden sample fixture；样本应只使用既有安全摘要和期望判定。
2. 为 golden sample 建立纯本地、确定性的评测/投影回归；判定不得依赖网络、真实 Provider、
   真实项目内容或运行时 Prompt。
3. 复用现有 project export context，新增或收紧一个创作档案 allowlist projection；archive
   只包含已批准的项目/工作流版本、generation contract/audit 安全摘要、质量摘要和人工反馈
   关联键，不复制原始执行输入或内部 audit read model。
4. 复用既有 workflow、generic task、quality metrics、generation execution audit 和 Autopilot
   invocation audit，提供 owner-scoped、只读的运行指标安全摘要；不新增表、migration、
   task store、workflow state、audit store 或 checkpoint owner。
5. API/UI 若在本任务中增加，只能是 owner-scoped readonly read model，必须使用明确 DTO/
   allowlist；不得增加控制、恢复、重试、replay 或自动执行入口。
6. 补齐 Rust/前端回归，覆盖脱敏、owner scope、空数据和历史兼容；完成 fmt/check/focused tests/
   frontend lint/build 与相关 E2E。
7. 记录每个指标的事实来源、时间语义和缺失数据语义，确保 UI/导出不把 aggregate summary
   误解为新的 canonical workflow/task/audit 事实。

## Acceptance Criteria

- [x] golden sample fixture 是稳定、可读、无敏感信息的 test-only 输入/期望输出，且不参与 production runtime。
- [x] 评测回归在本地确定性通过，覆盖正常摘要、缺失/未知 schema 和脱敏边界。
- [x] 创作档案仅从既有 owner 读取，导出为显式 allowlist；不返回 Prompt、credential、raw arguments/errors 或内部 actor/project/digest。
- [x] 运行指标只读关联既有 task/workflow/audit/quality owner，空数据与历史缺失保持兼容，不创建第二份状态事实。
- [x] 新增 read API（如需要）先验证 project owner，错误输出为稳定、安全摘要；不得改变 workflow/task/audit。
- [x] 前端（如需要）只展示评测/档案/指标，不提供 Pause/Resume/Steer、retry、replay、checkpoint/recovery 或 Autopilot 启动控制。
- [x] 不新增数据库 migration/schema、Provider/MCP runtime、真实 Prompt 执行、Autopilot Tool 或执行流程。
- [x] 文档、测试和质量门完整记录；路线文档继续明确 R8 不授权无人值守 Autopilot。

### 验收证据（2026-07-16）

实现与验证记录见 `implement.md:60-103`：golden fixture/脱敏投影、owner-scoped creative archive、
`runtime-metrics/v1` readonly DTO 和 Project Detail readonly UI 均已完成。focused R8 Rust tests 11/11、
前端目标 E2E 6/6，以及路线级 Rust 1801、前端 E2E 14 passed / 13 skipped、lint/build 已通过。
R8 仅收口只读证据闭环，不授权新的 Autopilot runtime、Provider/MCP、真实 Prompt、recovery/replay 或 schema。

## Out of Scope

- 多 Tool、多步骤、无人值守、整书或多卷 Autopilot。
- Pause、Resume、Steer、自动重试、checkpoint、recovery/replay。
- Provider/MCP、网络评测、真实 Prompt 执行、Prompt/credential 保存。
- 生产数据库 migration/schema 扩展，或新的 task/workflow/audit/checkpoint store。
- 将项目导出改造成全量内部数据库转储，或提前复制 TUI/Headless CLI。

## Planning Decision

R8 是跨 backend、frontend 与文档的复杂任务。实施将严格按：静态评测样本 → archive
allowlist projection → readonly metrics summary → UI/回归 的顺序推进。每一步只复用现有
事实 owner；若发现现有 read model 无法安全表达所需字段，则缩小 R8 输出范围，而不是新增
运行时控制、持久化事实或 schema。
