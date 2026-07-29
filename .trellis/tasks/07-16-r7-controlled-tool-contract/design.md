# Design: R7 Controlled Tool Contract First Slice

## 1. Design Decision

首个受控 Tool 采用 `transition_project_workflow`，而不是直接从大纲或章节生成切入。
它是同步、输入输出稳定的写入能力，已由唯一的 `novel_workflow_service` owner 承担权限、
转换表、CAS 和审计；因此能够先验证 Tool Contract 的安全边界，而不提前把 provider、后台
任务、checkpoint、恢复和取消混入同一变更。

```text
Provider ToolDef / ToolCall (transport only)
                |
                v
Autopilot Tool Contract v1 (static registry + typed validation)
                |
                v
AutopilotToolExecutionContext (authenticated actor + confirmation)
                |
                v
transition_project_workflow adapter
                |
                v
novel_workflow_service::transition (唯一领域 owner)
                |
                v
NovelWorkflowTransitionReceipt + existing transition audit
```

## 2. Module Boundary

新增 `backend-rs/src/services/autopilot_tool_contract_service.rs`，由两个 focused owner 组成：

```text
autopilot_tool_contract_service.rs
├─ schema_owner.rs      # stable Tool metadata, ToolDef projection, typed DTO, validation
├─ dispatch_owner.rs    # authenticated invocation, confirmation gate, domain service adapter
└─ tests.rs             # contract and DB-backed dispatch tests
```

`backend-rs/src/services/mod.rs` 只注册该 service module。不得修改 API route、tasks registry、
MCP 或 database schema。

## 3. Stable Contract

### 3.1 Registry

- contract schema version：`autopilot-tool-contract/v1`。
- v1 registry 是编译期静态 allowlist，只包含 `transition_project_workflow`。
- Tool metadata 包含：stable name、description、input schema、side effect（`mutating`）和
  confirmation policy（`required`）。
- `autopilot_tool_definitions()` 将 metadata 投影为现有 `crate::ai::types::ToolDef`。模型可见
  schema 不能绕过 dispatcher；所有实际执行仍从 registry 名称查找。

### 3.2 Typed input

```rust
#[serde(deny_unknown_fields)]
struct TransitionProjectWorkflowArgs {
    project_id: String,
    expected_phase: NovelWorkflowPhase,
    target_phase: NovelWorkflowPhase,
    reason: Option<String>,
    related_task_id: Option<String>,
}
```

- JSON input 必须是 object；`serde` 处理 required、unknown field 和 enum。
- adapter 显式拒绝空白 `project_id`；`reason` / `related_task_id` 继续交给既有
  `NovelWorkflowAuditContext::sanitized()` 执行统一清洗/长度策略，避免复制规则。
- `user_id` 没有 DTO 字段，因 `deny_unknown_fields` 被拒绝。
- JSON Schema 是手工构造的稳定 `serde_json::Value`：`type=object`、three required fields、
  public phase enum 和 `additionalProperties=false`。不引入通用 JSON Schema 依赖。

### 3.3 Execution context and confirmation

```rust
struct AutopilotToolExecutionContext<'a> {
    actor_user_id: &'a str,
    confirmation: AutopilotToolConfirmation,
}

enum AutopilotToolConfirmation {
    Missing,
    ConfirmedByUser,
}
```

context 仅由未来 authenticated Coordinator/control owner 创建；工具 JSON 和 provider response
不能写入或覆盖 actor。`transition_project_workflow` 在 confirmation 不是
`ConfirmedByUser` 时立即返回 `ConfirmationRequired`，不会触碰数据库/领域 service。

## 4. Dispatch and Error Mapping

`dispatch_autopilot_tool(db, context, tool_name, arguments)` 的固定顺序：

1. 检查 static allowlist，未知 Tool 返回 `UnknownTool`；
2. 解析并校验 object / typed input；失败返回 `InvalidArguments`，不调用领域 owner；
3. 检查 metadata 的 confirmation policy；失败返回 `ConfirmationRequired`；
4. 构造 `NovelWorkflowAuditContext`，调用 `novel_workflow_service::transition`；
5. 将领域 receipt 投影为 `AutopilotToolExecutionResultV1`；
6. 将领域错误映射为不含内部 detail 的 `ToolContractError`。

建议错误语义：

| Contract error | Source | Caller action |
| --- | --- | --- |
| `UnknownTool` | static registry miss | fail closed，不重试 |
| `InvalidArguments` | JSON/DTO/explicit boundary | 修正 call，不访问领域 service |
| `ConfirmationRequired` | mutating metadata | 请求人工确认 |
| `NotFoundOrAccessDenied` | workflow owner | 以统一 not-found/access 错误停止 |
| `StaleExpectedPhase` | workflow CAS | 重读 canonical workflow state |
| `InvalidTransition` | workflow owner | 请求人工决策/修正 target |
| `Internal` | workflow owner or serialization | 记录安全类别，按上层 retry policy 决定 |

现阶段不建立 HTTP 映射；未来 API owner 才可把这些 typed errors 映射成公开状态码。

## 5. Result and Audit Boundary

```text
AutopilotToolExecutionResultV1
- schema_version: autopilot-tool-contract/v1
- tool_name: transition_project_workflow
- output: NovelWorkflowTransitionReceipt
```

`NovelWorkflowTransitionReceipt` 保留 `changed`、`previous_phase` 和 canonical state。
Tool dispatcher 仅发出不含 raw arguments 的 `tracing::info!` outcome；写入型 workflow 的既有
`emit_transition_audit` 仍是领域 audit owner。该 tracing 不是 durable/queryable Tool audit，
因此本任务不宣称已经完成路线要求的全量 Tool audit；后续 Coordinator task 必须补齐该能力。

## 6. Compatibility, Operations and Rollback

- 仅新增内部 service 与 module registration；既有 HTTP/JSON/SSE/task schemas 不变。
- 无 migration、无新依赖、无 provider client 修改；Gemini tool-choice 不等价问题保持为后续风险。
- 该 Contract 不直接持久化 workflow、task、checkpoint 或 cancellation state。
- 回滚只需移除该新 service module 与 registration；`novel_workflow_service` 和已有 API 继续工作。
- 如果后续新增工具，每个工具必须先加入 static metadata、typed DTO、手工 schema、confirmation
  policy、domain adapter 和专项测试；不得接受自由工具名或泛型 SQL/function payload。
