# 小说级 Workflow State Machine

## Goal

在不新增第二套项目阶段事实、后台任务状态或章节 checkpoint owner 的前提下，把现有
`projects.status` 提升为 MuMuNovel 唯一的小说级创作阶段事实，为项目进度展示、人工回退、
后续 Story Packet、Coordinator 和恢复能力提供稳定、版本化、可并发校验的契约。

## User Value

- 用户可以在项目详情页明确看到小说所处的创作阶段和可执行的下一步。
- 用户可以显式推进或回退阶段，不再由字数进度、后台任务状态或局部页面状态推导项目阶段。
- 页面刷新、服务重启和多个客户端并发操作后，阶段仍以数据库中的同一事实为准。

## Confirmed Facts

- 当前唯一持久化项目阶段字段是 `projects.status`，Rust 模型类型仍是 `String`。
- 项目创建和完整向导创建写入 `planning`；向导完成写入 `writing`；向导清理写回 `planning`。
- 通用 `PUT /projects/{id}` 当前允许任意字符串直接覆盖 `status`，项目导入缺省可能写入 `draft`；书籍导入会直接写入 `writing`。
- `wizard_status/wizard_step` 仅描述初始化向导进度；后台任务状态仅描述执行状态；字数仅是指标。
- 当前没有通用项目业务审计表；R3 不在未授权情况下新增数据库表或字段。
- G0 已通过，R3 已解除阻塞；R4 及后续能力必须等待 R3 完成。

## Canonical Workflow Phases

R3 v1 使用以下规范阶段，序列化值固定为 snake_case：

1. `inspiration`
2. `foundation`
3. `world_building`
4. `character_design`
5. `outline`
6. `writing`
7. `reviewing`
8. `polishing`
9. `completed`

历史值按兼容别名读取：

- `planning`、`draft` → `foundation`
- `revising` → `reviewing`
- `active` → `writing`
- 九个规范值保持原义
- 未知持久化值不得静默猜测，API 返回明确的数据契约错误

新建项目和首次合法转换后只写规范值。现有数据库记录不执行批量迁移。

## Public Transition Rules

- `inspiration` → `foundation`
- `foundation` → `inspiration`、`world_building`、`writing`
- `world_building` → `foundation`、`character_design`
- `character_design` → `world_building`、`outline`
- `outline` → `character_design`、`writing`
- `writing` → `outline`、`reviewing`、`completed`
- `reviewing` → `writing`、`polishing`、`completed`
- `polishing` → `reviewing`、`completed`
- `completed` → `reviewing`、`polishing`
- 同态转换是幂等成功，但不出现在 `allowed_transitions` 中

`foundation → writing`、`writing → completed`、`reviewing → completed` 和
`completed → reviewing` 保留现有四态工作流的兼容能力。向导清理可以使用独立内部 reset，
但该 reset 不进入公开可选转换集合。

## Requirements

### Single Source of Truth

- `projects.status` 必须继续作为唯一小说级阶段持久化事实。
- 新 API、旧 PUT、向导完成、向导清理、项目导入和书籍导入必须复用同一 Rust phase 解析与转换 owner。
- `wizard_status`、`wizard_step`、`current_words`、`TaskStatus` 和章节 runtime state 不得成为阶段来源。
- 后台任务 pending/running/completed 不得自动推进或回退项目阶段。

### Versioned API Contract

- 提供 `GET /api/projects/{id}/workflow-state`。
- 提供 `POST /api/projects/{id}/workflow-state/transition`。
- 状态响应包含 `schema_version=1`、`project_id`、`phase`、`allowed_transitions`、
  `can_rollback`、`suggested_next_phase`、`updated_at` 和 `source="projects.status"`。
- 转换请求包含 `target_phase`、`expected_phase`、可选 `reason` 和可选 `related_task_id`；人工回退或完结后重新打开时 `reason` 必填。
- 所有权只来自认证 Claims，不接受请求体覆盖用户身份。

### Concurrency and Errors

- 转换必须使用 `expected_phase` 做乐观并发校验。
- 合法转换使用带 `id + user_id + current status` 条件的数据库更新，避免 last-write-wins。
- 非法转换、陈旧 expected phase 和未知持久化阶段返回明确 4xx；不存在或无权访问保持 404；
  数据库内部错误保持 500。
- 两个使用同一 `expected_phase` 的并发转换最多只有一个能够改变阶段。

### Compatibility

- 旧 Project CRUD 路径和响应结构保持可用。
- 旧 PUT 的 `status` 字段继续接受四态别名，但必须经过统一状态机；未知或非法值不再旁路写入。
- 项目导入将历史别名规范化；缺省值改为规范 `foundation`；未知值必须在验证或导入阶段明确拒绝。书籍导入进入 `writing` 时也必须调用同一 owner。
- 导出仍保留数据库中的权威阶段值。
- 不执行生产数据库 migration，不批量重写历史记录。

### Auditability

- 每次成功改变阶段必须发出结构化 `tracing` 审计事件，至少包含：
  `schema_version`、`project_id`、`actor_user_id`、`from_phase`、`to_phase`、
  `reason`（长度受限并去除控制字符）、`related_task_id` 和事件时间。
- 同态幂等请求记录为 no-op，不伪造数据库变更。
- R3 不新增审计表；若后续要求可查询的完整历史，由独立 Schema 授权任务实现，不能污染
  `generation_history`、后台任务 checkpoint 或其他业务表。

### Frontend

- 项目详情常驻区域显示当前阶段、建议下一阶段和可用转换。
- 前端只展示后端返回的 `allowed_transitions`，不复制合法转换表。
- 转换期间禁用重复提交；409/冲突后重新获取服务端状态。
- 成功转换后同步现有 Zustand `currentProject/projects` 中的 `status`。
- 字数达到 100% 仅影响进度展示，不能把项目阶段派生为 `completed`。
- UI 不读取后台任务 store 推导小说阶段。

## Acceptance Criteria

- [x] Rust 定义九态 `NovelWorkflowPhase`、历史别名解析、schema version 和唯一转换表。
- [x] GET workflow-state 对拥有者返回版本化状态视图，对无权/不存在项目返回 404。
- [x] POST transition 支持合法推进、人工回退、同态幂等和 `expected_phase` 并发冲突检测。
- [x] 非法转换和未知阶段在 Rust 层被拒绝，数据库不会被修改。
- [x] 旧 PUT、向导完成、向导清理、项目导入和书籍导入不能绕过统一 phase owner。
- [x] 页面刷新或服务重启后阶段仍来自 `projects.status`。
- [x] 结构化日志能审计每次真实转换的操作者、原因、时间和关联任务。
- [x] 项目详情 UI 显示当前阶段、建议下一步和后端允许的转换，并正确处理冲突刷新。
- [x] 字数进度、向导进度和后台任务状态不会覆盖项目阶段。
- [x] 现有四态 API 输入仍可兼容映射，Project CRUD 的非状态字段更新保持兼容。
- [x] 后端定向测试、完整 Rust 测试/check/fmt/clippy、前端 build/lint 和相关 E2E 通过。
- [x] 路线文档记录 R3 的实现证据和 G1 前置状态。

### 验收证据（2026-07-16）

上述验收项由 `.trellis/tasks/07-14-novel-workflow-state-machine/implement.md` 的 Test Matrix、
`backend-rs/src/services/novel_workflow_service.rs` 的 owner-scoped conditional-update/CAS 测试和
`frontend/e2e/project-workflow-state.spec.ts` 支持。默认单测不连接外部数据库；另有显式忽略的
`postgres_concurrent_transitions_with_same_expected_phase_change_at_most_once`，只在设置
`MUMU_R3_POSTGRES_URL` 且指向全新隔离 PostgreSQL 数据库时执行，已完成一次真实并发 CAS 复验。

## Out of Scope

- 新建 workflow audit/history 数据表或修改生产数据库 Schema。
- R4 Story Packet / Generation Intent。
- 角色模型路由、业务 checkpoint、统一 cooperative cancellation。
- Coordinator、Autopilot、整书或多卷无人值守生成。
- 把后台任务运行态合并进小说级阶段。
- 承诺任意 Token 位置断点续跑。

## Open Questions

无阻塞性产品问题。路线、兼容性和安全边界均可由仓库证据与既有授权确定。
