# PRD: 模型思考与生成内容实时输出面板

## Goal and User Value

为 MuMuNovel 的 AI 生成流程提供可选的实时模型输出视图，使用户能够像 ainovel-cli 一样区分查看：

1. Provider API 明确返回的 reasoning/thinking 内容；
2. 模型正在生成的正文、设定、大纲、角色等业务内容。

该功能用于提升长时间生成任务的可观察性和用户信任，不改变最终业务数据的提交规则。

## Confirmed Facts

- Rust `AIResponse` / `AIStreamChunk` 当前只有正文 `content`，没有 reasoning 字段。
- OpenAI-compatible、Anthropic、Gemini 客户端当前只投影正文或工具调用。
- SSE 当前使用 `type: "chunk"` 传输正文，前端 `sseClient` 只提供 `onChunk`。
- 创作页面大量复用 `SSEProgressModal`；后台任务主要通过 `progress/message/result` 和任务 SSE/轮询展示状态。
- ainovel-cli 将 thinking delta 与 text delta 分通道展示，并维护独立累计内容。
- 用户已明确要求显示模型显式返回的思考内容和具体生成内容，而不只是任务状态。
- 用户已授权按既定优化路线继续直接开发。

## Requirements

1. 保留现有 `chunk` 正文事件，新增独立、typed 的 reasoning 事件，不破坏现有消费者。
2. 仅展示 Provider 明确返回的 reasoning/thinking/summary；不得推断或伪造隐藏 chain-of-thought。
3. Provider 不支持 reasoning 时必须正常生成正文，并在 UI 中显示“当前模型未返回可展示的推理内容”。
4. 直接 SSE 与后台生成任务都应能够投影实时 reasoning 和正文预览；不能只覆盖 AI 测试接口。
5. 详细输出默认关闭，用户可选择开启；偏好保存在前端本地设置中。
6. reasoning 与正文使用独立 accumulator；reasoning 不得混入最终正文或正式业务保存结果。
7. 实时预览必须有容量上限，避免长篇生成无限占用服务端或浏览器内存。
8. 输出面板支持自动滚动并允许用户关闭自动滚动；切换页面或任务时不能串流。
9. 不展示 System Prompt、API key、Provider credential、原始 tool arguments、未脱敏错误或内部诊断。
10. 不新增数据库 migration/schema，不建立第二份 task/workflow/audit 事实 owner。

## Acceptance Criteria

- [ ] OpenAI-compatible fixture 中的 `delta.reasoning_content` 可被解析为 reasoning chunk，正文仍解析为 content chunk。
- [ ] Anthropic 仅在响应包含明确 thinking content block/delta 时输出 reasoning；Gemini 不支持时返回空 reasoning 而不影响正文。
- [ ] Rust `AIStreamChunk`/`AIResponse` 的新增字段为可选并具备向后兼容默认值。
- [ ] SSE `reasoning_chunk` 与现有 `chunk` 分离，旧 `chunk` 客户端行为保持不变。
- [ ] 前端 `SSEClientOptions` 提供 `onReasoningChunk`，reasoning 不进入正文累计结果。
- [ ] 通用生成进度 UI 提供“显示模型输出”开关和“思考过程/生成内容”双视图。
- [ ] 默认关闭详细输出；开启后可实时滚动，关闭自动滚动后不会强制跳到底部。
- [ ] 每个通道执行明确的字符容量裁剪，并在裁剪后给出提示。
- [ ] 后台任务链路能够安全投影有界实时预览，任务失败/取消时标识为未提交输出。
- [ ] 不支持 reasoning、空 chunk、混合 reasoning/content、任务切换、失败和取消路径均有测试。
- [ ] `cargo fmt/check/focused tests`、前端 `lint/build` 和目标 E2E 通过。

## Out of Scope

- 提取或展示模型未通过 API 返回的隐藏 chain-of-thought。
- 保存 reasoning 到章节、设定、角色或其他业务数据库表。
- 展示 System Prompt、Tool 原始参数、凭据或内部 transport diagnostics。
- 为 replay 历史完整 reasoning 新增持久化表或对象存储。
- 修改模型生成质量、Prompt 内容或自动化工作流决策逻辑。

## Open Questions

无阻塞问题。默认采用“详细输出关闭、仅短生命周期有界预览、不持久化 reasoning”的兼容方案。
