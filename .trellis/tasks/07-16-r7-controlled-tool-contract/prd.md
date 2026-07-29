# R7 受控 Tool Contract

## Goal

实现优化路线 R7 的第一条可执行纵切：定义一套由 Rust 服务端强制执行的受控业务
Tool Contract，并将首个写入型 Tool `transition_project_workflow` 映射到既有
`novel_workflow_service::transition`。该纵切必须验证身份边界、静态 allowlist、严格输入、
人工确认、状态机 CAS 和结构化结果；不得新增第二套 workflow、task 或 checkpoint 事实。

## User Value

- 后续 Autopilot Coordinator 可以调用稳定、可发现的业务 Tool，而不是拼接数据库更新或复用
  HTTP handler。
- 人工确认后的 workflow 推进仍由唯一的 Rust 状态机负责，避免模型或编排层覆盖人工修改。
- Tool 调用的失败类型可被 Coordinator 安全处理：未知 Tool、非法输入、缺少确认、无访问权限和
  phase 冲突均有明确边界。

## Confirmed Facts

- G1 已于 2026-07-16 通过；R7 已解锁，但本任务只实现 R7 的 Tool Contract 第一纵切。
- `novel_workflow_service::transition` 已拥有用户归属查询、合法 phase 转换、`expected_phase`
  比较与数据库条件更新（CAS）、清洗后的 tracing audit 和结构化 receipt。
- 既有 provider `ToolDef` / `ToolCall` 可承载模型可见 schema，但 `parameters: Value` 与
  JSON arguments 字符串不是服务端安全契约 owner。
- 当前没有通用 JSON Schema validator 依赖；本任务使用 `serde` typed DTO、
  `deny_unknown_fields`、显式非空/边界校验和手工稳定 JSON Schema 投影，不新增依赖。
- 现有 MCP plugin 调用与外部工具不属于本任务；它们的用户隔离/enable 检查不能替代业务 Tool
  的静态 allowlist 和领域服务权限边界。

## Requirements

### R1. Versioned controlled contract

- 新增内部 Rust owner，固定 schema version：`autopilot-tool-contract/v1`。
- 公开稳定 Tool 名称只包含 `transition_project_workflow`；未知名称必须在到达领域服务前拒绝。
- Contract 必须能投影成现有 AI `ToolDef`，但 `ToolDef` 仅是 provider transport DTO，不能成为
  直接执行入口。
- schema 投影必须是 object，声明 required 字段、字段类型、公共 workflow phase enum，且
  `additionalProperties` 为 false。

### R2. Typed invocation and authenticated scope

- Tool arguments 只允许：`project_id`、`expected_phase`、`target_phase`、可选 `reason` 与
  `related_task_id`。
- `user_id`、数据库表名、SQL、文件路径、Rust 函数名、prompt、API key 或任意原始执行配置
  不得作为 Tool 输入字段。
- actor `user_id` 必须来自内部 `AutopilotToolExecutionContext`，不得由模型或 Tool JSON 覆盖。
- 参数解析必须拒绝非 object、未知字段、未知 phase、空 project id 和无效 JSON；不得静默忽略。

### R3. Controlled mutating dispatch

- `transition_project_workflow` 标记为 mutating，执行前必须收到内部确认状态
  `ConfirmedByUser`；缺少确认时不得调用领域服务。
- dispatcher 必须将 typed input 映射为既有 `NovelWorkflowAuditContext` 与
  `novel_workflow_service::transition`；禁止直接更新 `projects.status`、复制 phase 转换表或
  调用 Axum route handler。
- `expected_phase` 必须是必填 CAS 前置条件。发生 stale phase 时返回可识别冲突，调用方必须
  重新读取 canonical state，禁止盲目重试。

### R4. Safe result and error boundary

- 成功结果必须是版本化、结构化的 `NovelWorkflowTransitionReceipt` 投影，至少包含
  `changed`、`previous_phase` 与 canonical state。
- 错误必须区分：unknown tool、invalid arguments、confirmation required、not found/access denied、
  stale phase、invalid transition 与 internal failure；外层不得返回数据库/内部实现细节。
- 仅记录安全的结构化 tracing 字段（Tool 名称、schema version、结果类别）；禁止记录 raw
  arguments、prompt、token、完整 URL、API key 或正文。

### R5. Compatibility and scope boundary

- 不修改已有 HTTP/JSON/SSE 字段，不新增公开 endpoint，不新增 migration，不新增依赖。
- 不创建 `novel_autopilot` task、Coordinator、pause/resume/steer、MCP dispatch、UI 或 durable
  Tool audit store；这些属于 R7 后续纵切。
- 不创建第二套 workflow phase、task store、business checkpoint、resume owner 或 cancellation
  durable state。
- 所有新增文本文件保持 UTF-8 无 BOM、LF-only、无 trailing whitespace。

## Acceptance Criteria

- [x] 新增 `autopilot-tool-contract/v1` 的静态 Contract registry，且仅暴露
  `transition_project_workflow`。
- [x] registry 能生成稳定 `ToolDef` JSON Schema；schema 约束 object、required fields、公共 phase
  enum 和 `additionalProperties: false`。
- [x] unknown Tool、非 object/无效 JSON、未知字段、缺少 required、空 project id、未知 phase 和
  试图提供 `user_id` 均在领域 service 前失败。
- [x] 未确认的 mutating invocation 被拒绝，且测试证明没有调用 workflow transition。
- [x] 确认后的合法 invocation 调用既有 `novel_workflow_service::transition`，保留 user/project
  access、合法转换、CAS、receipt 与 audit owner。
- [x] 无权限与 stale phase 的结果保留为安全、可区分的 Tool error；不得直接写数据库或泄露内部
  error detail。
- [x] 增加单元/数据库集成测试覆盖成功、确认拒绝、参数拒绝、未授权、stale phase 和 schema 投影。
- [x] `cargo fmt --check`、`cargo check`、focused tests、全量 Rust tests 通过；若默认 MSVC PDB
  linker 再现，按 G1 已验证的 `rust-lld + debuginfo=0` 仅作为环境规避运行测试。

## Out of Scope

- 完整基础设定→大纲→章节→审校 Autopilot loop。
- `novel_autopilot` 后台 task、Coordinator、task center、control endpoint、pause/resume/steer。
- 章节/大纲生成 Tool、MCP plugin Tool、跨 provider required tool choice 统一化。
- durable Tool execution audit 表或 migration；本纵切只复用既有 workflow tracing audit，不宣称
  已满足 R7 全量可查询审计目标。
- 自动 workflow phase 推进、重放、跨进程恢复、business checkpoint 复制。

## Open Questions

- 无阻塞产品问题。用户已明确授权后续直接开发；本任务按最小安全纵切实施。
- R7 后续 task 需单独决定 Coordinator 的 durable audit sink、后台任务控制面和跨 provider
  tool-choice 一致性（Gemini `Required` / named function 目前不等价）。

## 完成证据（2026-07-16）

- `cargo fmt --manifest-path backend-rs/Cargo.toml -- --check`：通过。
- `cargo check --manifest-path backend-rs/Cargo.toml`：通过；仅保留工作区既有 51 条 warning，R7 新模块未新增 warning。
- `cargo test --manifest-path backend-rs/Cargo.toml -j 1 autopilot_tool_contract_service -- --nocapture`：5/5 通过。
- `cargo test --manifest-path backend-rs/Cargo.toml -j 1 novel_workflow_service -- --nocapture`：17/17 通过。
- `cargo test --manifest-path backend-rs/Cargo.toml -j 1 --quiet`：1766/1766 通过。
- 测试进程使用 `rust-lld`、`debuginfo=0` 和 `/DEBUG:NONE` 规避本地 MSVC `LNK1318: PDB LIMIT (12)`；未修改产品构建配置。
- 新增/修改 R7 文件已通过 UTF-8 无 BOM、LF-only、无 trailing whitespace 检查。

R7 首个 Tool Contract 纵切完成；R7 的 Coordinator、`novel_autopilot` task、控制 API、人工 gate、审计、UI 和后续 Tool 仍未完成。
