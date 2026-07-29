# Design: Role Model Policy 与 Generation Execution Audit

## 1. Architecture Boundary

R5 在 R4 Generation Contract 与现有 AI runtime 之间增加两个独立 owner：

```text
GenerationContractSnapshotV1
  -> GenerationIntentKind
  -> role_model_policy_service
       GenerationRole
       RoleModelPolicyV1
       ResolvedRoleModelPolicyV1
       policy_digest
  -> AIConfig（仍由 SettingsService 构建密钥/base URL）
  -> AIService tracked execution
       AIExecutionTraceV1（仅脱敏 transport/model 事实）
  -> generation_execution_audit_service
       GenerationExecutionAuditV1
  -> existing history payload additive key
```

R4 继续拥有 Story Packet、Generation Intent 与 `input_digest`；R5 不修改 digest 输入。R6 后续拥有
checkpoint，不能复用 audit 充当恢复事实。

## 2. Schema Owners

### 2.1 GenerationRole

在新增 `backend-rs/src/services/role_model_policy_service/` 中定义：

```rust
pub enum GenerationRole {
    Planner,
    Writer,
    Reviewer,
}
```

提供 `GenerationRole::from_intent(GenerationIntentKind)` 的穷举 match。因为 enum 当前封闭，新增
intent 时编译器和测试会迫使更新映射。

### 2.2 RoleModelPolicyV1

```rust
pub const ROLE_MODEL_POLICY_SCHEMA_VERSION: &str = "role-model-policy/v1";

pub struct RoleModelSelectionV1 {
    pub provider: Option<String>,
    pub model: Option<String>,
}

pub struct RoleModelPolicyV1 {
    pub schema_version: String,
    pub roles: BTreeMap<GenerationRole, RoleModelSelectionV1>,
}
```

策略持久化位于 `settings.preferences.role_model_policy`。preferences helper 解析顶层 object，替换单个
key 后重新序列化，保留其他键。API Key、base URL、backup URLs、headers 不属于该 schema。

canonical digest 复用 R4 的 canonical JSON/sha256 模式，但由 R5 独立函数和 schema version 管理；
不得把 R5 policy 合并进 R4 contract。

### 2.3 Resolution Result

```rust
pub enum ModelSelectionSource {
    RouteOverride,
    RoleOverride,
    GlobalSettings,
    ProviderDefault,
}

pub struct ResolvedRoleModelPolicyV1 {
    pub role: GenerationRole,
    pub policy_schema_version: String,
    pub policy_digest: String,
    pub requested_provider: Option<String>,
    pub requested_model: Option<String>,
    pub resolved_provider: String,
    pub resolved_model: String,
    pub provider_source: ModelSelectionSource,
    pub model_source: ModelSelectionSource,
}
```

`requested_*` 只代表 route 显式输入；role/global/default 通过 source 表达。所有字符串 trim，provider
小写规范化，空字符串视为 absent。

## 3. Settings Integration

新增 helper：

- `read_role_model_policy(preferences)`
- `set_role_model_policy(preferences, policy)`
- `load_role_model_policy(db, user_id)`
- `build_role_aware_ai_config(...)`

`SettingsService::build_ai_config` 保持签名和旧调用者行为。role-aware builder 先加载完整 settings，
解析策略和优先级，然后复用同一密钥/base URL/max token/temperature 构建逻辑。为避免 DRY，现有
builder 的模型构造部分抽为私有 owner，但不改变公开结果。

`api_backup_urls` 应按现有 settings 恢复到 `AIConfig.backup_urls`；`fallback_strategy` 只决定是否
允许自动模型 fallback，不把原始值写入 audit。若此行为会改变旧调用者，则仅在 role-aware builder
启用，旧 builder 保持当前兼容输出。

## 4. Tracked AI Execution

### 4.1 Non-stream

不向现有 `AIResponse` 强塞 generation role。新增：

```rust
pub struct TrackedAIResponse {
    pub response: AIResponse,
    pub execution: AIExecutionTraceV1,
}
```

`AIService::generate_text_tracked*` 复用现有 client 调用，成功时记录实际 provider/model；模型
fallback 成功时记录 requested model、actual fallback model、分类原因。旧 `generate_text*` 通过
tracked 内核或原路径返回 `AIResponse`，保持签名。

### 4.2 Stream

新增 tracked stream wrapper，保留旧 chunk 和外层 SSE kind：

```rust
pub struct TrackedAIStream {
    pub stream: ReceiverStream<Result<AIStreamChunk, String>>,
    pub completion: oneshot::Receiver<AIExecutionTraceV1>,
}
```

AIService 在 primary/fallback 终态完成 trace。OpenAI endpoint 成功摘要只能输出 allowlist：
`endpoint_role`、`endpoint_index`、`total_attempts`、`failover_count`、`backup_endpoint_used`。
绝不复制 `effective_base_url`、`effective_endpoint` 或 raw diagnostics。

Anthropic/Gemini 暂无 endpoint list 时记录 `endpoint_failover=None`，不能伪造成功 failover。

## 5. Fallback Taxonomy

```rust
pub enum AIExecutionFallbackKind {
    None,
    ModelFallback,
    EndpointFailover,
    CandidateExecutorFallback,
}
```

一个执行可同时出现 model fallback 与 endpoint failover，因此持久化使用有序集合/flags，而不是
单一字符串。candidate executor fallback 由 chapter candidate gateway 显式加入，AIService 不推断
Rust/Python 路由。

reason 只保存稳定分类，例如 `model_not_found`、`model_unavailable`、`primary_endpoint_failed`、
`candidate_executor_failed`，不保存包含 URL、Prompt 或 provider 原始响应的错误字符串。

## 6. Generation Execution Audit

```rust
pub const GENERATION_EXECUTION_AUDIT_SCHEMA_VERSION: &str =
    "generation-execution-audit/v1";

pub struct GenerationExecutionAuditV1 {
    pub schema_version: String,
    pub role: GenerationRole,
    pub policy_schema_version: String,
    pub policy_digest: String,
    pub requested_provider: Option<String>,
    pub requested_model: Option<String>,
    pub resolved_provider: String,
    pub resolved_model: String,
    pub actual_provider: String,
    pub actual_model: String,
    pub provider_source: ModelSelectionSource,
    pub model_source: ModelSelectionSource,
    pub fallbacks: Vec<AIExecutionFallbackSummaryV1>,
    pub endpoint_summary: Option<EndpointExecutionSummaryV1>,
}
```

audit 由 typed fields 构建，禁止接收任意 JSON merge。章节 history additive key：

```json
{
  "generation_contract": { "...": "R4 summary" },
  "generation_execution_audit": { "schema_version": "generation-execution-audit/v1" }
}
```

旧 history 缺 key 返回 `Ok(None)`；未知 schema 返回 typed error，不影响旧 R4 summary 读取。

## 7. Entry Integration Order

1. 单章 direct candidate + history persistence。
2. 批量生成/恢复复用相同 prepared execution policy。
3. 全章/局部重生成。
4. outline generate/expand。
5. review/repair/analysis。

共享的 `runtime_execution_owner.rs` 当前有大规模 R4 diff；接线必须使用最小函数参数或新增 sibling
owner，不做整文件重写。

## 8. Compatibility

- 公开 request/response 字段只做 additive（如果本阶段暴露设置 API），旧字段不变。
- 不新增 SSE event kind；tracked stream metadata 只供后端 audit 使用。
- 旧 AIService 方法签名不变。
- 旧 history JSON 和旧 snapshot 缺 audit 时继续可读。
- R4 `compute_input_digest` 测试增加 policy/provider/model 变化不影响摘要的保护。

## 9. Security

允许进入 audit 的 transport 字段只有：

```text
endpoint_role
endpoint_index
total_attempts
failover_count
backup_endpoint_used
```

显式拒绝字段：

```text
api_key
authorization
headers
base_url
effective_base_url
effective_endpoint
prompt
messages
content
response_body
```

日志、测试 fixture 和 error message 同样执行该边界。

## 10. Rollback

- role policy 缺失或解析失败时回到全局 settings，并记录安全的 policy parse classification。
- tracked API 接线可逐入口回退到旧 API；旧 API 始终保留。
- history additive key 可停止写入，不需要 schema downgrade。
- 不涉及数据库 migration 或生产 downgrade。
