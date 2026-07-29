# 模型思考过程与生成内容实时输出：实现与验收

> 日期：2026-07-19
> 关联任务：`.trellis/tasks/07-19-model-reasoning-content-stream-panel`
> 关联路线：`docs/15-ainovel-cli-comparison-and-mumunovel-optimization.zh-CN.md`
> 性质：独立的可观察性补强；后续被 R9 Durable Workbench 复用，但自身不成为业务事实 owner

## 1. 用户可见目标

MuMuNovel 的长时间 AI 生成流程不再只能显示“运行中、百分比、当前阶段”。用户可在生成进度界面中选择打开“显示模型输出”，分别查看：

1. **生成内容**：模型正在输出的正文、设定、大纲、角色、组织、职业等业务文本；
2. **思考过程**：Provider API 明确返回的 reasoning/thinking 文本。

该面板默认关闭，不影响原有简洁进度界面。用户打开后可在两个标签间切换，并可选择是否自动滚动。

## 2. 安全边界

本功能只展示上游模型接口明确返回、并经 Provider adapter 识别的文本字段：

- OpenAI-compatible：`reasoning_content` 与正文 `content`；
- Anthropic：明确的 `thinking` content block / `thinking_delta`；
- Gemini：当前不伪造 reasoning；正文仍正常输出。

本功能**不提取、不推断、也不声称展示模型隐藏 chain-of-thought**。以下内容不会进入输出面板：

- System Prompt、用户请求原文或完整消息数组；
- API Key、Provider credential；
- 原始 tool arguments；
- transport diagnostics、内部堆栈或未脱敏错误；
- Anthropic `redacted_thinking`。

## 3. 端到端数据流

```text
Provider stream
  ├─ reasoning/thinking delta
  │    -> AIStreamChunk.reasoning_content
  │    -> SSE type = reasoning_chunk
  │    -> reasoningContent（独立累计）
  │
  └─ content delta
       -> AIStreamChunk.content
       -> SSE type = chunk
       -> generatedContent（独立累计）
```

直接 SSE 和后台任务使用相同的前端事件语义：

```json
{"type":"chunk","content":"生成正文增量"}
{"type":"reasoning_chunk","content":"Provider 明确返回的推理增量"}
```

后台任务的内容事件是 owner-scoped、短生命周期、best-effort 的实时预览：

- 断线后任务仍由 HTTP polling 继续跟踪；
- 不提供历史 replay；
- 不写入 background task store、checkpoint 或任务快照；
- 任务正式结果仍由原有 result/status 路径提交。

## 4. 正文与 reasoning 隔离保证

reasoning 与生成内容在每一层都分开：

1. Rust 类型使用独立可选字段；
2. SSE 使用独立事件类型；
3. `SSEClient` 只把 `chunk` 加入正文 accumulator；
4. `reasoning_chunk` 只调用 `onReasoningChunk`；
5. UI 使用独立的 `reasoningContent` / `generatedContent` 状态；
6. 单章生成和项目向导使用 run/request token，旧任务的迟到事件不能串入新任务；
7. reasoning 不参与章节、大纲、角色、组织、职业、世界观等正式业务结果解析和保存。

因此，开启或关闭输出面板都不会改变最终生成内容和数据库写入规则。

## 5. 容量与隐私控制

### 浏览器

- reasoning 通道最多保留最近 **50,000 个字符**；
- content 通道最多保留最近 **50,000 个字符**；
- 超限后 UI 显示截断提示；
- localStorage 只保存“是否显示”和“是否自动滚动”偏好；
- 实际模型输出不写入 localStorage，刷新后不会恢复。

### Rust 后台任务桥接

- pending transient output 总量上限：**64 KiB**；
- 待发送事件数量上限：**256**；
- 相邻同类型片段可在不超过 **8 KiB** 时合并；
- 队列只存在于内存，终态前 flush，绝不进入持久化任务记录。

## 6. 已接入范围

### 直接 SSE

- 项目创建向导：世界观、职业体系、角色、完整大纲；
- 单章生成；
- 章节再生成；
- AI Test 等复用 typed SSE client 的调用。

### 后台任务

- 大纲生成；
- 职业体系生成；
- 角色生成；
- 组织生成；
- 世界观重新生成；
- 章节批量生成。

页面切换、项目切换、任务切换、任务终态、主动取消和组件卸载时会关闭旧订阅，避免不同项目或不同任务串流。

## 7. UI 行为

- “显示模型输出”默认关闭；
- 默认标签为“生成内容”；
- 可切换“生成内容 / 思考过程”；
- 自动滚动默认开启；
- 用户向上滚动后自动停止强制跟随；
- Provider 未返回 reasoning 时显示：`当前模型未返回可展示的推理内容`；
- 任务失败或取消时，面板标注这些内容为“未提交输出”；
- 非阻塞浮层折叠时只保留简洁任务条，不暴露大段模型文本；展开后再显示双通道面板。

## 8. 如何判断是否成功开发

### A. 协议验收

1. 选择支持 reasoning 的 OpenAI-compatible 或 Anthropic 模型；
2. 发起单章生成、章节再生成或后台生成任务；
3. 浏览器 Network 中应看到独立的 `chunk` 与 `reasoning_chunk`；
4. `reasoning_chunk` 内容不得出现在最终正文 accumulator 或保存结果中。

### B. 页面验收矩阵

| 场景 | 入口 | 预期结果 |
|---|---|---|
| 项目创建向导 | 新建项目并依次生成世界观、职业、角色、大纲 | 面板可选显示；每一阶段开始时清空旧输出；正文和推理分别追加 |
| 单章生成 | 创作管理 → 章节 → 生成单章 | 生成期间浮层持续存在；正文实时追加；任务结束后才关闭 |
| 章节再生成 | 章节编辑/再生成 | 请求切换或取消后，旧请求输出不得继续写入 |
| 批量章节生成 | 批量生成进度浮层 | 展开浮层后可查看双通道输出；折叠条不展示文本 |
| 大纲/职业 | 对应创作管理页面发起后台生成 | 输出绑定当前 task id；终态和取消后关闭订阅 |
| 角色/组织 | 角色管理页面分别发起生成 | 两类任务均显示具体生成内容；项目切换后不得串流 |
| 世界观 | 世界设定页面重新生成 | 恢复任务可重新绑定实时事件；完成、失败、取消和轮询错误均清理订阅 |

### C. 通用交互验收

1. 打开生成进度界面，默认只看到原有进度；
2. 打开“显示模型输出”；
3. “生成内容”标签随模型输出实时追加具体文本；
4. 支持 reasoning 的 Provider 在“思考过程”标签实时追加明确返回的推理文本；
5. 关闭自动滚动或向上滚动后，界面不再强制跳到底部；
6. 开始第二个任务时，不能继续显示第一个任务的输出。

### D. 不支持 reasoning 的 Provider

1. 使用 Gemini 或未返回 reasoning 的模型；
2. 正文必须正常生成；
3. “思考过程”显示能力空态，而不是报错或伪造内容。

### E. 隔离与持久化验收

1. 生成结束后核对正式章节、大纲、职业、角色、组织和世界观数据，只包含业务结果；
2. reasoning 不得出现在任务 message、result、checkpoint、数据库记录或日志正文中；
3. 刷新页面后，开关偏好可保留，但上一次模型输出文本不得恢复；
4. SSE 连接中断时，任务状态轮询应继续，正式结果仍可完成。

## 9. 关键实现位置

### 后端

- `backend-rs/src/ai/types.rs`
- `backend-rs/src/ai/clients/openai.rs`
- `backend-rs/src/ai/clients/anthropic.rs`
- `backend-rs/src/ai/clients/gemini.rs`
- `backend-rs/src/utils/sse.rs`
- `backend-rs/src/api/background_tasks.rs`
- `backend-rs/src/tasks/types.rs`
- `backend-rs/src/services/chapter_candidate_output_service.rs`
- `backend-rs/src/services/wizard_service.rs`

### 前端通用层

- `frontend/src/utils/sseClient.ts`
- `frontend/src/hooks/useModelOutputStream.ts`
- `frontend/src/hooks/useBackgroundTaskOutputStream.ts`
- `frontend/src/components/ModelOutputPanel.tsx`
- `frontend/src/components/SSEProgressModal.tsx`
- `frontend/src/components/SSELoadingOverlay.tsx`

### 前端接入层

- `frontend/src/components/AIProjectGenerator.tsx`
- `frontend/src/components/ChapterRegenerationModal.tsx`
- `frontend/src/components/ChapterBatchProgressEntry.tsx`
- `frontend/src/components/SingleChapterGenerationOverlayEntry.tsx`
- `frontend/src/pages/Chapters.tsx`
- `frontend/src/pages/chapterSingleGenerationHelpers.ts`
- `frontend/src/pages/Outline.tsx`
- `frontend/src/pages/Careers.tsx`
- `frontend/src/pages/Characters.tsx`
- `frontend/src/pages/WorldSetting.tsx`

## 10. 质量门记录（2026-07-19）

| 检查 | 结果 | 说明 |
|---|---|---|
| `npm --prefix frontend run lint` | 通过 | 0 errors；33 条现有 Hook warnings，不阻断 |
| `npm --prefix frontend run build` | 通过 | TypeScript 与 Vite 生产构建通过；存在既有 circular chunk warning |
| `cargo fmt --manifest-path backend-rs/Cargo.toml -- --check` | 通过 | 无格式差异 |
| `cargo check --manifest-path backend-rs/Cargo.toml -j 1` | 通过 | 37 条其他优化模块的 unused/dead-code warnings |
| `cargo test --manifest-path backend-rs/Cargo.toml -j 1 reasoning -- --nocapture` | 环境阻塞 | MSVC 链接阶段 `LNK1318: PDB LIMIT (12)`，未执行到断言 |
| Playwright 真实模型流验收 | 未自动执行 | 需要可用后端、Provider credential 和支持 reasoning 的真实模型环境 |

Rust focused test 的失败不能记作断言失败，也不能记作测试通过。禁止通过删除整个 `target`、执行 `cargo fix` 或破坏工作区来绕过链接器限制。

## 11. 当前结论

双通道功能已经完成代码接入并通过静态编译质量门。是否在目标部署环境“最终成功”，应同时满足：

1. 真实 Provider 能返回显式 reasoning 字段；
2. Network 可观察到 `reasoning_chunk` 与 `chunk` 分离；
3. 页面能实时显示两类输出；
4. 最终业务数据中不包含 reasoning；
5. 任务切换、取消和项目切换后无旧输出串流。

## 12. R9 Durable Novel Autopilot 集成边界

R9 自动成书工作台复用本双通道面板显示模型调用过程，但必须区分三类数据：

1. **运行状态**：Run/Step/Background Task 的阶段、状态、预算和质量决策，可持久恢复。
2. **生成内容**：Provider 返回的业务正文或结构化结果，只有通过既有业务提交门后才进入项目数据。
3. **显式 reasoning/thinking**：仅当 Provider API 明确返回时进入 `reasoning_chunk`，只保留在当前前端内存窗口。

Durable Run 重启恢复的是状态和业务 checkpoint，不恢复或回放 reasoning。用户关闭“显示模型输出”
只影响界面可见性，不影响自动成书执行、章节提交、质量门或最终导出。系统不得把日志、Prompt、
调度解释或模型隐藏思维链伪装成 reasoning。

2026-07-19 的完整成书 Smoke 已验证 R9 的 Pause/Restart/Resume、章节返修、全书润色和真实导出；
该确定性 Mock Provider 验证的是编排与双通道边界，不替代“真实 Provider 显式 reasoning 字段”验收。
真实 reasoning 最终验收仍按第 8、10、11 节执行。
