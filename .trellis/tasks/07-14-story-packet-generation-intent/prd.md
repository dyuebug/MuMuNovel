# Story Packet / Generation Intent 统一生成契约

## Goal

建立一个由 Rust 服务端拥有、带版本且可稳定摘要的 Story Packet / Generation Intent
核心契约，将大纲生成、单章生成、批量章节生成、全章/局部重生成和章节审校/修复
入口归并到同一生成输入模型，同时保持现有 API、SSE、任务恢复和前端状态行为兼容。

用户价值：同一小说事实、创作约束和目标意图在不同生成入口中具有一致语义，为后续
R5 角色级模型策略、R6 业务 checkpoint 和 R7 Autopilot 提供唯一、可追溯的输入边界。

## Confirmed Facts

- R3 小说级 Workflow State Machine 已完成，R4 是当前唯一允许推进的主线。
- 单章 active route 已构造局部 `story_packet`，但类型仍为 `serde_json::Value`，没有
  schema version、稳定 digest 或统一来源合同。
- 当前 `generation_intent` 仍是自由 JSON，并硬编码
  `single_generation_active_route`。
- 大纲、批量、重生成和审校入口仍使用各自 DTO 与准备流程。
- 单章/批量已有 `workflow_runtime_state` JSON 快照和兼容恢复链路；章节生成历史已有
  可扩展 JSON payload。
- 当前没有覆盖全部生成入口的通用、长期、可查询输入审计表。
- 前端页面、Zustand store、旧请求 DTO、响应、SSE event 和错误码属于兼容边界。

## Requirements

1. Rust 服务端必须拥有唯一的强类型 Story Packet / Generation Intent schema，并提供明确的
   schema version、intent kind、target、来源元数据和输入 digest。
2. Story Packet 必须由服务端根据数据库权威项目/章节事实、历史快照和 continuity ledger
   构建；客户端不得直接提交或覆盖完整 Story Packet。
3. 归并优先级必须明确且可测试：系统默认值 → 项目事实/默认值 → 兼容历史快照 →
   当前合法请求 override；不得以空值覆盖有效事实。
4. canonical serialization 必须稳定：对象键顺序和等价嵌套 JSON 表达不应改变 digest，
   数组业务顺序和真实创作约束变化必须改变 digest。
5. 大纲生成/展开、单章生成、批量章节生成、全章重生成、局部重生成、章节审校/修复
   必须通过入口适配器映射到同一核心契约；旧路由和 DTO 不删除。
6. 单章与批量运行时必须把契约快照写入现有
   `workflow_runtime_state.story_packet` 命名空间，并保留已有 progress、quality、gateway
   和 checkpoint 字段。
7. 新快照恢复优先使用版本化契约；旧快照缺少或无法识别该字段时继续使用现有 compat
   options/request runtime state，且不得 panic。
8. 章节 generation history 的既有 JSON payload 必须包含可回放的契约摘要，同时保持旧记录
   和现有 quality/candidate metadata 可读。
9. Story Packet 快照不得包含 API Key、认证头、完整敏感 provider 请求或其他凭据。
10. 实现必须保持旧 API request/response、SSE event、Zustand/task store 和错误码行为兼容，
    前端只在共享类型确有必要时做最小变更。

## Acceptance Criteria

- [x] 存在唯一共享 Rust schema owner，覆盖 Story Packet、Generation Intent、版本、目标、
      来源和稳定 digest；核心链路不再以自由 `Value` 作为契约事实。
- [x] canonical digest 测试覆盖对象键顺序、嵌套 JSON、数组顺序、默认值、约束变化、
      runtime-only 字段和 schema version。
- [x] 大纲、单章、批量、全章重生成、局部重生成、审校/修复入口均有适配测试，证明
      旧 DTO 能构建同一核心契约且保留原默认值/override 语义。
- [x] 单章和批量新任务快照包含 schema/version/intent/digest；resume 新快照不漂移，旧快照
      fallback 仍通过。
- [x] 章节 generation history 新记录包含契约摘要，旧记录无该字段时仍能读取。
- [x] 现有单章/批量/重生成/大纲/审校相关 Rust 测试不回归。
- [x] 前端 `npm run lint`、`npm run build` 通过；若请求类型发生变化，相关 targeted E2E 通过。
- [x] `cargo fmt --check`、R4 focused tests、`cargo check` 和完整 Rust tests 通过。
- [x] 未新增数据库 migration、第二套任务系统、第二个小说阶段事实或 Coordinator。
- [x] 优化路线文档回填 R4 完成证据，并将下一主线切换到 R5/R6。

## Acceptance Evidence — 2026-07-15

- The shared Rust owner, canonical digest, adapters, runtime snapshots, resume fallback, review/repair projection,
  and optional generation-history summary are covered by the Phase A-F implementation evidence.
- Current-worktree focused suites passed for generation contract, single, batch, restore, regeneration, outlines,
  analysis, and history; complete Rust tests passed 1689/1689.
- `cargo fmt --check` and `cargo check` passed. Frontend lint passed with 0 errors / 33 existing warnings and
  frontend production build completed in 5.42s.
- Compatibility audit confirmed unchanged public DTO/response/SSE/frontend store behavior and no R4 migration,
  second task system, second novel-phase fact, or Coordinator.
- Targeted Playwright is N/A because R4 does not change a page request/response or frontend state contract.
- Detailed evidence lives under `validation/phase-f-*.log`, `validation/phase-g-*.log`, and
  `validation/phase-g-compatibility-audit.md`.

## Out of Scope

- R5 的 planner/writer/reviewer 角色模型路由、实际 provider/model/fallback 策略和配置版本。
- R6 的 workflow revision、idempotency key、业务 checkpoint 状态机和 output reference 绑定。
- R7 Autopilot Coordinator、Tool 调度、人工门禁和 Pause/Resume 产品流程。
- 新增通用 generation run / story packet 数据表或生产数据库 migration。
- 一次性删除旧 API、旧 DTO、旧快照和旧历史记录兼容逻辑。
- 将 runtime snapshot 冒充覆盖全部生成入口的永久审计数据库。

## Notes

- 采用无 migration 的 R4 MVP：单章/批量复用现有 runtime snapshot，章节历史复用现有
  JSON payload；其他入口先统一运行时契约，不承诺全类型长期独立审计查询。
- 实现必须使用 UTF-8 无 BOM，保持当前未提交工作区中的无关改动不被覆盖。
