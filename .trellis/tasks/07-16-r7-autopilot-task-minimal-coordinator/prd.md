# R7 Autopilot Task and Minimal Coordinator

## Goal

在已完成的 `autopilot-tool-contract/v1` 之上，实现 R7 的第二条受控纵切：新增可被既有后台任务
lifecycle 执行的 `novel_autopilot` 单次任务，以及不调用 AI Provider 的最小 Coordinator。它只能执行由
已认证用户明确确认的一条 allowlisted Tool 调用，并继续委托现有 Tool Contract 与 workflow owner。

## User Value

- 已确认的项目 workflow 推进可以进入现有后台任务 lifecycle，获得 running/progress/cancel/terminal
  状态和任务中心兼容投影，而不是在 HTTP handler 或模型输出中直接写业务状态。
- 首个 Coordinator 能证明“任务 → 受控 Tool → canonical Rust Service”的调用链，同时不把 Prompt、
  Provider ToolCall 或裸 JSON 当作权限边界。
- 进程重启或恢复场景会明确安全失败，避免在未持久化的确认/输入条件下自动重放写入动作。

## Confirmed Facts

- R7 第一纵切已完成：`transition_project_workflow` 是唯一 allowlisted Tool，mutating dispatch 必须使用
  `AutopilotToolConfirmation::ConfirmedByUser`，并委托 `novel_workflow_service::transition`。
- `TaskRecord` 持久化 task lifecycle、result、checkpoint 和 fingerprint，但不保存原始 `payload`；因此不能
  安全地恢复或自动重放包含确认和 Tool arguments 的 Autopilot invocation。
- `background_tasks::spawn_task_execution` 已提供 Pending → Running、全局 cooperative cancellation、
  generic `execute_task` dispatch、terminal ownership 与 SSE task projection。
- `production_ci_contract_tests` 要求每个 `execute_task` 分支都有显式 recovery policy，并校验前端
  `BackgroundTaskType` 与 Rust execution/recovery owner 一致。
- 当前 generic background task route 已由认证用户创建任务；该请求是本纵切唯一的人工确认来源，不接受
  Provider ToolCall、Prompt 或后台恢复过程作为确认来源。

## Requirements

### R1. Typed Autopilot task payload and internal scope

- 新增 `novel_autopilot` task type；其 payload 必须使用 strict typed DTO（`deny_unknown_fields`），且仅允许
  `tool_name`、`arguments` 与 `confirmed_by_user`。
- 仅接受 `transition_project_workflow`；未知 Tool、无效/非 object arguments、未知字段、空 project id、
  未确认调用与 task/payload project scope 不一致均在 workflow service 前失败。
- task actor 只能使用 `TaskRecord.user_id`；task project scope 只能使用 `TaskRecord.project_id`，不能由 payload
  或 Tool JSON 覆盖。
- 将 task project scope 注入 Tool Contract internal execution context；scope 不匹配必须 fail closed。

### R2. Minimal deterministic Coordinator

- 新增 focused Rust service owner，将 typed task payload 映射为 `dispatch_autopilot_tool_call`。
- Coordinator 不调用 AI Provider、不读取 Prompt、不解析模型自然语言、不调用 MCP、不自行更新数据库。
- mutating command 只在 `confirmed_by_user == true` 时传入 `ConfirmedByUser`；否则传入 `Missing` 并保持
  workflow 无变化。
- 成功结果仅投影版本化 Tool execution receipt；失败结果返回稳定、安全文本，不回显 raw arguments、Prompt、
  token、URL、API key 或内部错误细节。

### R3. Background lifecycle and recovery safety

- `execute_task` 增加 `novel_autopilot` 分支并复用既有 generic task lifecycle、cancellation registration、
  progress/terminal owner 与 task result 投影；不得创建第二套 task registry、task table、SSE kind 或 task state。
- `novel_autopilot` 必须登记为显式 `NonResumable` recovery policy：启动恢复只能终止并要求用户重新发起，
  不得从丢失 payload、确认或 Tool arguments 的 snapshot 自动重放。
- task 成功前后不复制 workflow CAS、权限或 transition matrix；所有业务写入继续经过 Tool Contract 和
  `novel_workflow_service`。

### R4. Compatibility and presentation contract

- 保持已有 `POST /api/background-tasks` path 和 JSON 字段兼容；不新增 Autopilot start/pause/resume/steer
  endpoint，不新增 migration，不新增 durable Tool audit store。
- 为既有前端 `BackgroundTaskType`/label 增加 `novel_autopilot`，仅保证任务中心能够安全展示已有 task，
  不在本任务新增 Autopilot 控制 UI 或入口。
- 新增/修改文本文件为 UTF-8 无 BOM、LF-only、无 trailing whitespace。

## Acceptance Criteria

- [x] `novel_autopilot` payload 对 Tool name、arguments、unknown fields、confirmation 和 project scope 实施严格验证。
- [x] task runner 的 actor/project 均从 `TaskRecord` 内部字段取得，payload 不能跨项目或注入 user ID。
- [x] confirmed happy path 仅通过 `autopilot_tool_contract_service` 和 canonical workflow service 修改项目状态；
  receipt 在 background task result 中可用。
- [x] missing confirmation、unknown Tool、invalid arguments、scope mismatch、unauthorized project 与 stale expected phase
  都返回安全失败，且相应 workflow state 不发生越权或错误变更。
- [x] `novel_autopilot` 使用既有 cancellation/task lifecycle，并拥有显式 `NonResumable` recovery policy；
  recovery 不自动执行 Tool。
- [x] 前端 task type/presentation 与 Rust executor/recovery owner contract 一致，未新增控制 UI。
- [x] `cargo fmt --check`、`cargo check`、focused task/coordinator/Tool/workflow tests、相关 production contract tests、
  frontend type/lint check（如可运行）与全量 Rust tests 通过。

### 验收证据（2026-07-16）

严格 payload、`TaskRecord` actor/project scope、confirmed Coordinator→Tool Contract→canonical
workflow transition、`NonResumable` recovery policy 和现有 task presentation 均已由
`implement.md:57-65`、`backend-rs/src/services/autopilot_coordinator_service.rs` 及
`backend-rs/src/api/background_tasks.rs` 覆盖。当前路线级 Rust 全量测试、前端 lint/build 均通过。
该任务只使用既有 cancellation/lifecycle，不引入自动恢复、控制 UI 或第二个 workflow owner。

## Out of Scope

- AI Provider 驱动的多轮计划、ToolChoice 强制、Prompt 模板、MCP 调用或从模型 `ToolCall` 直接执行。
- `novel_autopilot` 的公开 start/pause/resume/steer API、React 控制 UI、审批 UI、task center 交互改造。
- durable Autopilot payload、Tool audit 表、migration、checkpoint schema 扩展、自动恢复/replay、多 Tool 事务。
- 大纲/章节/审校 Tool、整书/多卷无人值守生成、G2 gate、R8 eval/metrics。

## Open Questions

无阻塞产品问题。根据用户已授予的“后续直接开发”授权，本任务按最小安全范围实施；任何需要新增
公开控制 API、migration 或 durable confirmation/audit 的需求必须创建后续独立任务并重新评审。
