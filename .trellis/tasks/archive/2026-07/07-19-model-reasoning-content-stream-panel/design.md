# Design: 模型 reasoning/content 双通道实时输出

## Architecture and Boundaries

### Canonical data flow

```text
Provider response
  -> provider adapter normalization
  -> AIResponse / AIStreamChunk
  -> generation service consumer
  -> direct SSE or background-task live-output projection
  -> frontend typed decoder/store
  -> optional output panel
```

每层只认识相邻层的 typed contract。正文与 reasoning 从 Provider adapter 开始即分离，最终正文保存逻辑只读取 `content`。

## Backend Contract

### AI types

为响应类型增加可选字段，并使用 serde default 保持旧 fixture/调用方兼容：

```rust
pub reasoning_content: Option<String>
```

`AIStreamChunk.content` 继续表示业务正文；`reasoning_content` 只表示 Provider 显式 reasoning delta。

### Provider adapters

- OpenAI-compatible：读取标准/兼容供应商明确返回的 `message.reasoning_content` 和 `delta.reasoning_content`；不存在则 `None`。
- Anthropic：仅识别明确的 thinking content block / thinking delta，不从普通 text 推断。
- Gemini：当前响应协议没有已确认的独立 reasoning 字段时保持 `None`；不得把普通 candidates text 当 reasoning。
- Tool call arguments 永远不进入 reasoning/content 预览通道。

### SSE contract

保留现有：

```json
{"type":"chunk","content":"正文增量"}
```

新增：

```json
{"type":"reasoning_chunk","content":"显式推理增量"}
```

事件构造由 `backend-rs/src/utils/sse.rs` 单一 owner 提供。reasoning 事件不参与最终正文累计。

### Background task live output

后台任务不能把完整输出写入 `message` 或最终 `result`。采用已有任务事件/运行态 owner 上的可选有界投影：

```text
live_output.reasoning_tail
live_output.content_tail
live_output.reasoning_truncated
live_output.content_truncated
live_output.updated_at / revision
```

约束：

- 仅 active task 暴露短生命周期 tail；
- 每通道固定字符上限，按 Unicode 字符安全裁剪；
- 完成后最终业务结果仍由原 result owner 提供；
- reasoning 不进入数据库业务实体；若现有任务快照为持久 owner，则只保留有界安全 tail，并在契约中明确非 canonical/replay 保证；
- 用户/项目 owner scope 沿用现有 background task 权限校验。

若代码证据表明现有 task event channel 可承载无持久化事件，则优先使用事件通道；否则在兼容 DTO 中增加有界 snapshot，禁止新增表。

## Frontend Contract

### Typed decoder

`SSEMessage` 增加 `reasoning_chunk`，`SSEClientOptions` 增加：

```ts
onReasoningChunk?: (content: string) => void;
```

正文 accumulator 和 reasoning accumulator 分离；Promise fallback 结果只使用正文 accumulator。

### Reusable output panel

扩展通用生成进度 UI，而不是逐页复制：

- “显示模型输出”开关，默认关闭；
- tabs：`思考过程`、`生成内容`；
- 每个通道独立累计、空态、截断提示；
- 自动滚动开关，默认开启；
- 用户主动向上滚动或关闭自动滚动后停止强制滚动；
- task/session id 变化时清空旧输出，防止串流；
- Provider 无 reasoning 时显示能力空态。

偏好仅保存 UI 开关/自动滚动，不保存模型输出内容。

## Compatibility

- 所有新增字段均 optional/default；旧 Provider、旧 fixture、旧 SSE consumer 正常工作。
- `chunk` 类型语义不变。
- 最终业务结果、数据库 schema、Prompt 和生成决策不变。
- 详细输出默认关闭，现有 UI 初始行为和性能基本不变。

## Security and Privacy

- allowlist 只接受 normalized reasoning/content string；
- 不转发 tool calls、request messages、System Prompt、diagnostics、credential 或 raw errors；
- 前后端均实施容量限制，防止内存放大；
- 后台 task owner scope 不变，禁止跨项目读取；
- 日志只记录事件类型/字符数/截断状态，不记录实际内容。

## Rollout and Rollback

- Feature 通过默认关闭的 UI 开关渐进启用。
- 协议为 additive，可独立回滚 UI 而不影响后端正文流。
- 若某 Provider reasoning 格式不稳定，仅禁用该 adapter 的 reasoning 解析，正文链路不受影响。
- 不涉及 migration，回滚无需数据处理。

## Test Strategy

1. Provider JSON/SSE fixtures：reasoning-only、content-only、mixed、tool-call、empty、done。
2. SSE contract：事件类型、转义、正文 accumulator 隔离。
3. Background task：owner scope、容量裁剪、task 切换、terminal state。
4. Frontend unit/component：默认关闭、tabs、auto-scroll、empty/truncated state。
5. E2E：至少覆盖一个直接 SSE 创作入口和一个后台任务入口。
