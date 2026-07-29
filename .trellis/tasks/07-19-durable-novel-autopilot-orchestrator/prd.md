# Durable Novel Autopilot Orchestrator

## Goal

在不改变现有 R7 `novel_autopilot` 单次受控 Tool 调用契约的前提下，为 MuMuNovel 新增小说级、可持久恢复的自动创作编排器。用户可以从现有项目或向导阶段启动，自动串联基础设定、世界观、职业、角色、组织、大纲、逐章写作、质量闭环、全书审查、润色、完结判定和导出，最终得到一本结构完整、章节已落库的小说。

## Product Boundary

- 现有 `novel_autopilot` 保持单次确认、单工具、不可恢复；新能力使用独立的 Durable Run 与独立后台任务类型，避免破坏 R7 安全契约。
- Durable Run 只拥有“流程推进、步骤执行记录、预算和人工门”事实；项目、章节、大纲、角色、组织、质量结果和导出文件仍由现有业务模型/服务拥有。
- 模型正文与 Provider 明确返回的 reasoning/thinking 继续通过现有双通道输出展示；reasoning 不写入项目业务数据或 Durable Run 持久化记录。
- 首个版本只复用系统当前支持的模型 Provider 和生成配置，不引入新的模型协议或外部队列。

## Requirements

### 1. Run lifecycle

- 用户可为自己拥有的项目创建 Durable Novel Autopilot Run。
- 同一项目同一时刻最多一个活动 Run；重复创建返回当前活动 Run，而不是并发生成两本内容。
- Run 支持 `queued`、`running`、`waiting_human`、`paused`、`completed`、`failed`、`cancelled` 状态。
- 用户可查询 Run 列表/详情、暂停、恢复、取消，并可在 `waiting_human` 状态提交接受、重试、修复或停止决定。
- 服务重启后，`queued`/`running` Run 必须从持久化步骤恢复；旧执行迟到结果不得覆盖新 attempt、暂停或取消后的状态。

### 2. Execution scopes

启动时支持以下范围：

- `planning_only`：补齐基础资料并生成到章节大纲，不生成正文。
- `next_n_chapters`：从当前可写章节开始生成指定数量章节。
- `continue_from_current`：跳过已完成章节，从第一个未完成章节继续到已有大纲末尾。
- `complete_book`：补齐全部前置资料，生成全部章节，并执行全书收尾流程。

### 3. Automated planning flow

当项目资料缺失且范围需要时，编排器按依赖顺序复用现有服务：

1. 校验/补齐项目基础设定；
2. 生成世界观；
3. 生成职业体系；
4. 生成主要角色；
5. 生成组织；
6. 生成分卷/章节大纲；
7. 将 Novel Workflow 推进到可写阶段。

已存在且有效的资料默认跳过；用户可通过配置选择重建，但默认不得覆盖人工编辑内容。

### 4. Chapter loop and quality closure

- 按章节号顺序准备现有 Generation Contract / Story Packet / Role Model Policy。
- 每章只通过现有章节生成服务创建候选稿和提交结果，不创建第二套章节存储。
- 每章生成后执行现有分析/质量门，得到 `accept`、`auto_repair`、`retry` 或 `manual_review` 决策。
- `auto_repair` 和 `retry` 受单章最大尝试次数限制；超过限制进入人工门或失败，不能无限循环。
- 接受章节后更新 completed chapters、总字数、质量趋势、摘要/故事记忆/伏笔等现有派生数据。
- 用户可配置人工门：`fully_automatic`、`high_risk_only`、`every_n_chapters`、`every_volume`、`every_chapter`。

### 5. Budget and stopping policy

Run 配置至少支持：

- 最大生成章节数；
- 最大总 Token（有 Provider usage 时按实际值，无时按项目既有估算策略）；
- 最大估算成本；
- 最大运行时长；
- 单章最大生成/返修尝试次数；
- Provider 连续失败上限；
- 质量连续不合格上限。

在启动模型调用前和步骤提交后都检查限制。达到限制时停止启动新步骤并进入 `waiting_human` 或 `failed`，已经取消/过期的结果不得提交。

### 6. User guidance

- 用户可在暂停或人工门状态提交下一步指导文本；指导只作用于后续未开始步骤。
- 指导文本进入现有 Generation Intent/Prompt 组装边界，不直接拼接到 SQL、日志或审计公开字段。
- 每次指导变更增加 Run epoch/version，运行中的旧步骤完成后必须被判定为 stale。

### 7. Book completion

`complete_book` 在章节循环结束后必须：

- 验证大纲章节数、已完成章节和缺失章节一致性；
- 执行全书一致性审查并产生可查看的安全摘要；
- 对被标记章节执行受限返修/润色；
- 将 Novel Workflow 推进至 `reviewing`、`polishing`、`completed`；
- 复用项目导出能力产生最终导出结果或可下载文件引用；
- 只有所有硬性完结条件满足时才标记 Run `completed`。

### 8. Frontend workbench

项目创作管理中新增“自动创作”工作台：

- 创建 Run 时选择范围、人工门、预算和重试限制；
- 显示总阶段、当前步骤、当前章节、章节完成率、总字数、Token/成本/时间、质量趋势和失败原因；
- 提供暂停、恢复、取消、人工决定和后续指导控件；
- 可选显示模型实时生成内容与 Provider 明确 reasoning；关闭时不渲染高频输出，但不影响后台执行；
- 页面刷新后从 API 恢复 Run 状态，不依赖浏览器内存作为事实源；
- 运行指标继续使用现有固定布局策略，不随内容滚动丢失。

### 9. Security and compatibility

- 所有 Run/Step 查询和控制必须先验证项目所有权；非所有者保持现有不可见项目 `404` 语义。
- Durable 记录不持久化原始 Prompt、Provider reasoning、密钥、完整模型响应或原始异常。
- 错误对外使用稳定 error code；原始错误只进入受控服务端日志且不得包含敏感内容。
- 保持现有章节、向导、批量生成、R7 Autopilot、项目导出 API 向后兼容。

## Acceptance Criteria

### Durable lifecycle

- [x] PostgreSQL Alembic 与 Rust migration metadata 包含 Run/Step 表、索引、外键和可逆 downgrade。
- [x] 同项目并发创建只产生一个活动 Run。
- [x] 创建、列表、详情、暂停、恢复、取消和人工决定 API 有 owner/非 owner/非法状态测试。
- [x] 服务重启测试证明活动 Run 从最后 committed step 恢复，不重复提交已完成章节。
- [x] epoch/version 测试证明取消、暂停、指导更新或重试后的迟到结果被拒绝。

### End-to-end flow

- [x] 空资料项目可以自动完成世界观、职业、角色、组织和大纲阶段。
- [x] 有大纲项目可以自动生成至少三章，逐章质量检查并正确累计进度/字数。
- [x] 质量失败分别覆盖 accept、auto repair、retry、manual review 和重试耗尽分支。
- [x] `planning_only`、`next_n_chapters`、`continue_from_current`、`complete_book` 均有服务测试。
- [x] `complete_book` 完成全书审查、受限润色、Workflow 完结和导出引用；缺章时不得错误完成。

### Budget and controls

- [x] Token、成本、时长、章节数、连续 Provider 失败和连续质量失败均能阻止新步骤启动。
- [x] 暂停后不启动下一步骤；恢复从同一 committed checkpoint 继续；取消后不再修改业务数据。
- [x] 人工门和用户指导只影响后续步骤，且不把指导/Prompt/reasoning 写入公开 Run/Step 响应。

### Frontend and observability

- [x] 自动创作工作台可创建和控制 Run，刷新后状态一致。
- [x] 工作台显示阶段、章节、预算、质量、错误和导出结果。
- [x] 实时内容/reasoning 显示可开关，关闭后后台生成不受影响。
- [x] Runtime Metrics、后台任务列表和 SSE 能识别新的 Durable Autopilot 任务类型。
- [x] 前端 lint/build、Rust fmt/check、focused tests、迁移语法/metadata tests 和真实 HTTP smoke 全部通过，或对环境阻塞给出可复现证据。

## Non-Goals

- 不在本任务中实现多用户协同编辑或多代理并行写同一本小说。
- 不保证文学质量达到人工出版标准；系统保证流程、质量门和可追溯性。
- 不持久化或展示模型私有隐藏推理；只显示 Provider 明确返回的 reasoning 字段。
- 不替换现有 Background Task、Novel Workflow、Chapter、Outline 或 Project Export 事实 owner。
