# Implement: 模型 reasoning/content 双通道实时输出

## Ordered Checklist

1. [x] 盘点所有 `AIResponse` / `AIStreamChunk` 构造点与 Provider fixture，确定 additive 字段改动范围。
2. [x] 扩展 Rust AI 类型，并为不支持 reasoning 的 Provider 保持 `None` 兼容行为。
3. [x] 实现 OpenAI-compatible 完整响应与 stream `reasoning_content` 解析，补 mixed stream fixture。
4. [x] 实现 Anthropic 显式 thinking block/delta 解析；Gemini 保持安全空能力行为。
5. [x] 在 SSE owner 新增 `reasoning_chunk`，扩展直接生成流消费者，保证正文结果不混入 reasoning。
6. [x] 在后台任务 live event owner 实现有界、瞬时、不可 replay 的双通道输出桥。
7. [x] 扩展前端 SSE decoder，并以 `useModelOutputStream` / `useBackgroundTaskOutputStream` 统一状态归并。
8. [x] 抽取 `ModelOutputPanel`，接入 `SSEProgressModal` 与 `SSELoadingOverlay`，实现可选显示、tabs、自动滚动和截断提示。
9. [x] 接入项目向导、单章、章节再生成、批量章节、大纲、职业、角色、组织和世界观入口。
10. [x] 补 Rust reasoning 解析/隔离测试并更新验收文档；Playwright 真实模型流验收保留为部署环境手工门。
11. [x] 执行质量门并记录真实结果；Rust focused tests 因本机 MSVC `LNK1318` 未执行到断言。

## Implemented Contracts

### Provider / Rust

- `AIResponse.reasoning_content: Option<String>`
- `AIStreamChunk.reasoning_content: Option<String>`
- OpenAI-compatible 只读取明确的 `reasoning_content`
- Anthropic 只读取明确的 `thinking` / `thinking_delta`
- Gemini 不伪造 reasoning

### SSE

```json
{"type":"chunk","content":"正文增量"}
{"type":"reasoning_chunk","content":"显式推理增量"}
```

reasoning 不进入正文 accumulator、业务解析、任务快照、checkpoint 或持久化结果。

### Background Task Output Bounds

- `MAX_PENDING_TASK_OUTPUT_EVENTS = 256`
- `MAX_PENDING_TASK_OUTPUT_BYTES = 64 KiB`
- `MAX_MERGED_TASK_OUTPUT_BYTES = 8 KiB`
- 输出事件 best-effort、owner-scoped、不可 replay

### Frontend Lifecycle

- 两个通道各保留最近 50,000 字符
- localStorage 仅保存 UI 偏好，不保存模型文本
- 单章/向导使用 run token 隔离迟到事件
- 后台页面使用 task id 绑定事件；完成、失败、取消、轮询错误、项目切换时清理
- 世界观页已修复重复清理插入，每个终态恰好执行一次输出订阅清理

## Validation Results — 2026-07-19

```text
PASS  npm --prefix frontend run lint
      0 errors, 33 non-blocking existing warnings

PASS  npm --prefix frontend run build
      TypeScript and Vite production build completed

PASS  cargo fmt --manifest-path backend-rs/Cargo.toml -- --check

PASS  cargo check --manifest-path backend-rs/Cargo.toml -j 1
      37 unrelated unused/dead-code warnings

BLOCKED  cargo test --manifest-path backend-rs/Cargo.toml -j 1 reasoning -- --nocapture
         LINK : fatal error LNK1318: PDB LIMIT (12)
         Link failed before test assertions executed

NOT RUN  Playwright real-provider stream acceptance
         Requires running backend and valid provider credentials
```

## Review Gates

- [x] reasoning 从 Provider 到 UI 的每个边界都有 typed owner。
- [x] hidden CoT、Prompt、credential、tool args 不可到达 UI。
- [x] reasoning 不参与最终正文、章节保存或业务 result。
- [x] 直接 SSE 和后台任务均有真实代码链路。
- [x] 新字段/事件对旧客户端和不支持 Provider 向后兼容。
- [x] 服务端和浏览器内存均有明确上限。
- [x] 旧任务迟到事件和项目切换事件具有生命周期隔离。

## Remaining Environment Validation

1. 在可运行后端和真实 Provider credential 的环境执行手工验收矩阵。
2. 在解除 Windows PDB `LIMIT` 的构建环境重跑 `cargo test ... reasoning`。
3. 不应通过删除整个 `target`、`cargo fix`、reset/checkout 等破坏性方式绕过环境问题。

## Risky Files and Rollback Points

- `backend-rs/src/ai/types.rs`：新增字段为 additive；回滚时必须同步 Provider fixture 与所有结构体字面量。
- Provider parser：只允许明确字段，不能把普通 content 或内部诊断当 reasoning。
- `backend-rs/src/api/background_tasks.rs`：只允许瞬时事件 projection，不得写入任务持久事实。
- `frontend/src/components/SSEProgressModal.tsx` / `SSELoadingOverlay.tsx`：新 UI 默认关闭，旧调用不传 `modelOutput` 时保持原行为。
- 工作区存在大量其他改动；禁止 reset/checkout/覆盖非本任务内容。
