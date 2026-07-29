# 15 - ainovel-cli 对比分析与 MuMuNovel 优化路线

> 文档类型：架构对标与工程优化决策
>
> 编写日期：2026-07-12
>
> 路线确认日期：2026-07-13
>
> 路线状态：**已确定并生效**
>
> 对比项目：`MuMuNovel`、`ainovel-cli`
>
> 适用范围：MuMuNovel Rust 后端、React 前端、后台任务、AI 工作流、质量运行时与工程治理

---

## 1. 文档目标

本文档基于两个项目的实际源码、构建结果和运行架构，从产品定位、AI 编排、状态机、
后台任务、持久化、恢复、配置、测试和部署等维度进行对比，并明确 MuMuNovel 后续的
优化方向。

本文档重点回答四个问题：

1. MuMuNovel 与 ainovel-cli 的核心差异是什么；
2. ainovel-cli 中哪些设计值得 MuMuNovel 借鉴；
3. 哪些设计不适合直接复制；
4. MuMuNovel 应该按照什么顺序实施优化，以及如何验收。

本文不是对现有质量路线图的替代，而是补充“自治编排、任务恢复、模型路由和工程治理”
视角。小说质量相关建设仍以以下文档为准：

- `docs/11-novel-quality-roadmap.zh-CN.md`
- `docs/12-novel-quality-runtime-architecture.zh-CN.md`
- `docs/小说创作操作系统_完整方案_v1.0.md`

---

## 2. 一句话结论

MuMuNovel 和 ainovel-cli 面向的是两种不同的产品形态：

- **MuMuNovel 是小说创作工作台和数据平台**，强调可视化编辑、人工控制、多用户、
  结构化数据和长期运营；
- **ainovel-cli 是自治小说生成执行器**，强调 Coordinator 调度、多角色 Agent、
  显式状态机、长任务 checkpoint 和无人值守运行。

因此，MuMuNovel 不应改造成 ainovel-cli，也不应迁移到纯 CLI 或纯文件 Store。
推荐路线是：

```text
MuMuNovel 保持：
Web UI + Rust Services + PostgreSQL + Background Tasks

选择性吸收 ainovel-cli：
显式创作状态机 + 角色模型策略 + 可恢复 checkpoint + 可选 Coordinator
```

最终目标是：

> 将 MuMuNovel 从“功能丰富的 AI 写作工作台”升级为
> “可人工控制、可后台执行、可恢复、可自动驾驶的小说创作操作系统”。

### 2.1 最终路线决策卡

本项目采用**可靠性优先、契约统一、最后引入自治编排**的单一路线，不再将“直接复制
ainovel-cli”“先做 Autopilot”或“迁移到 Go/TUI”作为并列候选方案。

```text
产品底座（保持不变）
Web 创作工作台 + Rust Services + PostgreSQL + 统一后台任务

可靠性主线（已完成）
Password Hash + Migration Executor 安全前置（已完成）
  -> R0.1 PostgreSQL Auth Schema Compatibility（已完成：源码 + 隔离 PostgreSQL 证据）
  -> R0.2 本地 PostgreSQL + Rust + Playwright 真实 E2E（已完成：14/14）
  -> R0.3 GitHub runner 真实 E2E 证据（已完成）
  -> G0 生产可靠性门禁（GO）

P1 能力主线
R3 Workflow State Machine
  -> R4 Story Packet / Generation Intent
  -> R5 角色级模型策略
  -> R6 可恢复业务 Checkpoint
  -> G1 统一契约门禁
  -> R7 受控 Autopilot MVP
  -> G2 自动驾驶安全门禁

P2 反馈闭环
R8 Eval + 创作档案 + 运行指标
```

**当前执行判断**：R0.1、R0.2、R0.3、G0、R1、R2、R3、R4、R5、R6、G1-Cancel、G1、R7、G2
与 R8 均已完成并形成可审计证据。R0.3 已通过 GitHub Hosted Runner 真实链路核验，G0 为 GO；R3-R6
已建立唯一 workflow 事实、统一 Story Packet、角色级模型追溯与 business checkpoint，G1-Cancel
已补齐进程内 cooperative cancellation 和 terminal ownership 防护。G1 于 2026-07-16 判定为 GO，
R7 已于 2026-07-16 以受控 Autopilot MVP 形式正式收口；受控 Tool Contract、`novel_autopilot` 最小任务/Coordinator、认证/项目作用域固定/一次性人工确认的控制 API、durable invocation audit，以及项目 workflow 内 owner-scoped、脱敏的只读调用历史面板均已完成并通过本地质量门禁。G2 已在同日完成固定样本和 failure-injection 审查并判定为 GO：`action API → generic task → queued audit → Coordinator/Tool → workflow/audit terminal → readonly history` 在故障下仍保持 owner 边界和安全回滚；history API 继续使用显式输出 allowlist，避免内部 actor、项目和 digest 字段出网。R8 已完成最小只读闭环；history 仍只是审计读取，不是 task runtime owner、checkpoint/recovery/replay 控制面，也不增加 Pause/Resume/Steer、自动重试或多步骤自治。

**R8 是当前已授权路线的终点，当前没有待开发的 R9；任何后续 Autopilot 扩展必须另行立项，并通过新的兼容性、回滚和安全门禁。**

```text
R0.1-R0.3 = PASS
G0          = GO
R3-R6       = PASS
G1-Cancel   = PASS
G1          = GO (2026-07-16)
R7          = PASS（受控 MVP，2026-07-16 正式收口）
G2          = GO（当前受控单次 MVP 安全门禁，2026-07-16；不等于无人值守授权）
R8          = PASS（只读评测 / 创作档案 / 运行指标，2026-07-16）
```

**Go / No-Go 规则**：

- R0.1、R0.2、R0.3 任一未通过，G0 均为失败，禁止进入 R3；
- R3、R4、R6 未统一状态、生成输入和业务 checkpoint 前，禁止进入 R7；
- G1 未通过，不开发整书 Autopilot；G2 未通过，不扩大为无人值守多卷生成；
- 不新建第二套后台任务系统，不新建第二套章节 checkpoint/resume owner；
- 数据库 Schema 变更必须单独取得明确授权，路线确认本身不等于变更授权。

**下一动作**：G2 与 R8 已于 2026-07-16 完成；任何后续能力均须独立立项和门禁授权。R8 仅以脱敏 golden sample、既有 owner 的只读导出和运行摘要为最小范围，不能复制 task/workflow/audit/checkpoint 事实。任何 Autopilot 扩展仍须拆分独立任务；不得把 history 或 R8 指标扩展为 Pause/Resume/Steer、checkpoint/recovery/replay、自动重试或任务中心控制面。G2 GO 仅通过当前单次人工确认闭环的安全门禁，仍禁止多步骤无人值守自治。
R0.1/R0.2 完成不等于生产数据库已经迁移，也不授权任何 production downgrade。

R0.1 的最小授权范围、upgrade/downgrade 数据保护、实施文件边界和验收矩阵已单独固化在：

- `docs/16-r0.1-auth-schema-authorization-package.zh-CN.md`
- `docs/17-r0.2-local-real-e2e-evidence.zh-CN.md`
- `docs/18-r0.3-local-runner-contract-evidence.zh-CN.md`

该文档现同时记录授权边界与实施证据；源码和隔离数据库验证已完成，但不代表已经允许执行生产数据库 migration。

### 2.2 路线生效声明

自 2026-07-13 起，本文中的以下顺序作为 MuMuNovel Rust 优化开发的正式执行基线：

```text
R0.1 Auth Schema Compatibility
  -> R0.2 Local Real E2E
  -> R0.3 GitHub Runner Evidence
  -> G0 Reliability Gate
  -> R3 Workflow State Machine
  -> R4 Story Packet / Generation Intent
  -> R5 Role Model Policy
  -> R6 Business Checkpoint
  -> G1-Cancel Cooperative Cancellation
  -> G1 Contract Gate
  -> R7 Controlled Autopilot MVP
  -> G2 Autopilot Safety Gate
  -> R8 Eval / Archive / Metrics
```

该基线的执行含义如下：

1. **Phase 1 Gate 已收口**：R0.1、R0.2、R0.3、G0、R3、R4、R5、R6、G1-Cancel 与 G1 已完成；
   R1、R2 作为已完成的可靠性能力保留，不重复建设同类基础设施；当前唯一下一主线为 R7。
2. **Schema 授权已履行**：2026-07-13 的明确授权仅覆盖 `password_hash` 类型、新 revision、
   initial schema、Python frozen source-map、migrator metadata、固定合同测试和隔离数据库验证。
3. **授权边界继续有效**：未授权生产数据库 migration、真实 downgrade、production downgrade CLI、
   其他表字段修改或历史 revision 改写；R0.1-R0.3 与 G0 的通过不扩大该授权边界。
4. **严格串行过 Gate**：R0.1 完成后才运行 R0.2，R0.2 通过后才采集 R0.3，三者全部通过
   才能审查 G0；G0 通过前不启动 R3，G1 通过前不启动 R7。
5. **路线变更必须留痕**：只有满足 16.8 节的变更条件时才能调整，并同步更新阶段依赖、
   验收证据、兼容性影响和回滚策略。

当前阶段状态（更新于 2026-07-16）：**R0.1、R0.2、R0.3、G0、R3、R4、R5、R6、G1-Cancel、G1、R7 与 G2 已通过。R7 保持单次、人工确认、NonResumable 的受控 MVP 边界；G2 仅确认该闭环在固定样本和故障注入下安全，R8 Eval / 创作档案 / 运行指标已完成，并继续保留冻结边界。**

### 2.3 Runner 证据硬化裁决

进程生命周期和构建来源证据归入 **R0.3 GitHub Runner Evidence**，不新增 R0.4，也不改变
R0.1 → R0.2 → R0.3 的主依赖链：

- 已完成的直接 binary 启动、PID 双写、四态 lifecycle JSON、失败关闭和 artifact 顺序继续作为
  R0.3 的固定合同；
- Linux Runner 上的 `/proc/<pid>/exe` 身份核验和 Rust binary SHA-256 已作为 R0.3 固定合同
  落地，用于防止 PID 复用、错误进程清理和构建来源不明；
- 身份不匹配时不得向未知 PID 发信号，不得生成成功 manifest；
- 该硬化已在 R0.2 通过后实施，并通过 Linux 容器覆盖身份匹配、身份不匹配、非法 PID 和
  进程提前退出；Windows 业务 harness 未为模拟 `/proc` 引入复杂抽象；
- provenance 静态合同或本地脚本验证不能替代真实 GitHub Runner 执行结果。

这一裁决遵循 KISS/YAGNI：保留能提高证据可信度的最小 Linux 原生检查，但不把 CI 实现细节
升级为新的产品阶段或当前阻塞项。

---

## 3. 项目概况

### 3.1 MuMuNovel

MuMuNovel 当前是大型全栈 Web 应用：

```text
Browser
  -> React / TypeScript / Ant Design
  -> Nginx
  -> Rust / Axum
  -> SeaORM / SQLx
  -> PostgreSQL
```

主要能力包括：

- 灵感生成与智能向导；
- 世界观、角色、职业、组织和关系管理；
- 大纲生成、展开和编辑；
- 章节生成、批量生成、局部重写、整章重生成和润色；
- 伏笔、记忆、章节分析和质量趋势；
- 拆书导入、项目导入导出和 Prompt 工坊；
- 本地账号、LinuxDO OAuth 和多用户数据隔离；
- SSE、轮询、后台任务中心和 Docker 部署。

当前生产后端已经是 Rust，Python 主要作为迁移、测试和运维支撑，不再是正式生产 runtime。

关键参考：

- `README.md:63`
- `README.md:108`
- `README.md:263`
- `README.md:341`
- `README.md:409`
- `backend-rs/src/main.rs:28`
- `backend-rs/src/api/router.rs:233`

### 3.2 ainovel-cli

ainovel-cli 是 Go 实现的自治小说生成 CLI：

```text
main
  -> TUI 或 Headless
  -> Host
  -> Coordinator
  -> Architect / Writer / Editor
  -> Tools
  -> JSON / Markdown / JSONL Store
```

核心能力包括：

- Coordinator 统一调度；
- 短篇和长篇 Architect；
- Writer 与 Editor 分工；
- Phase / Flow 状态机；
- Pause、Continue、Abort、Resume 和 Steer；
- 本地原子写入与追加式 checkpoint；
- 按角色配置 Provider、Model、Fallback 和 Reasoning Effort；
- TUI 和无人值守 Headless 运行。

关键参考：

- `../ainovel-cli/go.mod:1`
- `../ainovel-cli/cmd/ainovel-cli/main.go:27`
- `../ainovel-cli/internal/host/host.go:36`
- `../ainovel-cli/internal/host/host.go:335`
- `../ainovel-cli/internal/agents/build.go:116`
- `../ainovel-cli/internal/agents/build.go:213`

---

## 4. 核心差异矩阵

| 维度 | MuMuNovel | ainovel-cli | 判断 |
|---|---|---|---|
| 产品定位 | Web 小说创作平台 | 自治小说生成 CLI | 不是直接竞品 |
| 主要交互 | 页面、表单、弹窗、关系图 | TUI、命令行、Headless | MuMuNovel 更适合普通作者 |
| AI 驱动方式 | 功能 API 和 Service 驱动 | Coordinator 自主决策 | ainovel-cli 自治性更强 |
| 人工参与 | 每个业务步骤均可编辑 | 运行中 Pause/Steer | 两者应互补 |
| 数据源 | PostgreSQL | 本地 JSON/Markdown/JSONL | MuMuNovel 更适合产品化 |
| 多用户 | 支持 | 不支持 | MuMuNovel 明显占优 |
| 多项目 | 支持 | 以单本小说目录为中心 | MuMuNovel 明显占优 |
| 工作流状态 | 通用任务状态 + 业务工作流 | 整本书 Phase/Flow 状态机 | MuMuNovel 缺少上层统一状态机 |
| 后台任务 | 多任务、可轮询、可取消 | 单机长任务生命周期 | MuMuNovel 更适合并发任务 |
| 重启恢复 | 恢复记录，活跃任务默认失败 | 根据 checkpoint 继续执行 | ainovel-cli 更完整 |
| 模型配置 | 用户/项目/请求级 | 角色级模型与 fallback | MuMuNovel 可借鉴角色策略 |
| 部署 | PostgreSQL + Rust + Nginx | 单二进制或 Docker | ainovel-cli 更轻量 |
| 安全边界 | 认证、权限、数据库隔离 | 本地进程权限 | MuMuNovel 责任更复杂 |
| 可视化 | 丰富 | 有限 | MuMuNovel 明显占优 |
| 自动写整本书 | 尚未形成统一自治闭环 | 核心能力 | MuMuNovel 可增加可选模式 |

---

## 5. AI 编排方式对比

### 5.1 MuMuNovel：显式业务流程驱动

MuMuNovel 的主要链路是：

```text
用户选择功能
  -> 前端调用明确 API
  -> Rust Route
  -> Workflow / Service
  -> AIService
  -> Provider Client
  -> 结构化结果
  -> 页面预览或持久化
```

优点：

- 行为边界明确；
- 权限和项目隔离容易控制；
- 数据写入可以通过 Service 和数据库事务保护；
- 每个功能都能独立测试、回滚和优化；
- 用户可以在任意环节人工编辑。

限制：

- 用户需要主动触发每个步骤；
- 缺少跨世界观、角色、大纲、章节和审稿的上层调度者；
- 多条功能链可能形成不同的输入契约和恢复语义；
- 自动完成整本小说需要页面或调用方持续驱动。

AI Provider 入口参考：

- `backend-rs/src/ai/service.rs:13`
- `backend-rs/src/ai/service.rs:66`
- `backend-rs/src/ai/service.rs:183`
- `backend-rs/src/ai/service.rs:215`

### 5.2 ainovel-cli：Coordinator 自治驱动

ainovel-cli 的主要链路是：

```text
用户给出目标
  -> Coordinator 判断当前阶段
  -> 调用 Architect / Writer / Editor
  -> SubAgent 调用 Tool
  -> Tool 返回事实结果
  -> Coordinator 判断下一步
  -> 循环直到完成或暂停
```

角色分工：

| 角色 | 主要职责 |
|---|---|
| Coordinator | 读取状态、选择角色、决定下一步和处理异常 |
| Architect Short | 短篇、单卷和紧凑结构规划 |
| Architect Long | 长篇、分卷和持续升级结构规划 |
| Writer | 章节构思、写作、自审和提交 |
| Editor | 审查、修复和质量控制 |

优势是自动化程度高，但风险也更集中：

- Coordinator Prompt 漂移会影响全局；
- 长会话上下文和成本更难预测；
- 业务规则容易隐藏在 Prompt 中；
- 如果 Agent 可以直接修改数据，权限和一致性风险较大。

### 5.3 对 MuMuNovel 的明确结论

MuMuNovel 不应该让 Coordinator 取代现有 Service。

正确边界应当是：

```text
Coordinator：决定调用哪个业务能力
Service：校验权限、执行业务规则、修改数据库
Tool：把受控 Service 暴露给 Coordinator
Background Task：承载运行、取消、进度和 checkpoint
```

必须坚持：

> Coordinator 有编排权，但没有绕过 Service 直接写数据库的权力。

---

## 6. 状态机对比

### 6.1 MuMuNovel 当前状态

通用后台任务状态为：

```text
pending
running
completed
failed
cancelled
```

`TaskRecord` 同时保存：

- 用户和项目；
- 任务类型；
- 进度和消息；
- result 和 error；
- stage code；
- execution mode；
- workflow scope；
- checkpoint；
- payload fingerprint。

参考：`backend-rs/src/tasks/types.rs:6`

这套状态适合描述“一个任务有没有完成”，但不能完整描述“一本小说创作到了哪个阶段”。

### 6.2 ainovel-cli 当前状态

小说 Phase：

```text
init -> premise -> outline -> writing -> complete
```

写作 Flow：

```text
writing
reviewing
rewriting
polishing
steering
```

Go 层明确校验转换，不允许 Phase 回退，也不允许 Flow 任意跳转。

参考：

- `../ainovel-cli/internal/domain/transitions.go:9`
- `../ainovel-cli/internal/domain/transitions.go:27`
- `../ainovel-cli/internal/domain/transitions.go:37`
- `../ainovel-cli/internal/domain/transitions.go:60`

### 6.3 MuMuNovel 的缺口

MuMuNovel 当前拥有大量成熟的局部 workflow，但缺少一个小说级统一生命周期：

```text
灵感
  -> 创作基础
  -> 世界观
  -> 角色
  -> 大纲
  -> 写作
  -> 审校
  -> 发布准备
```

这会带来三个问题：

1. 自动驾驶模式无法仅根据统一状态决定下一步；
2. 页面和后台任务需要自行判断前置条件；
3. 项目级创作进度难以统一展示和恢复。

需要增加领域状态，但不能把它塞进 `TaskStatus`。

推荐分离：

```text
TaskStatus
  = 单个执行任务的生命周期

NovelWorkflowPhase
  = 一本小说的创作生命周期

GenerationStage
  = 某次生成内部的候选、评审、修复和保存阶段
```

---

## 7. 持久化与恢复对比

### 7.1 MuMuNovel

MuMuNovel 的业务数据存储在 PostgreSQL，适合：

- 多用户数据隔离；
- 复杂实体关系；
- 查询和统计；
- 事务与约束；
- 多实例服务演进。

通用后台任务目前采用：

```text
进程内 TaskRegistry
  + data/runtime/background_tasks.json
  + 每 1.5 秒周期快照
```

参考：

- `backend-rs/src/tasks/persistence.rs:9`
- `backend-rs/src/tasks/persistence.rs:19`
- `backend-rs/src/tasks/persistence.rs:43`
- `backend-rs/src/tasks/persistence.rs:67`

服务重启后，Pending/Running 孤儿任务会被标记为 Failed，而不是恢复执行上下文。

参考：`backend-rs/src/tasks/recovery.rs:8`

因此，当前能力应准确描述为：

> 可恢复任务记录和前端呈现，但通用任务层不能自动继续全部执行上下文。

### 7.2 ainovel-cli

普通文件使用临时文件和 `os.Rename` 原子替换：

- `../ainovel-cli/internal/store/io.go:36`
- `../ainovel-cli/internal/store/io.go:41`
- `../ainovel-cli/internal/store/io.go:63`

步骤 checkpoint 追加到：

```text
meta/checkpoints.jsonl
```

读取时逐行解析，损坏行被跳过，尾部截断不会破坏之前的记录。

参考：

- `../ainovel-cli/internal/store/checkpoints.go:16`
- `../ainovel-cli/internal/store/checkpoints.go:174`
- `../ainovel-cli/internal/store/checkpoints.go:182`

Host 可以根据 checkpoint 和当前进度构造 Resume Prompt，继续交给 Coordinator。

参考：`../ainovel-cli/internal/host/host.go:335`

### 7.3 对 MuMuNovel 的明确结论

MuMuNovel 不应把 PostgreSQL 替换成本地文件，但应借鉴两点：

1. 后台任务快照必须具备崩溃一致性；
2. 可恢复任务必须记录业务事实 checkpoint，而不是只记录百分比。

推荐将任务分成三类：

| 类型 | 示例 | 重启策略 |
|---|---|---|
| 不可恢复请求 | 单次流式模型调用 | 标记失败，允许重试 |
| 可重放任务 | 润色、分析、简单生成 | 根据原 payload 重新发起 |
| 可继续工作流 | 批量章节、拆书、多阶段生成 | 从业务 checkpoint 继续 |

---

## 8. 配置与模型路由对比

### 8.1 MuMuNovel

配置来源主要包括：

- 环境变量；
- 用户设置；
- 项目默认值；
- 页面表单；
- 单次请求覆盖参数。

这适合多用户产品，但模型选择主要围绕用户默认配置和具体功能调用。

### 8.2 ainovel-cli

配置覆盖顺序：

```text
~/.ainovel/config.json
  -> ./.ainovel/config.json
  -> --config 指定文件
```

参考：`../ainovel-cli/internal/bootstrap/configfile.go:53`

同时支持角色级：

- Provider；
- Model；
- Fallback；
- Reasoning Effort；
- Context Window；
- 推理强度动态调整。

参考：

- `../ainovel-cli/internal/bootstrap/config.go:48`
- `../ainovel-cli/internal/bootstrap/config.go:91`
- `../ainovel-cli/internal/bootstrap/config.go:97`
- `../ainovel-cli/internal/bootstrap/config.go:360`

### 8.3 MuMuNovel 应采用的模型策略

MuMuNovel 应增加“任务角色模型策略”，而不是简单复制 CLI 配置文件。

建议角色：

```text
planner    世界观、大纲和复杂推理
writer     章节正文和扩写
editor     审校、重写和质量修复
extractor  信息抽取、摘要和状态更新
researcher 联网检索和资料整理
```

推荐解析顺序：

```text
请求显式覆盖
  -> 项目角色策略
  -> 用户角色策略
  -> 用户默认模型
  -> 系统默认模型
```

每个角色策略至少包含：

```text
provider
model
fallbacks
reasoning_effort
max_tokens
context_window_override（可选）
```

---

## 9. 工程健康现状

### 9.1 MuMuNovel 本次验证

实际执行：

```powershell
cargo check --manifest-path "backend-rs/Cargo.toml"
npm run build --prefix "frontend"
```

结果：

- Rust `cargo check` 通过；
- 前端服务门面校验通过；
- 前端可见文本编码校验通过；
- TypeScript 编译通过；
- Vite 生产构建通过；
- Rust 存在 3 条 dead-code warning；
- Vite 存在 1 条 circular chunk warning。

项目中存在大量 Rust 测试函数，但本次没有执行完整 `cargo test`，不能据此声明所有测试通过。

### 9.2 MuMuNovel Rust CI 与真实 E2E 现状

2026-07-12 已完成 workflow 静态对齐：

- `backend-ci` 已覆盖 `backend-rs/**`，执行 Rust `fmt`、`check`、`test` 和增量 Clippy 门禁；
- Python job 已收敛为 migration/support 回归，不再代表生产 runtime；
- `e2e-smoke` 已切换为 PostgreSQL 18、Rust migration executor、Rust server 和现有
  Playwright auth/background-task smoke。

本地真实链路已进一步证明：

```text
PostgreSQL 18 -> Rust migration -> release preflight -> Rust server -> /readyz 200 -> /releasez 200 -> Playwright 14/14
```

早期认证阶段曾因 Argon2 `password_hash` 写入历史 `VARCHAR(64)` 超长而返回 `HTTP 500`；
R0.1 已修复该 Schema 契约，R0.2 随后以 PostgreSQL 18、20 个 revision、120 个 SQL step、
`/readyz`/`/releasez` 和 Playwright 14/14 完成闭环。当前剩余缺口仅为 R0.3 实际 GitHub Runner
绿色证据与可下载 artifact。

### 9.3 ainovel-cli 本次验证

实际执行结果：

| 命令 | 结果 |
|---|---|
| `go build ./...` | 通过 |
| `go vet ./...` | 通过 |
| `go test ./...` | Windows 下失败 |

失败集中在：

1. `HOME` 与 `USERPROFILE` 的测试隔离差异；
2. 通知命令固定依赖 `sh -c`；
3. 自更新测试断言 Unix `0755` 权限。

ainovel-cli 的 GitHub Actions 也只有 Docker 和 Release，没有普通 PR/Push 的
`go test/go vet/go build` 质量门禁。

---

## 10. MuMuNovel 目标架构

推荐目标架构：

```text
┌───────────────────────────────────────────────┐
│ Web UI                                        │
│ 编辑、预览、确认、关系图、任务中心、人工干预   │
└──────────────────────┬────────────────────────┘
                       │
                       ▼
┌───────────────────────────────────────────────┐
│ Rust API / Application Services               │
│ 权限、校验、事务、业务事实所有权               │
└──────────────────────┬────────────────────────┘
                       │
                       ▼
┌───────────────────────────────────────────────┐
│ Background Task Runtime                       │
│ 状态、取消、事件、checkpoint、重试、恢复策略   │
└───────────────┬───────────────────┬───────────┘
                │                   │
                ▼                   ▼
┌────────────────────────┐  ┌───────────────────┐
│ Explicit Rust Workflows│  │ Optional          │
│ 现有章节/向导/导入流程  │  │ Coordinator       │
└────────────┬───────────┘  └─────────┬─────────┘
             └─────────────┬──────────┘
                           ▼
┌───────────────────────────────────────────────┐
│ Controlled Domain Tools / Services            │
│ World / Character / Outline / Chapter / Edit  │
└──────────────────────┬────────────────────────┘
                       ▼
┌───────────────────────────────────────────────┐
│ PostgreSQL + Task Event + Workflow Checkpoint │
└───────────────────────────────────────────────┘
```

所有权必须明确：

| 层 | 拥有的职责 | 不应拥有的职责 |
|---|---|---|
| UI | 发起、观察、预览、确认、干预 | 复制业务工作流 |
| Route | 鉴权、解析、调用应用服务 | 内联长流程和任务状态机 |
| Background Task | 生命周期、进度、事件、取消 | 小说业务规则 |
| Coordinator | 选择下一项受控能力 | 直接写数据库 |
| Domain Service | 业务校验、事务和数据写入 | 页面状态 |
| PostgreSQL | 业务事实和可恢复状态 | Prompt 决策 |

---

## 11. MuMuNovel 优化路线总览

优化分为四个阶段：

```text
Phase 0  工程基线与任务可靠性
Phase 1  统一创作状态和模型策略
Phase 2  可选自动驾驶 Coordinator
Phase 3  评测、审计和长期反馈闭环
```

优先级定义：

- **P0**：当前生产可靠性或验证链存在明显缺口，应优先完成；
- **P1**：形成自治工作流和长期架构能力的关键建设；
- **P2**：提高评测、可移植性和长期优化效率。

---

## 12. Phase 0：工程基线与任务可靠性

### 12.1 P0-1：建立 Rust 生产后端 CI

> 当前状态（更新于 2026-07-14）：**R0.1、R0.2、R0.3 全部完成，G0 已通过**。
> 本地 PostgreSQL 18/Rust/Playwright 与 GitHub Hosted Runner 均通过 migration、release preflight、
> `/readyz`、`/releasez` 和 14/14 smoke；R3 已解除阻塞。

#### 目标

确保 PR 和 Push 验证的就是实际生产 Rust runtime。

#### 实施前问题

- `backend-ci.yml` 只运行 Python pytest；
- `backend-rs/**` 变更不会触发正式后端 CI；
- E2E 启动 Python Uvicorn，与生产部署不一致。

#### 实施动作

1. 在现有 `.github/workflows/backend-ci.yml` 中增加 `rust-production` job，
   避免制造第二个重叠 workflow；
2. `backend-ci.yml` 和 `e2e-smoke.yml` 均监听 `backend-rs/**`；
3. Rust 生产门禁实际运行：

```powershell
cargo fmt --manifest-path backend-rs/Cargo.toml -- --check
cargo check --locked --manifest-path backend-rs/Cargo.toml
cargo test --locked --manifest-path backend-rs/Cargo.toml
cargo clippy --locked --manifest-path backend-rs/Cargo.toml --all-targets -- `
  -D clippy::correctness -D clippy::suspicious
```

4. E2E 使用 PostgreSQL 18，先运行 Rust `migration-executor`，再启动 Rust server；
5. Playwright 继续执行认证与后台任务页面 smoke，并在失败时保留报告和 Rust 日志；
6. Python pytest job 保留并命名为 `python-migration-support`。

严格 `-D warnings` 当前会触发约 208 个历史诊断。本轮修复了 migration lock 的
`suspicious_open_options`，并以 correctness + suspicious 作为增量强制门禁；约 206 个
其余 warning 作为独立技术债清理，不使用全局 `allow` 掩盖。

#### 验收标准

- `backend-rs/**` 任意 PR 能触发 Rust CI；
- Rust 编译、测试、Clippy 任一失败都会阻止合并；
- Playwright E2E 使用 Rust runtime；
- Python CI 不再被描述为生产后端 CI；
- CI 文档明确 Rust 与 Python 的所有权边界。

#### 非目标

- 本阶段不删除 Python 测试；
- 不在同一任务中重写全部 E2E；
- 不要求一次性清零所有历史 warning，可先建立基线和增量门禁。

---

### 12.2 P0-2：后台任务快照原子化

#### 目标

避免进程退出、磁盘写入中断或主机掉电时产生半写 JSON 快照。

#### 当前问题

`backend-rs/src/tasks/persistence.rs` 使用 `tokio::fs::write()` 直接覆盖
`background_tasks.json`。

#### 实施动作

已落地方案（2026-07-12）：

```text
进程内 Tokio mutex
  -> 序列化 version 1 snapshot
  -> 写入 background_tasks.json.tmp
  -> write_all / flush / sync_all
  -> 校验 temp
  -> 有效 primary: background_tasks.json -> background_tasks.json.bak
  -> 无效 primary: 隔离为唯一 .corrupt-* 文件
  -> background_tasks.json.tmp -> background_tasks.json
  -> Unix best-effort 同步父目录
```

加载顺序固定为 `primary -> backup -> temp`。Windows 不依赖覆盖式 rename；提交失败且
primary 缺失时，会尝试将 backup 回滚为 primary。

中期方案：

- 将关键 TaskRecord 持久化到 PostgreSQL；
- JSON 快照仅作为应急和诊断副本；
- 任务状态变更时写库，而不是只依赖 1.5 秒轮询快照。

#### 验收标准

- 写入过程中终止进程不会破坏上一个有效快照；
- 启动时遇到损坏快照有明确日志和降级策略；
- 临时文件不会无限累积；
- Windows 和 Linux 均有测试；
- 保存失败不会影响正在执行的业务任务。

#### 非目标

- 不在短期原子化任务中同步引入完整消息队列；
- 不把 PostgreSQL 业务数据改成本地文件。

---

### 12.3 P0-3：定义恢复语义分类

#### 目标

统一“恢复呈现、重新执行、断点继续”三个概念，避免前后端和用户对恢复能力产生误解。

#### 建议状态

```text
recovery_policy:
  fail_on_restart
  replay_from_payload
  resume_from_checkpoint
```

```text
recovery_state:
  not_required
  recoverable
  recovering
  recovered
  recovery_failed
```

#### 实施动作

1. 为后台任务类型建立恢复策略注册表；
2. 明确每个 `task_type` 的恢复等级；
3. 服务启动时根据策略处理孤儿任务；
4. 任务中心显示“需重新发起”或“可继续恢复”；
5. 不允许通过是否存在 `checkpoint` 猜测恢复能力。

#### 验收标准

- 所有长期 task type 都有显式恢复策略；
- 前端不把“记录恢复”展示成“执行已恢复”；
- `fail_on_restart` 任务保留清晰重试入口；
- `resume_from_checkpoint` 任务能从已验证的业务 checkpoint 继续；
- 恢复失败后不会产生重复章节或重复数据写入。

---

## 13. Phase 1：统一创作状态和模型策略

### 13.1 P1-1：建立小说级 Workflow State Machine

#### 目标

在现有局部工作流之上，建立一本小说的统一创作阶段，为自动驾驶、进度展示和恢复提供依据。

#### 推荐 Phase

```text
inspiration
foundation
world_building
character_design
outline
writing
reviewing
polishing
completed
```

可额外维护运行状态：

```text
idle
running
waiting_user
paused
blocked
failed
```

#### 实施动作

1. 定义 `NovelWorkflowPhase` 和合法转换；
2. 明确哪些阶段允许回退，哪些只能通过创建新 revision 处理；
3. 将项目当前阶段持久化到数据库；
4. 为阶段转换记录操作人、原因、时间和关联 task；
5. 前端项目页显示当前阶段、阻塞原因和建议下一步；
6. Coordinator 和人工页面共用同一个状态源。

#### 验收标准

- 非法阶段转换在 Rust 层被拒绝；
- 页面刷新和服务重启后阶段不丢失；
- 人工操作与后台任务不会互相覆盖阶段；
- 每次转换可审计；
- 阶段状态不与 `TaskStatus` 混用；
- 不破坏已有项目和 API，历史项目有明确默认阶段推导规则。

#### 建议落点

候选位置，仅作为设计指引：

- `backend-rs/src/models/novel_workflow_run.rs`
- `backend-rs/src/services/novel_workflow_service.rs`
- `backend-rs/src/api/projects.rs` 或独立 workflow route
- `frontend/src/features/novel-workflow/`

正式路径应在具体设计任务中结合现有目录所有权确认。

---

### 13.2 P1-2：统一 Generation Intent / Story Packet

#### 目标

把项目默认值、用户请求、章节目标、质量策略和修复意图统一成可快照、可审计的生成契约。

这项工作与 `docs/11-novel-quality-roadmap.zh-CN.md` 中的 Story Packet 方向一致，
不应再创建第二套对象。

#### 必要字段

```text
project_id
chapter_id / outline_id
story_goal
conflict_goal
character_changes
relationship_changes
foreshadow_actions
forbidden_items
quality_policy
style_policy
memory_snapshot_ref
model_policy_ref
source_revision
payload_fingerprint
```

#### 实施动作

1. 盘点章节生成、批量生成、重生成和大纲生成输入；
2. 建立统一归并层；
3. 明确默认值、项目值和请求覆盖值的优先级；
4. 将最终生成契约保存为 snapshot；
5. checkpoint 只引用稳定契约，不依赖易变页面状态。

#### 验收标准

- 单章、批量、重生成至少共享同一核心契约；
- 同一输入可以计算稳定 fingerprint；
- 生成历史能够回放当时使用的最终参数；
- 前端不再自行拼接同名但语义不同的 payload；
- 保持现有 API 兼容门面。

---

### 13.3 P1-3：角色级模型策略

#### 目标

根据任务角色选择模型，提升质量、成本和 Provider 故障恢复能力。

#### 推荐角色

| 角色 | 典型任务 | 默认模型特征 |
|---|---|---|
| planner | 世界观、大纲、复杂规划 | 强推理 |
| writer | 正文、扩写、重写 | 长上下文、稳定文风 |
| editor | 审校、修复、评分 | 低温度、结构化输出 |
| extractor | 摘要、状态抽取 | 快速、低成本 |
| researcher | 联网检索和资料整理 | Tool 能力、引用友好 |

#### 实施动作

1. 新增角色策略 schema；
2. 支持用户级和项目级覆盖；
3. 支持 fallback 列表；
4. 记录最终使用的 provider/model；
5. 将 reasoning effort 纳入可选能力，而不是假设所有 Provider 都支持；
6. 失败切换必须记录原因和成本。

#### 验收标准

- 每个核心生成任务能解析出明确角色；
- 模型解析顺序有单元测试；
- 主模型失败后可按配置 fallback；
- 历史记录可以看到实际使用的模型；
- 未配置角色策略时保持现有默认行为；
- API Key 不进入日志、checkpoint 或前端任务 result。

---

### 13.4 P1-4：业务 checkpoint 标准

#### 目标

让可继续任务保存足够的业务事实，而不是只保存进度百分比和提示文本。

#### 推荐结构

```json
{
  "schema_version": 1,
  "workflow_type": "chapter_batch_generation",
  "workflow_version": "...",
  "input_fingerprint": "...",
  "phase": "generating",
  "completed_item_ids": ["..."],
  "current_item_id": "...",
  "candidate_ids": ["..."],
  "source_revision": 12,
  "last_committed_step": "candidate_persisted",
  "resume_token": null
}
```

#### 实施原则

- checkpoint 必须有 schema version；
- checkpoint 只保存恢复需要的业务事实；
- checkpoint 不保存 API Key 和完整敏感 Prompt；
- 每一步必须明确是“可重复执行”还是“恰好一次提交”；
- 恢复前必须校验 input fingerprint 和 source revision；
- 数据库写入必须具备幂等键或唯一约束。

#### 验收标准

- 在指定步骤终止服务后可以稳定恢复；
- 恢复不会重复创建章节或重复应用正文；
- 输入或源 revision 改变时拒绝错误恢复；
- 旧 checkpoint schema 有迁移或明确失败提示；
- checkpoint 与任务进度投影保持一致。

---

## 14. Phase 2：可选自动驾驶 Coordinator

### 14.1 P1-5：新增 Autopilot，而不是替换现有流程

#### 目标

为高级用户增加“一键推进多个创作阶段”的可选模式，同时保留现有人工工作台。

#### 建议入口

```text
task_type = novel_autopilot
execution_mode = interactive | unattended
```

#### Coordinator 可调用的受控能力

```text
inspect_project_state
prepare_story_foundation
generate_world_building
generate_characters
generate_outline
generate_next_chapter
review_chapter
repair_chapter
advance_workflow_phase
request_user_decision
pause_workflow
```

每个 Tool 必须：

- 调用已有 Rust Service；
- 返回结构化事实；
- 校验用户和项目权限；
- 声明是否幂等；
- 声明是否推进阶段；
- 不允许接受任意 SQL、文件路径或内部函数名。

#### 人工门禁

建议默认在以下阶段等待人工确认：

1. 创作基础完成后；
2. 世界观与主要角色完成后；
3. 大纲完成后；
4. 每卷完成后；
5. 质量门禁持续失败时；
6. 成本、字数或章节数超过预算时。

#### 验收标准

- Coordinator 不能绕过 Service 写数据库；
- 所有 Tool 调用可审计；
- 用户可以暂停、继续和注入新的创作方向；
- 页面关闭后任务继续运行；
- 服务重启后的行为符合任务恢复策略；
- Autopilot 关闭时现有页面流程行为不变；
- 达到人工门禁时不会自动越过确认点。

#### 非目标

- 首版不要求无人值守生成整本百万字小说；
- 不把所有业务规则迁移进 Prompt；
- 不允许 Coordinator 自由发明数据库字段或 API；
- 不删除现有同步/SSE 兼容入口。

---

### 14.2 P1-6：运行时干预协议

#### 目标

借鉴 ainovel-cli 的 Steer 能力，让用户在长任务运行中改变后续方向，而不是只能取消重来。

#### 建议事件

```text
pause_requested
resume_requested
steer_submitted
user_decision_required
user_decision_resolved
budget_limit_reached
workflow_blocked
```

#### 处理规则

- 干预不直接修改正在生成的不可恢复模型请求；
- 干预写入任务事件队列，在安全边界消费；
- 已开始的数据库事务不被 Prompt 干预打断；
- Steer 必须记录提交人、时间和生效 checkpoint；
- 同一 checkpoint 后的多条 Steer 按序号处理。

#### 验收标准

- 用户提交 Steer 后可以看到“待消费/已消费”；
- Steer 不会被重复应用；
- 多端同时操作有确定冲突策略；
- 暂停后任务不会继续推进阶段；
- 恢复后从明确安全点继续。

---

## 15. Phase 3：评测、审计和反馈闭环

### 15.1 P2-1：Prompt / Tool 协议回归门禁

#### 目标

避免 Prompt、Provider 或 Tool schema 变化悄然破坏生成质量和结构化输出。

#### 测试层次

```text
Schema Test
  -> Tool Contract Test
  -> Recorded Provider Fixture
  -> Golden Sample Evaluation
  -> Optional Live Model Evaluation
```

#### 覆盖范围

- 灵感输出；
- 世界观和角色结构；
- 大纲生成与展开；
- 单章和批量章节生成；
- 局部重写和整章重生成；
- 章节分析与修复建议；
- 伏笔和人物状态抽取；
- Autopilot Tool 选择。

#### 验收标准

- 核心 JSON 输出均有 schema 校验；
- Tool 参数和结果有契约测试；
- 固定样本能够比较版本差异；
- CI 默认运行不依赖真实付费 API；
- Live Eval 作为计划任务或手动任务运行；
- 结果记录模型、Prompt 版本、成本和耗时。

---

### 15.2 P2-2：创作档案包导出

#### 目标

保持 PostgreSQL 为真实数据源，同时提供人类可读、Git 友好和可审计的项目导出。

#### 推荐结构

```text
novel-export/
  manifest.json
  project.json
  foundation.md
  world-building.md
  characters.json
  organizations.json
  relationships.json
  outline.md
  chapters/
    0001-title.md
  generation-history.jsonl
  workflow-events.jsonl
  checkpoints.jsonl
```

#### 验收标准

- 导出不包含 API Key、密码和 OAuth Token；
- manifest 包含 schema version；
- Markdown 可直接阅读；
- JSON 可重新导入或用于诊断；
- 大项目支持流式导出；
- 导出结果稳定，避免无意义字段顺序变化。

---

### 15.3 P2-3：质量与运行指标统一

#### 目标

把现有质量趋势与后台任务运行指标连接起来，形成可定位的反馈闭环。

#### 建议指标

质量指标：

- 章节质量平均分；
- 低于阈值占比；
- 自动修复成功率；
- 人工修改字数占比；
- 大纲偏移次数；
- 人物状态冲突次数。

运行指标：

- 各 task type 成功率；
- 平均耗时和 P95；
- Provider 错误率；
- fallback 触发率；
- 服务重启后失败任务数；
- checkpoint 恢复成功率；
- 单章 Token 和成本。

#### 验收标准

- 能从失败章节追溯到任务、模型、Prompt 和生成契约；
- 能区分质量失败、Provider 失败、配置失败和恢复失败；
- 指标不记录敏感 Prompt 或 API Key；
- 前端质量面板与任务中心使用统一任务标识关联。

---

## 16. 已确定的实施路线

> 决策状态：**已确定（Roadmap v1.3，固化 R0.3 lifecycle / provenance 边界）**
>
> 决策日期：2026-07-13
>
> 执行原则：阶段串行、任务独立、门禁通过后再进入下一阶段。

本路线不再作为并列候选方案讨论。除非生产事故、核心产品方向变化或阶段验证证明
前置假设不成立，后续 Trellis 任务应按照本节顺序创建和实施。

### 16.1 总体依赖链

```text
已完成的本地可靠性子项：R1 快照原子化 -> R2 恢复策略注册表与前端指引

当前阻断主线：
R0.1 PostgreSQL Auth Schema Compatibility
  -> R0.2 本地 PostgreSQL + Rust + Playwright 真实 E2E
  -> R0.3 GitHub runner 真实 E2E 证据
  -> G0 生产可靠性门禁
  -> R3 小说级 Workflow State Machine
  -> R4 Story Packet / Generation Intent
  -> R5 角色级模型策略
  -> R6 业务 checkpoint 标准
  -> G1 统一契约门禁
  -> R7 Autopilot MVP
  -> G2 自动驾驶安全门禁
  -> R8 Eval、档案导出和指标闭环
```

不得绕过 `G0` 直接实现 Coordinator，也不得在 Story Packet 和 checkpoint 尚未统一时
启动整书自动驾驶。

### 16.1.1 当前执行切片（已确定）

R0.1 授权前曾优先完成 Rust 只读命令 `release-readiness-preflight`；授权实施后，该命令已在
隔离 PostgreSQL 上返回 `release_ready=true`。它继续作为 R0.2/R0.3 的前置证据，不新增路线阶段。

该切片的固定边界如下：

1. 不启动 HTTP server，直接检查数据库连接、Rust migration live head 与 password verifier
   最终目标存储契约；
2. 复用 Rust `/releasez` 的同一 production release 判定所有者，不在 CLI、PowerShell、Node
   或 Workflow 中复制 target Schema 规则；
3. 输出结构化 JSON；仅当真实 PostgreSQL、migration head 和 `TEXT / unbounded_text` 目标契约
   全部满足时退出码为 `0`，其他情况失败关闭并返回非零；
4. 只执行元数据查询，不运行 migration、不执行 DDL、不自动修复 Schema；
5. SQLite、兼容但非目标的 `VARCHAR(97+)`/无界 `VARCHAR`、数据库不可用和 head mismatch 均不能
   作为 R0.1 或 production release 的通过证据。

当前近期执行顺序锁定为：

```text
只读 release-readiness-preflight（已完成）
  -> 独立数据库 Schema 变更授权（已确认）
  -> R0.1 Auth Schema Compatibility（已完成）
  -> R0.2 Local Real E2E（已完成）
  -> R0.3 GitHub Runner Evidence（当前）
  -> G0 审查
```

第一项现已完成：CLI 与 `/readyz`、`/releasez` 共用
`production_readiness_service`，stdout 只输出结构化 JSON，日志进入 stderr；TEXT、兼容但
非目标的 `VARCHAR(255)`、历史 `VARCHAR(64)`、head mismatch、SQLite 和数据库不可用均有
失败关闭测试。preflight 也已接入 GitHub Rust E2E workflow，固定在 migration 后、Server
启动前运行，并分别保留 JSON、stderr 与原始退出码；Server 启动后的 `/readyz`、`/releasez`
证据仍然保留。相关 preflight 契约测试为 9/9，通过 fmt、check、Clippy correctness+suspicious；加入 migration
executor evidence gate 后为 10/10，再加入 Runner success evidence artifact gate 后更新为 11/11，
最终 Rust 全量回归为 1590/1590。

该结果最初只消除了本地执行入口、Runner 早期失败诊断和 Rust release 判定复用方面的不确定性；
现已叠加 R0.1 Schema 实施与隔离 PostgreSQL 证据；该段描述的是 R0.2 执行前的准备状态。
R0.2 后续已在 `r02-local-real-e2e-20260713-final2` 完成，当前仅 R0.3 与 G0 未完成。

Settings 本地 HTTP mock 的并发抖动也已定位为 Windows system proxy 对 local gateway 请求的
干预。OpenAI、Gemini、Anthropic 现仅对 loopback/`host.docker.internal` 使用 local-only
proxy bypass，远程 Provider 代理行为保持不变；Settings 测试已通过 100/100 轮并发压力验证。
该结果属于 R0.2/R0.3 的无 Schema 质量证据准备，不代表 R0.1、R0.2、R0.3 或 G0 完成。

R2 的浏览器级恢复语义合同也已补齐：`background-task-recovery-semantics.spec.ts` 从
localStorage 中的重启前 active 任务出发，经过真实 `ProtectedRoute`、前端轮询 service、Rust 风格
mock 终态、Zustand 归一化和任务中心渲染，覆盖 `restart_required`、`resume_available`、
`manual_review` 与 unknown `non_resumable` 四类结果，并验证章节任务保留 checkpoint、
`canResume=true` 和单任务“继续”入口。该测试只证明前端恢复投影与交互合同，不启动 Rust
进程、不连接 PostgreSQL，因此不能替代 R0.2 本地真实 E2E，也不改变 R0.1、R0.2、R0.3 或 G0
状态。

Migration executor evidence gate 也已闭环：CLI stdout 现在只输出单一 JSON report，tracing、配置与
连接诊断进入 stderr；GitHub Runner migration step 分别保存 JSON、stderr 与原始退出码，并在
失败时按原值传播、阻止 preflight 和 Server 启动。production CI 契约 10/10、YAML、Git Bash
语法与隔离进程探针均通过，探针同时证明 process exit code 与 report `exit_code` 一致。该结果仍只
属于 R0.2/R0.3 的无 Schema 证据准备，不代表真实 PostgreSQL Auth E2E、GitHub Runner 证据，
也不代表 R0.1、R0.2、R0.3 或 G0 完成。

Runner success evidence artifact gate 随后补齐：两项 Playwright smoke 成功后才生成
`runner-success.json`，记录 Rust runtime owner、PostgreSQL、各发布门禁状态以及 GitHub SHA、run ID
和 attempt；既有 `rust-readiness-diagnostics` artifact 改为成功/失败均上传。因此未来绿色 Runner
也会留下可下载、可定位到具体提交的结构化证据，而失败链路不会伪造 success manifest。新增契约后
production CI 为 11/11，完整 Rust 回归为 1590/1590，YAML、Git Bash 与 JSON 行为探针均通过。
这仍只是 R0.3 证据能力准备，尚未实际取得 GitHub Runner 成功执行，不能把 R0.3 或 G0 标记完成。

Runner backend process lifecycle evidence gate 进一步补齐：workflow 不再后台运行 `cargo run`
wrapper，而是先 build、再直接启动 `mumu-novel-backend`，使持久化 PID 与最终 server 一致。
Playwright 后的 always cleanup 现在先记录 `rust-backend-lifecycle.json`，正常 TERM 退出才允许
生成带 `backend_lifecycle=passed` 的 `runner-success.json`；PID 提前退出会记录 `already_exited`
并失败，TERM 十秒未退出会 KILL、保留 `forced_kill` 证据并令 job 失败。cleanup 已移动到
compatibility-stable diagnostics artifact 上传前，
因此绿色和失败 Runner 都能持久化完整生命周期证据。production CI 契约保持 11/11，Git Bash
真实 TERM/already-exited/forced-KILL 探针与 JSON 精确解析均通过。该切片当时仍未触发真实 GitHub Runner，因此不能完成 R0.3 或 G0。R0.1、R0.2 后续已完成；
当前状态以 2.1、17.1 和 `docs/17-r0.2-local-real-e2e-evidence.zh-CN.md` 为准。

### 16.2 Stage A：R2 可靠性任务收口（已完成并冻结）

R2 的核心实现、前端恢复指引、启动恢复即时持久化、生产任务类型覆盖防漂移合同和本地质量
门禁均已完成并冻结。当前 Trellis 指针为
`.trellis/tasks/07-12-rust-production-ci-e2e`，只推进 R0.3/G0，不把后续 R6 checkpoint 或新产品
能力混入当前 P0。R2 已完成的收口项包括：

1. 生产 `task_type` 集合与 23 项恢复策略注册表已完成最终审计；
2. 生产源码合同和完整 Rust 回归证据已写入任务记录；
3. R2 不再吸收 R6 checkpoint、统一取消架构或 Coordinator 工作。

完成标志：

> R2 的实现、注册表覆盖、启动顺序合同和验证证据全部可追溯；后续优化不再与后台任务
> 恢复策略任务混合实施。

### 16.3 Phase 0：生产可靠性

#### R0：Rust 生产 CI 与真实 E2E 对齐

状态（更新于 2026-07-14）：**R0.1、R0.2、R0.3 全部完成；G0 已正式审查为 GO，R3 已解除阻塞**。

已完成：Rust `fmt`、`clippy`、`check`、`test` 已进入生产后端 CI；E2E smoke 已从
Python Uvicorn 切换为 PostgreSQL 18 + Rust migration executor + Rust server；Python runtime
已收敛为 migration/support 回归目标。

首次真实本地验证曾成功通过 migration 和 `/health`，随后在本地管理员创建阶段失败：

```text
HTTP 500
Query Error: value too long for type character varying(64)
```

根因是 Argon2 哈希长度超过 `user_passwords.password_hash VARCHAR(64)`。因此 R0 必须按以下
三个子门禁串行完成：

1. **R0.1 PostgreSQL Auth Schema Compatibility（已完成）**：`password_hash` 容量、Rust migration、
   初始 schema、冻结的 Python migration/source-map 契约与真实 PostgreSQL auth 回归均已通过；
2. **R0.2 Local Real Rust E2E（已完成）**：PostgreSQL migration、Rust server、`/readyz`、
   `/releasez`、`auth.spec.ts` 和 `background-task-pages.spec.ts` 已 14/14 通过；
3. **R0.3 GitHub Runner Evidence（已完成）**：实际 GitHub Hosted Runner 已给出同一链路的绿色证据；
   backend-ci Run `29264634399` 与 e2e-smoke Run `29264634424` 全部通过，包含直接 Rust binary、
   生命周期、成功 manifest、诊断 artifact、Linux executable identity 与 binary SHA-256 provenance。

R0.1 的无 Schema 安全前置已经完成：Password Hash 已收敛为唯一 Rust Service Owner，且只有
`password_hash::Error::Password` 映射为普通密码不匹配；unsupported algorithm、version、参数错误
以及损坏 verifier 均返回显式 `InvalidVerifier`。验证器在执行昂贵 Argon2 计算前强制 canonical
合同：`argon2id`、`v=19`、`m=19456`、`t=2`、`p=1`、32 字节输出，拒绝数据库中被篡改的
超大参数造成登录资源消耗放大。legacy SHA-256 verifier 已改为十六进制解码后使用
`subtle::ConstantTimeEq` 比较固定 32 字节摘要，继续兼容历史大写值且避免普通字符串比较的潜在提前退出。
`AuthService::authenticate_local()` 现已通过显式业务决策区分认证成功与普通凭证错误，
损坏 verifier 的 `InvalidVerifier` 保持向上游传播，不会伪装为用户名或密码错误。该前置已通过
Password Hash 10/10、AuthService 业务与数据库边界 7/7、完整 Rust 1558/1558 测试、fmt、check 和
Clippy correctness+suspicious。新增的 `login_local()` 数据库回归已证明正确 legacy SHA-256 密码
会升级为 canonical Argon2，错误密码不会改写 verifier；canonical Argon2 正确登录不会重复 rehash
或更新 `updated_at`；损坏 verifier 返回显式错误，并保持数据库 verifier 与 `updated_at` 不变。
测试套件的 `INSTANCE_ID` 并发污染也已用测试专用锁与 RAII 恢复消除。

Migration Executor 的 revision 执行边界也已完成无 Schema 加固：每个 revision 在同一个数据库
事务中执行全部 SQL steps 并更新 `alembic_version`，只有两者同时成功才提交；SQL step 失败或
revision head 更新失败会显式回滚当前 revision，前一 revision 的已提交结果保持不变。新增 3 条
事务回归覆盖成功提交、SQL 中途失败回滚和 head 更新失败回滚，模块测试 26/26 通过。
完整门禁首次暴露 Settings 临时 HTTP server 测试夹具的并发非确定性后，夹具已统一为带真实
`/__test_ready` HTTP barrier 和 RAII abort 兜底的测试 server；Settings 50 条测试以 16 线程连续
20 轮（累计 1000 条）通过。

R0 Workflow 生产所有权与执行顺序也已由 Rust 测试固化：新增测试专用
`production_ci_contract_tests` 模块，直接编译期读取 `backend-ci.yml` 与 `e2e-smoke.yml`，覆盖
`backend-rs/**` 触发范围、fmt → check → test → Clippy 顺序、Python migration/support 定位，以及
PostgreSQL → Rust migration → Rust server → health → Playwright 的真实 E2E 顺序；同时拒绝
`uvicorn`、`alembic-sqlite.ini` 和 `sqlite+aiosqlite` 回流，并保护 Playwright 报告、Rust 日志和
进程清理诊断。契约测试 5/5 通过，当前完整 Rust 结果为 1566/1566；fmt、check、Clippy
correctness+suspicious、前端构建和两份 Workflow YAML 解析全部通过。

随后补齐 Rust readiness 对 migration head 的真实判定：`/readyz` 不再只依赖 startup 与数据库
ping，而是同时要求 live `alembic_version` 与 Rust catalog head 匹配；head mismatch、缺表、空表或
查询失败都会保持 `503 not_ready`，并继续通过既有 `schema_migration.live_database_head` 输出诊断。
E2E workflow 的 Rust server wait 已从 liveness-only `/health` 切换为 `/readyz`，Rust 契约测试同时
禁止回退。

在此基础上又增加 Auth password verifier 存储容量的只读 gate：Rust 从密码哈希 owner 复用
canonical Argon2 PHC 的 97 字符长度契约，并查询 PostgreSQL `information_schema.columns`。
`TEXT`、无界 `VARCHAR` 或容量至少 97 的 bounded `VARCHAR` 可以允许 readiness，但只有 `TEXT`
匹配 R0.1 的最终 `unbounded_text` target；历史 `VARCHAR(64)`、缺列、不支持类型或查询失败均返回
`503 not_ready`，诊断位于 `schema_migration.auth_password_hash_storage`。SQLite 等非 PostgreSQL
环境明确标记 `not_applicable_non_postgres`，不能作为 PostgreSQL 兼容性证据。

加入 Runner readiness 防漂移契约后，当前完整 Rust 结果更新为 1575/1575；Workflow 契约
6/6、fmt、check、Clippy correctness+suspicious、YAML 解析和 readiness wait Bash 语法检查均通过。
真实 PostgreSQL `VARCHAR(64) -> /readyz 503` 的查询/解码链路仍需在 R0.2 或取得相应环境授权后
验证，当前纯逻辑和 SQLite 回归不能替代真实数据库证据。


为保证上述诊断可以成为 R0.3 runner 证据，E2E workflow 不再把 `/readyz` 响应丢弃到
`/dev/null`。每次轮询会保存最后一次 JSON 与 HTTP 状态，Rust 后端日志也写入同一
`e2e-diagnostics/` 目录；超时时直接打印三项信息，workflow 失败时上传独立
`rust-readiness-diagnostics` artifact。Rust 契约测试同时禁止该证据链回退。该工作只完成证据
准备，不等于已经在 GitHub runner 上取得 R0.3 真实执行结果。


同时，R0.2 增加生产目标 Schema Gate：`/readyz 200` 只能证明当前字段容量可运行，不能证明
R0.1 最终目标已经落地。Playwright 启动前会读取已保存的 readiness JSON，并强制要求 live
migration head 匹配、支持 canonical Argon2、`matches_target_storage_contract=true`，且目标标识
仍为 `unbounded_text`。因此无界 `VARCHAR` 或 `VARCHAR(97+)` 即使运行兼容，也不能冒充 R0.1/G0
完成证据。Node 行为验证已证明 target=true 通过、compatible-but-non-target 明确失败。最新完整验证为
Rust 1576/1576、Workflow 契约 7/7，fmt、check、Clippy correctness+suspicious、YAML、Bash
语法与 Node 行为验证全部通过。

随后进一步把生产 target 判定收回 Rust：新增 `/releasez` 作为 release readiness，保留
`/readyz` 的 runtime readiness 语义。`/releasez` 复用 live head 与 verifier storage 检查，并额外
要求最终 `unbounded_text` target；兼容但非目标的 `VARCHAR` 和非 PostgreSQL 证据都会返回 503。
E2E Workflow 已删除内联 Node 字段判断，改为直接执行 `/readyz -> /releasez -> Playwright`，并
保存两类响应与 HTTP 状态。这样本地 R0.2 和 GitHub Runner 使用同一个 Rust-owned 发布判定，
避免三套脚本规则漂移。新增测试后最新完整 Rust 基线为 1578/1578，Workflow 契约 7/7，
fmt、check、Clippy correctness+suspicious、YAML 与 Git Bash 语法检查全部通过。R0.1 随后已完成
Schema 实施和隔离 PostgreSQL Auth 验证；R0.2 Playwright 全链路也已在 final2 证据中完成，
尚未完成的是 R0.3 Runner 证据。

R0.1 已将 `password_hash`、Rust revision catalog、initial schema、Python frozen source-map、
migrator metadata 和固定 head/count 合同同步到新目标；生产数据库 migration 与 production downgrade
仍不在本次授权范围内。

Runner backend 生命周期证据现已完成最小闭环：workflow 直接执行构建后的 Rust binary，保存
PID 与后端日志；cleanup 在成功 manifest 和 artifact 上传之前运行，并输出 `not_started`、
`already_exited`、`terminated`、`forced_kill` 四态 JSON。Server 提前退出或只能强制 KILL 均失败
关闭，只有正常 TERM 退出才允许继续生成成功证据。该合同已有 11 条防漂移测试和本地 TERM、
already-exited、forced-KILL 行为探针保护，但它仍属于 R0.3 的静态与本地证据准备。

R0.3 的剩余硬化不拆成新阶段：在真实 GitHub Runner 采证前，使用 Linux 原生
`/proc/<pid>/exe` 核验被清理进程与启动 binary 一致，并记录 binary SHA-256；若身份不匹配，
必须保留诊断、拒绝向未知 PID 发信号并阻止成功 manifest。由于该能力只增强 Runner 证据
可信度，实施时机固定在 R0.2 通过之后，不得用它替代 R0.1/R0.2，也不为 Windows 本地探针
引入可注入伪 `/proc` 等额外复杂度。

完成标志：R0.1、R0.2、R0.3 全部通过，生产环境实际运行的 Rust 后端受到 CI 和真实 E2E
直接保护。仅 workflow 语法正确或 `/health` 返回 200 均不能单独视为完成。

#### R1：后台任务快照原子化

状态：**实现完成，并通过本地 Rust 质量门禁**（2026-07-12）。

已落地固定 primary/backup/temp 双槽协议、损坏候选隔离、Windows 双 rename 提交、
提交失败 rollback、进程内保存 mutex，以及 primary → backup → temp 启动回退。

验证证据：定向测试 9/9、完整 Rust 测试 1533/1533、fmt、check 和 Clippy
correctness+suspicious 全部通过。

完成标志：进程崩溃或机器异常不能轻易留下半写 JSON，并且旧快照可用于降级恢复。
业务级恢复分类不在 R1 内，继续由 R2 承担。

#### R2：后台任务恢复策略注册表

状态：**实现完成，并通过本地 Rust/前端质量门禁**（2026-07-12）。

每个 `task_type` 已登记一种恢复等级：

- `restartable`：可从原始输入安全重启；
- `checkpoint_resumable`：可从业务 checkpoint 继续；
- `manual_confirmation`：重启前必须人工确认；
- `non_resumable`：只能明确失败并保留诊断信息。

已落地 23 项唯一静态注册表（5 个 restartable、2 个 checkpoint_resumable、16 个
manual_confirmation）及 unknown/non_resumable 安全 fallback。恢复只投影可操作终态，不自动
重放缺失 payload；章节批量/单章恢复继续复用现有数据库 runtime-state owner。

前端字段链路原本已存在，本轮补充共享 `getTaskRecoveryGuidance()`，在任务中心为
`restart_required`、`resume_available`、`checkpoint_missing`、`manual_review` 和
`non_resumable` 显示明确操作指引；“继续”按钮仍只对原有章节 resume owner 开放。

验证证据：恢复测试 11/11（含日志隐私、陈旧候选保护和重复恢复幂等合同）、启动持久化
源码合同 1/1、启动恢复原子 owner 防漂移合同 1/1、`TaskRegistry::update_if()` 原子 primitive
测试 4/4、执行器覆盖防漂移合同 1/1、前端类型/Owner/恢复策略跨层合同 1/1、production CI
contracts 15/15、TaskRecord 兼容测试 2/2、`TaskStreamHub` 并发与生命周期测试 3/3、后台任务 API
测试 25/25（含 terminal record 拒绝迟到更新后 channel bridge 自行终止、Pending 准入/取消并发合同、
授权—订阅间隙 connected 快照补偿，以及 lag 后快照重同步/旧缓冲丢弃）、完整 Rust 测试
1610/1610、mock Playwright 恢复语义合同
1/1、fmt、
check、Clippy correctness+suspicious、前端
build 和 lint 全部通过。日志隐私合同将孤儿恢复日志严格限定为 `task_id`、`task_type`、
`recovery_policy`、`projected_status`，并拒绝 payload、result、checkpoint 或整条 record。恢复写入
会在 registry 写锁内复核最新状态并使用最新记录生成投影，陈旧候选不会覆盖并发完成/取消的
终态，重复恢复也不会二次改写诊断。恢复只刷新 `completed_at`/`updated_at`，并精确保留
`started_at`（包括 `None`）；未真正进入 running 的 pending 孤儿任务不会被伪造执行开始时间。
同一次恢复投影只采样一个事实时间，并同时写入 checkpoint `updated_at`、任务 `completed_at` 和
`updated_at`，避免诊断 checkpoint 与终态记录出现微秒级时间漂移。启动 orphan recovery 也已统一
复用 `TaskRegistry::update_if()`：active predicate 与投影在同一原子 owner 中执行，不再通过普通
`update()` 的闭包内早退和闭包外 mutable metadata 回传实现；生产源码合同会直接拒绝该 owner
退化，该收敛不改变恢复策略、API 或 Schema。

运行期 Generic TaskRecord 终态单调性也已补强：`Pending -> Running` 通过原子条件更新取得执行
准入，pending 任务被取消后延迟 spawn 不再启动业务执行；`Completed`、`Failed`、`Cancelled` 均为
不可回退终态，迟到 executor 结果和 channel progress/message 不再覆盖终态或 recovered `Failed`
的恢复语义。取消在单一 registry 写锁内完成用户归属、active 状态、checkpoint 与终态投影，消除
`get() -> update()` TOCTOU；channel `success` 只传递 active 数据，最终 `Completed` 及其 result、
`completed_at` 统一由 `complete_task()` 拥有。迟到 lifecycle owner 被原子 predicate 拒绝后也不会向
SSE 订阅者广播虚假的 progress/result/error 事件，确保 registry 与 stream 投影一致。
`TaskStreamHub` 的首次 sender 创建现已在单一写锁内原子完成，两个并发首次订阅者共享同一
broadcast channel；fanout 会等待 sender-map 锁而非因 `try_read()` 竞争静默丢事件。最终
`done`、`error`、`cancelled` 会在发送时原子移除 sender：现有 receiver 消费终态后关闭，后续重连
创建新 channel 并从 connected 快照读取终态，避免历史任务 sender 在进程内无界累积。SSE 路由还会
先订阅、再刷新 connected 快照，使授权与订阅之间发生的终态转换由最新快照或队列事件覆盖。慢订阅者
发生 broadcast lag 时会对同一 receiver `resubscribe()` 到 channel 尾部，再从 `TaskRegistry` 发送最新
`connected` 快照；旧 progress 缓冲不会在最新状态后回放，terminal 快照发出后 stream 立即关闭。
已经进入 Running 的底层 AI/数据库操作仍缺少统一 cooperative cancellation token，本轮只保证状态机和迟到
registry 写入安全；深度取消应另立设计任务。
Playwright 合同覆盖恢复终态到 Zustand 与任务中心操作的前端链路，不作为 PostgreSQL/Rust
真实 E2E 证据。
启动顺序已固定为 snapshot load → orphan recovery → conditional immediate save → periodic workers
→ router；通用 `execute_task()` 的 20 个唯一类型均有显式恢复策略。前端 24 个唯一任务类型固定为
23 个 Rust 已知策略类型加一个 `unknown` sentinel，且执行器之外只允许 3 个明确独立 owner；这不
改变 snapshot version、任务 Schema、unknown API 兼容行为或 best-effort 保存边界。

完成标志：服务重启后，系统不再用同一种失败策略处理所有活跃任务；前端能够显示
准确、可操作的恢复状态。该标志已在本地实现和测试层满足。

#### G0：生产可靠性门禁

只有同时满足以下条件，才能进入 Phase 1：

- R0.1 的密码哈希容量契约和升级迁移通过真实 PostgreSQL 回归；
- R0.2 本地 Rust E2E 与 R0.3 GitHub runner E2E 均为绿色；
- 快照写入具备原子性和损坏处理测试；
- 现有长任务全部登记恢复等级并向前端提供可操作语义；
- 任务失败、重启、重试不会产生明显重复写入；
- 没有新增第二套后台任务基础设施或第二套章节 checkpoint/resume owner。

当前判定（2026-07-14）：**G0 通过（GO）**。R0.1、R0.2、R0.3、R1、R2 均已满足，
R3-R6、G1-Cancel 与 G1 已完成；R7 已解除 G1 前置阻塞，且已完成受控 Tool Contract 与最小任务/Coordinator 两个纵切，整体仍在实施中。

G0 逐项审查证据：

| 条件 | 判定 | 权威证据 |
|---|---|---|
| R0.1 PostgreSQL 密码哈希容量与迁移回归 | PASS | fresh/old-head PostgreSQL 18、20 revisions / 120 SQL、Argon2 登录升级与 guarded downgrade 失败关闭 |
| R0.2 本地 E2E + R0.3 Hosted Runner E2E | PASS | 本地 Playwright 14/14；GitHub Runs `29264634399`、`29264634424`；artifact `8284985490` 与 ZIP SHA-256 核对 |
| 快照原子性与损坏处理 | PASS | primary/backup/temp 原子提交、flush/sync、损坏隔离、并发与失败回滚定向测试；完整 Rust 门禁通过 |
| 长任务恢复等级与前端语义 | PASS | `TASK_RECOVERY_POLICIES` 覆盖 24 个唯一生产 task type，四类策略、未知类型降级和前端操作指引均有测试 |
| 失败、重启、重试的重复写入防护 | PASS | `repeated_recovery_is_idempotent`；恢复后立即原子保存；有副作用任务不静默自动重放；终态拒绝迟到覆盖 |
| 单一基础设施与 checkpoint owner | PASS | 继续复用 Rust `TaskRegistry`、`/background-tasks` 与章节 runtime snapshot/upsert owner；未新增第二套任务或 checkpoint 系统 |

### 16.4 Phase 1：统一创作状态与生成契约

#### R3：小说级 Workflow State Machine

先统一“项目处于哪个创作阶段”，不直接承担后台执行状态。状态机应建立合法转换、
版本号和人工回退规则，并复用现有 workflow service，避免出现第二套业务事实。

当前判定（2026-07-14）：**R3 已完成本地实现和质量证据**。`projects.status` 已成为唯一小说级
阶段事实，九阶段转换、兼容入口、CAS 冲突、Rust API、项目详情 UI 和 focused Playwright 均已
落地；完整 Rust 测试 1646/1646 通过，前端 build/lint 通过。严格全仓 Clippy 仍受非 R3 历史
基线错误阻塞，R3 新增 owner 文件无命中；当前 SQLite/mock CAS 证据也不宣称替代真实
PostgreSQL 隔离级并发验证。R3 不新增数据库 migration、工作流表或第二状态字段。

#### R4：Story Packet / Generation Intent

统一大纲、章节、重生成、审校和 Autopilot 的生成输入。所有现有入口通过兼容适配层
归并到同一个核心契约，不要求一次删除旧 API。

当前判定（2026-07-15）：**R4 已完成本地实现和质量门禁**。统一 Rust schema owner、canonical
SHA-256 digest、单章/批量/恢复/重生成/大纲/审校适配、版本化 runtime snapshot 与 optional
history summary 已落地；旧 API DTO、response、SSE event kind、前端请求类型和 Zustand/task store
保持兼容。当前工作树 focused suites 全部通过，完整 Rust 测试 1689/1689、`cargo fmt --check`、
`cargo check`、前端 lint（0 error / 33 existing warnings）和 build 均通过；R4 未新增 migration、
第二套任务系统、第二个小说阶段事实或 Coordinator。R5、R6、G1-Cancel 与 G1 后续均已完成，当前主线正式切换为 R7。

#### R5：角色级模型策略

在统一 Generation Intent 之后，再引入 planner、writer、reviewer 等角色的模型策略。
必须记录实际 Provider、模型、fallback 原因和配置版本，默认配置保持简单。

#### R6：业务 checkpoint 标准

checkpoint 以业务边界为单位，例如“大纲已确认”“章节草稿已保存”“审校结果已生成”，
而不是尝试从任意 Token 位置继续。必须包含 revision、幂等键、输入摘要和输出引用。

当前判定（2026-07-16）：**R6 已完成本地实现和质量门禁**。首个
`business-checkpoint/v1` 边界为 `chapter_draft_saved`，checkpoint additive 写入现有
`workflow_runtime_state.business_checkpoint`，复用 R4 `input_digest`，并以 typed allowlist
生成 revision、canonical `sha256:` 幂等键和 chapter output reference。resume 会在 reset 和
runtime dispatch 前校验 schema、payload、幂等键、digest、task/project scope、chapter 存在性与
非空正文；legacy missing 保持旧恢复路径。

真实 SQLite/SeaORM 测试已经执行 contract snapshot → chapter content save →
`chapter_succeeded.persist()` → 后续 `failed.persist()` → resume，证明 checkpoint 在既有 snapshot
merge 后保留且从下一章继续。business checkpoint 14/14、resume 81/81、runtime state 137/137、
完整 Rust tests 1755/1755、`cargo fmt --check` 与 `cargo check` 全部通过；未新增 migration、表、
公开 API/SSE 变化或第二套 task/checkpoint system。

#### G1：统一契约门禁

只有同时满足以下条件，才能进入 Phase 2：

- 项目级阶段只有一个权威来源；
- 核心生成入口能够生成统一 Story Packet；
- 模型选择和 fallback 可追溯；
- 至少一种长流程通过业务 checkpoint 完成恢复验证；
- 旧页面和旧 API 仍可通过兼容门面工作；
- 状态机、任务状态和 checkpoint 的职责边界有文档和测试保护。

当前判定（2026-07-16）：**G1 通过（GO）**。六项条件均具备实现 owner 与测试/质量证据；审查中发现的
replacement registration 未取消旧 token 缺口已修复，完整 Rust tests 1761/1761 通过。非阻塞风险与
逐项证据见 `docs/19-g1-contract-gate-review.zh-CN.md`。R7 已解锁并在同日完成受控 MVP 收口；其后 G2 固定样本与 failure-injection 安全门禁也已判定为 GO。

### 16.5 Phase 2：受控 Autopilot MVP

#### R7：只实现一个最小闭环

Autopilot 第一版只覆盖：

```text
基础设定确认 -> 大纲生成 -> 人工确认 -> 单章生成 -> 审校 -> 人工验收
```

实施顺序固定为：

1. **已完成**：定义受控 Tool Contract；
2. **已完成**：将唯一 allowlisted Tool 映射到现有 Rust Service；
3. **已完成**：新增单次、非可恢复的 `novel_autopilot` 后台任务；
4. **已完成**：实现只执行一条已确认 Tool 调用的最小 Coordinator；
5. **已完成（第三纵切）**：增加认证、项目作用域固定的控制 API 与一次性人工确认 gate；
6. **已完成（第四纵切）**：新增项目 owner 范围的 durable invocation audit；任务创建前持久化 `queued` 事实，成功与 workflow CAS 原子提交，失败/取消仅记录脱敏稳定码；
7. **已完成（第五纵切）**：项目 workflow 面板提供按需加载、owner-scoped、脱敏的只读 invocation history；该展示不拥有 task runtime、checkpoint 或 recovery/replay，也不提供 Pause、Resume、Steer、重放或自动重试。

Coordinator 无权直接写数据库、绕过权限检查或自行解释内部表结构。业务事务、幂等、
状态转换和数据写入仍由 Rust Service 负责。

#### R7：最终验收收口（2026-07-16）

**判定：PASS（受控 MVP）**。本判定只覆盖已批准的单次、人工确认、
`transition_project_workflow` 受控调用；它不授权多 Tool 编排、Provider/Prompt
运行时调用、Pause/Resume/Steer、checkpoint/recovery/replay、自动重试，或无人值守
整书/多卷生成。

| 验收项 | 通过证据 |
| --- | --- |
| 可信 scope、actor 与确认 | API 仅从 route 注入 `project_id`、仅从 Claims 获取 actor，严格 DTO 拒绝 body scope/actor 注入，并要求 `confirmed_by_user=true`。 |
| 唯一执行链 | action API 只创建既有 generic `novel_autopilot` task；Coordinator 只调用受控 Tool Contract；workflow CAS 仍由 `novel_workflow_service` 负责。 |
| Tool schema 与业务边界 | 静态 allowlist 仅暴露 `transition_project_workflow`；参数、公共 phase、确认与项目 scope 在写入前校验。 |
| audit 与失败安全 | queued audit 先于执行持久化；workflow 成功与 succeeded audit 同事务；失败/取消只记录稳定、脱敏错误码。 |
| history 隐私与 UI | history route 先验证 owner access，再以显式 allowlist 投影内部 audit read model；前端仅按需展示安全摘要，不派生控制或恢复操作。 |
| 可重复验证 | 2026-07-16 重跑 `cargo fmt --check`、`cargo check -j 1`、`cargo test -j 1 autopilot`（25/25）、frontend lint/build 及 workflow Playwright（5/5）均通过。 |
| 负向能力约束 | 未引入第二套 task/workflow owner、自动重试、多步骤自治、Provider/MCP、Pause/Resume/Steer、checkpoint/recovery/replay；`novel_autopilot` 仍显式 NonResumable。 |

详细需求到源码/测试的映射见
`docs/20-r7-controlled-autopilot-mvp-acceptance-review.zh-CN.md`。R7 的任务目录仍保留
为可审计实现证据；不因收口结论自动执行 Git 提交或归档。

#### G2：自动驾驶安全门禁

Autopilot 扩大范围前必须满足：

- 当前 Autopilot 不提供 Pause/Resume；若未来扩展为可中断任务，必须先定义业务 checkpoint owner、恢复幂等与显式人工确认，再通过 G2 审查；
- 关键阶段默认需要人工确认；
- Tool 输入输出有 schema 验证；
- 每次直接业务 Tool 调用可追溯到任务、Tool/schema、用户确认事实、脱敏输入摘要和结果/稳定错误码；Provider、模型、Prompt 不适用时必须为 `NULL`，且不得保存原始 Prompt；
- Coordinator 失败不会破坏人工工作台；
- 最小闭环有固定样本回归测试和失败注入测试。

G2 最终判定（2026-07-16）：**GO（仅当前受控单次 MVP 安全门禁）**。固定 fixture、queued/terminal audit failure injection、CAS/Tool 拒绝、owner history 故障和前端只读回归均已通过；完整证据见 `docs/21-g2-autopilot-safety-gate-review.zh-CN.md`。该 GO 不授权多卷整书无人值守生成，且不授权 Pause/Resume/Steer、checkpoint/recovery/replay、自动重试、Provider/MCP runtime 或多 Tool/多步骤自治。

### 16.6 Phase 3：评测、审计和反馈闭环

#### R8：按真实运行数据扩展

实施顺序为：

1. Prompt/Tool Eval 和 golden sample；
2. 创作档案包导出与脱敏；
3. 质量指标和任务运行指标关联；
4. 根据真实失败样本提出未来独立 Autopilot gate 的候选输入；R8 本身不扩展 Tool 或执行流程；
5. 评估是否需要 Headless API/CLI，而不是预先复制 TUI。

完成标志：每次质量变化都能关联到工作流版本、模型策略、Prompt、Tool 调用和人工反馈，
新能力通过评测数据而不是主观判断进入生产路线。

### 16.7 明确延期和禁止并行的事项

以下事项不进入当前 Roadmap v1.3：

- 整体迁移到 Go；
- 使用本地文件替代 PostgreSQL；
- 用 TUI 替换 Web 创作工作台；
- 新建第二套后台任务或第二套项目阶段状态；
- 将事务、权限和业务规则迁移到 Coordinator Prompt；
- 承诺从任意 Token 位置断点续跑；
- 在 G1 之前开发整书 Autopilot；
- 在没有真实使用数据前提前设计多 Coordinator 集群。

### 16.8 路线变更规则

路线只有在以下情况之一发生时才能调整：

1. 生产事故要求插入紧急可靠性任务；
2. 阶段验证证明核心技术假设不可行；
3. 产品目标明确取消或提前 Autopilot；
4. 数据合规、Provider 政策或基础设施发生重大变化。

路线调整必须记录：变更原因、受影响阶段、兼容性影响、回滚方式和新的验收门禁，不能仅
通过临时实现改变架构方向。

---

## 17. 已确定的 Trellis 任务拆分

每项任务独立规划、实现、验证和回滚。任务编号表示执行顺序，不表示可以并行跳过依赖。

| 顺序 | 优先级 | Trellis 任务 | 前置条件 | 主要交付物 |
|---|---|---|---|---|
| R0.1 | P0 | PostgreSQL Auth Schema Compatibility（已完成） | workflow 静态实现 + 明确 Schema 授权 | TEXT 容量、升级 migration、初始 schema、隔离 PostgreSQL auth/guard 证据 |
| R0.2 | P0 | 本地 PostgreSQL + Rust + Playwright 真实 E2E（已完成） | R0.1 | 20 revisions / 120 SQL、readyz/releasez、Playwright 14/14、cleanup 后 success 证据 |
| R0.3 | P0 | GitHub runner 真实 E2E 证据（已完成） | R0.2 | Runs `29264634399` / `29264634424`、lifecycle、binary identity/hash、artifact `8284985490` |
| R1 | P0 | 后台任务快照原子化（已实现，本地门禁通过） | 无未完成前置 | 原子写、损坏恢复、跨平台测试 |
| R2 | P0 | 后台任务恢复策略注册表（已完成并冻结） | R1 | 恢复分类、孤儿任务处理、恢复结果即时持久化、前端可操作指引、慢订阅者快照重同步 |
| R3 | P1 | 小说级 Workflow State Machine（已完成，本地门禁通过） | G0 | 九阶段 schema、转换 owner、CAS API、项目进度 UI、兼容入口与 focused E2E |
| R4 | P1 | Story Packet 统一生成契约（已完成，本地门禁通过） | R3 | schema、归并服务、快照、兼容门面 |
| R5 | P1 | 角色级模型策略（已完成，本地门禁通过） | R4 | 配置、解析、fallback、历史记录 |
| R6 | P1 | 可恢复业务 checkpoint 标准（已完成，本地门禁通过） | R2、R4 | schema、幂等、恢复测试 |
| R7 | P1 | 受控 Autopilot MVP（已完成，2026-07-16） | G1 | 受控 Tool、最小任务/Coordinator、控制 API/人工 gate、durable invocation audit 与 owner-scoped readonly history 已完成；审计不等于恢复/重放能力，仍保持单次、人工确认、NonResumable 边界 |
| R8 | P2 | Eval、档案与指标闭环（已完成，2026-07-16） | G2 | 静态脱敏 fixture、只读创作档案、运行指标与可追溯验证；不新增控制或执行能力 |

下一项路线工作已经确定为：

> **R4、R5、R6、G1-Cancel 与 G1 已完成本地实现、质量证据和门禁审查。
> G1 已于 2026-07-16 判定为 GO；R7 已完成受控 Tool Contract、最小任务/Coordinator、控制 API/人工 gate 与 durable invocation audit 四个纵切，继续维持人工确认与非可恢复边界。**

R1、R2 已完成本地实现和质量门禁，R2 功能范围已经冻结。R2 的恢复策略、前端可操作指引、
终态单调性、sender 生命周期和 SSE 慢订阅者 lag 快照重同步均已闭环；孤儿任务恢复投影会
在周期保存启动前按恢复数量条件即时写入现有原子快照，避免服务在首个 1.5 秒保存窗口前
再次退出时遗留旧 active snapshot；源码合同还会从真实 `execute_task()` match 提取通用执行
类型并阻止漏登记恢复策略。该加固不改变 snapshot version、任务 Schema、unknown API 兼容
行为或恢复策略。
R0.1 已完成授权范围内的全部源码和隔离 PostgreSQL 验证：Rust/Python revision graph 为 20 项，
head 为 `20260712_password_hash_phc_text`；production `migration-executor` 继续保持 upgrade-only。
guarded `downgrade_steps` 只作为 catalog/source-map 合同，并已证明长 verifier 存在时失败关闭，
本路线不新增、不执行生产 downgrade CLI。R0.2 本地真实 E2E 与 R0.3 GitHub Hosted Runner
证据均已完成，G0、R3、R4、R5、R6、G1-Cancel、G1、R7 与 G2 已通过；R7 保持受控 Tool Contract、最小
`novel_autopilot` 任务/Coordinator、控制 API/人工 gate 与 durable invocation audit 的单次闭环，G2 仅证明该闭环在故障下安全，仍不得扩张为无人值守多卷生成。

当前优化路线据此固定为五段，不再并行扩张功能面：

1. **R0.1 已完成**：`password_hash` 容量、migration catalog/head、initial schema 与隔离
   PostgreSQL auth 回归已形成证据；不代表已迁移生产数据库。
2. **R0.2 已完成**：PostgreSQL migration、Rust server、auth、后台任务与 Playwright 14/14 已形成
   可重复本地证据，成功清单在 runtime/container cleanup 后生成。
3. **R0.3 Runner 证据**：把同一链路搬到 GitHub runner，保留 binary identity/hash、生命周期和
   失败诊断 artifact；禁止用 mock 或仅静态合同替代。
4. **G0 审查后的 R3-R6、G1-Cancel 与 G1 已完成**：小说级状态机、Story Packet、模型策略、业务
   checkpoint 与 cooperative cancellation 均复用 Rust owner 和现有兼容入口，未创建第二套工作流或任务系统。
5. **R8 已完成**：R7 与 G2 的安全边界下，R8 已交付 Eval、脱敏创作档案和运行指标；继续保留人工确认、NonResumable 与既有回滚边界，不新增 pause、恢复或控制能力。

除非门禁证据发生变化，后续优化任务应按此顺序执行；性能微调、UI 扩展、
新 Agent/Coordinator 功能均不得绕过 R0.1-R0.3 与 G0。

### 17.1 最终执行路线决议（Roadmap v1.7，2026-07-16）

R0.1、R0.2、R0.3、G0、R3、R4、R5、R6、G1-Cancel 与 G1 已完成，后续工作继续固定为一条主依赖链；
R7 已完成受控 Autopilot MVP 收口，G2 已完成安全门禁审查，R8 亦已完成；当前没有待开发的 R9，不并行扩张任何无人值守或控制/恢复产品功能。

```text
已完成：R2-SSE-Lag 收口，R2 已冻结
  |
  +-- 当前 P0 主线
          R0.1 PASS -> R0.2 PASS -> R0.3 PASS -> G0 PASS
                                    |
                                    v
                 R3 PASS -> R4 PASS -> R5 PASS -> R6 PASS
                                                            |
                                                            v
                                           G1-Cancel PASS -> G1 GO
                                                                 |
                                                                 v
                                                     R7 PASS -> G2 GO -> [CURRENT] R8
```

R5、R6、G1-Cancel、G1、R7、G2 与 R8 已完成并冻结兼容边界。当前没有后续 R9；任何新路线
必须独立立项，并且不得用 UI、Agent 或新的后台任务基础设施绕过既有 owner 和前置验收。

#### 当前即时队列

| 顺序 | 当前动作 | 状态 | 完成条件 |
|---|---|---|---|
| 1 | R2-SSE-Lag：lag 后 resubscribe、读取最新记录并发送既有 `connected` 快照 | **已完成** | API 25/25、完整 locked Rust 1610/1610、fmt、check、Clippy 通过；未新增事件类型和 Schema |
| 2 | 冻结 R2 | **已完成** | 恢复策略、终态单调性、sender 生命周期和 lag 重同步形成稳定合同；不再吸收跨业务取消架构 |
| 3 | R0.1 PostgreSQL Auth Schema Compatibility | **已完成** | 容量、catalog/head、initial schema 和隔离 PostgreSQL auth 回归通过 |
| 4 | R0.2 本地真实 E2E | **已完成** | PostgreSQL migration、Rust `/readyz`/`/releasez`、auth、后台任务页面和 Playwright 14/14 全绿 |
| 5 | R0.3 Runner 证据 | **已完成** | Runs `29264634399` / `29264634424`；真实 binary、SHA-256 provenance、生命周期、成功 manifest 与 artifact `8284985490` 完整 |
| 6 | G0 审查 | **GO（2026-07-14）** | R0.1、R0.2、R0.3、R1、R2 全部满足且证据可审计；R3 已解除阻塞 |
| 7 | R3 小说级 Workflow State Machine | **已完成，本地门禁通过** | 九阶段 owner、CAS/409、兼容写入口、Rust API、项目详情 UI；Rust 1646/1646、前端 build/lint、focused E2E 1/1 通过 |
| 8 | R4 Story Packet / Generation Intent | **已完成，本地门禁通过** | 统一输入 owner、digest、各生成入口适配、snapshot/history 兼容；Rust 1689/1689、前端 lint/build、兼容审计通过 |
| 9 | R5 Role Model Policy | **已完成，本地门禁通过** | Role Policy、解析优先级、tracked execution、实际 Provider/model/fallback 审计；Rust 1736/1736、digest/安全/兼容审计通过 |
| 10 | R6 Business Checkpoint | **已完成，本地门禁通过** | `business-checkpoint/v1`、monotonic revision、canonical 幂等键、R4 digest、typed output 与 DB-backed success→failure→resume；Rust 1755/1755 |
| 11 | G1-Cancel Cooperative Cancellation | **已完成，本地门禁通过** | 统一 token/registration owner、generic/batch 传播、commit 后 signal、terminal CAS、迟到结果拒绝、failure injection、cancel-vs-completion 竞态；Rust 1761/1761 |
| 12 | G1 统一契约门禁 | **GO（2026-07-16）** | 六项统一契约均有 owner 与测试/质量证据；replacement cancellation 缺口已修复；审查报告见 `docs/19-g1-contract-gate-review.zh-CN.md` |
| 13 | R7 受控 Autopilot MVP | **PASS（2026-07-16）** | 单次非可恢复任务、最小 Coordinator、认证项目作用域控制 API/人工 gate、durable invocation audit 与 owner-scoped readonly history 均已收口；不提供 Pause/Resume/Steer、checkpoint、恢复或重放。 |
| 14 | G2 Autopilot Safety Gate | **GO（2026-07-16）** | fixture、CAS/Tool 拒绝、queued/terminal audit failure injection、history 故障与 frontend readonly 回归通过；不扩大无人值守或控制/恢复能力。 |
| 15 | R8 Eval / 创作档案 / 运行指标 | **已完成（2026-07-16）** | 静态脱敏 golden sample、owner-scoped 创作档案 allowlist、`runtime-metrics/v1` 派生只读摘要和 UI/E2E 证据均已完成；不新增 Autopilot Tool 或执行流程。 |

R2 已冻结，R0.1、R0.2、R0.3、G0、R3、R4、R5、R6、G1-Cancel、G1、R7 与 G2 已完成。当前不得继续扩张后台任务基础设施；
R8 已完成；任何下一主线必须独立立项，UI 或 Agent 功能不得绕过既有 owner、人工确认与已冻结的安全边界。

#### R5 完成证据与边界（2026-07-15）

- `role-model-policy/v1` 建立 canonical role mapping，以及
  `route > role > global > default` 的 provider/model 分字段解析；provider 切换不会继承不兼容模型。
- `AIService` 保留旧公开方法，并新增 tracked non-stream/stream execution；实际 provider/model、
  model fallback、endpoint failover、candidate fallback 与失败分类统一进入 typed audit。
- single、batch create/resume、regeneration、Wizard Outline、Batch Outline、ChapterReview/Repair
  均按既有契约返回有序 `generation_execution_audit`；核心章节生成 history additive 持久化。
- canonical audit 采用 allowlist，不保存 Prompt、正文、API key、Authorization 或完整 endpoint URL；
  未新增 migration、SSE event kind，未删除或重命名旧公开字段。
- R4 `input_digest` 忽略 policy/provider/model/fallback 等 runtime-only 字段，专项回归 1/1 通过。
- `cargo fmt --check`、`cargo check` 与完整 Rust tests 1736/1736 通过；R5 无 frontend 契约变化，
  frontend lint/build/E2E 记录为 N/A。
- ChapterReview 的审计边界是 analysis result；后台 wrapper 和现有数据库没有独立 durable audit
  字段。R5 不新增 checkpoint/schema，也不污染 `analysis_report` 等业务字段。该 durable gap 由
  R6 Business Checkpoint 在独立版本化业务边界中评估，而不是在 R5 临时增加 migration。


#### R6 完成证据与边界（2026-07-16）

- `business-checkpoint/v1` 使用单一 typed owner，字段仅包含 schema、boundary、revision、
  canonical idempotency key、R4 `input_digest`、typed chapter output reference 与 recorded time。
- `chapter_succeeded` 在章节正文已持久化后，通过现有 runtime persistence/snapshot merge owner
  写 checkpoint；revision 取成功章节数与已有 revision 的最大值，legacy 缺 contract 时保持旧行为。
- resume 在 task reset 与 runtime dispatch 前完成 checkpoint schema/payload/key/digest/output 校验；
  unknown、invalid、mismatch、missing、empty、task/project out-of-scope 均返回固定 typed error。
- DB-backed 证明执行真实 `chapter_succeeded.persist()`，再写入后续失败并恢复 chapter-2；checkpoint
  在 additive merge 中保留，旧 `workflow_runtime_state.checkpoint` 与其他 runtime 字段未被替换。
- checkpoint allowlist 和错误文本不保存或回显 Prompt、正文、API Key、Authorization、完整 URL；
  未新增 migration、表、task store、公开 API 必填字段或 SSE event kind。
- `cargo fmt --check`、`cargo check`、business checkpoint 14/14、resume 81/81、runtime state 137/137
  与完整 Rust tests 1755/1755 全部通过；frontend 契约无变化，lint/build/E2E 记录为 N/A。

#### R7 首个受控 Tool Contract 纵切完成证据（2026-07-16）

- 新增 `autopilot-tool-contract/v1` 内部 registry，目前静态 allowlist 仅暴露
  `transition_project_workflow`；provider `ToolDef` 仅承载 schema 投影，不是执行安全边界。
- dispatcher 固定执行 `allowlist -> 严格 typed 参数解析 -> 用户确认 ->
  novel_workflow_service::transition`；actor `user_id` 只来自内部 execution context，拒绝 JSON
  注入、未知字段、非 object、未知 phase 与空 project id。
- 写入仍完全委托既有 workflow owner 的权限检查、合法 phase 转换和 `expected_phase` CAS；未新增
  HTTP/SSE API、migration、后台 task、Coordinator、第二套 workflow/task/checkpoint 事实或直接 SQL。
- `autopilot_tool_contract_service` 5/5、`novel_workflow_service` 17/17 聚焦测试通过；使用
  `rust-lld`、`debuginfo=0` 与 `/DEBUG:NONE` 的测试进程规避本地 MSVC `LNK1318` 后，完整 Rust
  tests **1766/1766** 通过。`cargo fmt --check` 和 `cargo check` 通过；后者仅保留工作区既有
  51 条 warning，R7 新模块未新增 warning。
- 首个纵切不等于 R7 完成。其后已完成最小 Coordinator 与单次、非可恢复的 `novel_autopilot`
  后台任务；仍未开发 typed control API、人工 gate、Pause/Resume/Steer、任务中心控制面、durable Tool
  audit、更多业务 Tool、可恢复 Autopilot invocation，以及 G2 所需的 Eval/取消联动与安全验收。

#### R7 第二纵切：最小后台任务与确定性 Coordinator 完成证据（2026-07-16）

- 新增严格 `NovelAutopilotTaskPayload`：仅接受 `tool_name`、原始 `arguments` 与
  `confirmed_by_user`，拒绝未知字段；actor 与 project scope 分别只能来自 `TaskRecord.user_id` 和
  `TaskRecord.project_id`，payload 不可注入 user/project。
- Tool Contract execution context 增加可选 canonical `project_scope`。后台任务调用时，
  `transition_project_workflow.arguments.project_id` 必须与 `TaskRecord.project_id` 相同；不匹配会在
  workflow 写入前 fail closed。无 task scope 的既有 direct Tool 调用保持兼容。
- `novel_autopilot` 进入既有 `spawn_task_execution -> execute_task -> complete_task/fail_task` 生命周期；
  沿用 cancellation、TaskRegistry、SSE 和 result projection，未创建第二套 terminal owner、task store 或 SSE kind。
- Coordinator 不调用 Provider/MCP、不读取 Prompt、不自行解析自然语言，也不直接写数据库；它只把已确认
  Tool 调用委托给 `autopilot_tool_contract_service -> novel_workflow_service::transition`，并仅将版本化
  `autopilot-tool-contract/v1` receipt 写入既有 task result。
- 因 `TaskRecord` 不持久化 payload、确认和 Tool arguments，`novel_autopilot` 显式登记为
  `NonResumable`；进程重启时只投影“不可恢复，请重新发起”，绝不 replay 写操作。
- 前端只补充 `BackgroundTaskType` 与“小说自动驾驶”展示标签；本纵切未新增 Autopilot 启动入口、控制 UI、
  API、migration、checkpoint/audit 表或 Provider Tool Calling。
- `cargo fmt --check`、`cargo check`、Tool Contract 7/7、Coordinator 3/3、background task 27/27、
  recovery 12/12、production contract 16/16、完整 Rust suite、frontend lint/build 均通过。前端 lint 的
  既有 Hook 依赖 warning 与 R7 文件无关；本纵切文本已验证 UTF-8 无 BOM、LF-only、无尾随空白。

R7 仍为 **IN PROGRESS**：认证、项目作用域固定的控制 API 与一次性人工 gate 已完成；后续必须先以独立任务审查 Pause/Resume/Steer、durable audit、任务中心控制面、更多 allowlisted Tool、可恢复 invocation 与多步骤自治的 owner、恢复和审计边界。这些能力完成并满足 G2 前，禁止扩展为无人值守多卷/整书生成。


#### R7 第三纵切：认证控制 API 与一次性人工 gate 完成证据（2026-07-16）

- 新增唯一入口 `POST /api/projects/:project_id/autopilot/actions`；actor 仅来自认证 `Claims`，
  项目作用域仅来自 route path，并在创建 task 前复用既有项目访问校验。
- 请求顶层与 `arguments` 均拒绝未知字段，拒绝请求体注入 `project_id` / `user_id`，仅接受
  `transition_project_workflow`、canonical public phase 与明确的 `confirmed_by_user: true`。
- route 只构建已确认的 `novel_autopilot` invocation 并委托既有 generic task lifecycle；它不直接
  调用 workflow service、不写 SQL、不创建第二套 task、SSE、terminal、checkpoint 或 recovery owner。
- 修复 generic task lifecycle 的严格 payload 边界：`novel_autopilot` 原样接收严格 invocation
  payload，`project_id` / `user_id` 的可信 authority 仍在 `TaskRecord`；其它 task type 保持既有
  runtime enrichment，从而不放宽 Coordinator 的 `deny_unknown_fields`。
- API 集成测试覆盖 `API create -> generic task -> queued audit -> Coordinator -> Tool Contract ->
  novel_workflow_service::transition -> terminal audit -> readonly history`，并断言 result receipt schema 为
  `autopilot-tool-contract/v1`、workflow/audit phase 一致，以及 history response 只含 UI allowlist。
- `cargo fmt --check`、`cargo check`、API 4/4、background tasks 29/29、Coordinator 3/3、
  Tool Contract 7/7、workflow 17/17 以及完整 Rust suite **1779/1779** 均通过。未改动前端
  源码，因此未重复执行 frontend lint/build。

R7 仍为 **IN PROGRESS**。本纵切不是 Pause/Resume/Steer、任务中心控制 UI、
可恢复 invocation、更多 Tool 或多步骤自治的授权，更不代表 G2 已通过。

#### R7 Workflow Panel Launch 完成证据与边界（2026-07-16）

- 完成 R7 的第四个可验证纵切：既有 Workflow Panel 增加独立的“后台受控切换”入口；
  用户只能从服务端提供的 `allowed_transitions` 选择目标，且每次都必须在确认弹窗中确认
  当前/目标阶段与可选 reason，才会创建一条 `novel_autopilot` 任务。
- 前端请求严格限定为 `transition_project_workflow`、canonical phase 与
  `confirmed_by_user: true`；project ID 只在 `/projects/:project_id/autopilot/actions` 路径，
  请求体不接受/不注入 `project_id` 或 `user_id`。成功响应复用既有 task store 同步，未建立
  第二套任务、SSE、workflow 或恢复 owner。
- 成功后仅提示并打开既有 Background Task Center；不调用原同步 `transition()`，不调用
  `updateProject()`，不对 canonical workflow phase 做乐观写入。失败保留弹窗和 reason，
  显示后端错误且不自动重试。
- 修复延迟挂载竞态：任务中心可能在首屏约一秒后才注册事件监听；一次性、非持久化的
  pending open-request 确保快速创建任务后的任务中心仍会打开。该标记仅可靠投递 UI
  view-switch intent，不保存业务任务、阶段、checkpoint 或恢复状态。
- Playwright 已覆盖严格 payload、无 scope/actor 注入、无工作流乐观变更、任务中心开启及
  `novel_autopilot` 可见性。`novel_autopilot` 的 recovery policy 仍为 `NonResumable`，
  UI 不得暗示可 resume。
- 已通过 frontend lint/build、聚焦 E2E（2/2）、Rust fmt/check 与 Autopilot API（4/4）回归。
  lint/Rust 输出的 warning 均为未改动模块的既有告警，不改变本纵切验收结论。

R7 仍然 **IN PROGRESS**。durable invocation audit 已完成；后续未开发项仍包括 Pause /
Resume / Steer、durable invocation recovery、更多 allowlisted tools、多步骤自治和 G2。**G2
判定为 GO 前，禁止开发无人值守整书或多卷生成。**

#### G1-Cancel 完成证据与边界（2026-07-16）

- 建立进程内 cooperative cancellation registry，以 scope、task ID 和唯一 registration ID 标识执行实例；
  replacement 会取消旧 token，旧实例 cleanup 不能删除新 registration，cancel/cleanup/Drop 均保持幂等。
- generic background task 与 batch startup/resume 均在生命周期 owner 使用 biased `tokio::select!`；所有 child
  progress bridge 监听同一 token，取消分支退出但不投影 `Failed`。
- runtime task patch 与 snapshot patch 在同一事务中提交，并通过 `status NOT IN
  ('completed', 'failed', 'cancelled')` 拒绝迟到结果；cancel 只对 `pending/running` 获取 terminal ownership，
  task 与 cancelled snapshot 同事务提交。
- cancellation signal 仅在持久化事务 commit 成功后发送；failure injection 证明回滚时 token 不 signal、task
  保持 `running`。barrier 驱动的 cancel-vs-completion 竞态证明只有一个 terminal owner 可以成功，且 task 与
  snapshot 终态一致。
- DB-backed 回归覆盖 cancelled/completed/failed 后迟到 preparing/success/failure patch 拒绝、replacement cleanup、
  resume checkpoint 生命周期与异步 dispatch 断言边界；未新增公开 API 必填字段、SSE event kind、状态字符串、
  schema、migration、Cargo 直接依赖、task store、项目事实或恢复协议。
- `cargo fmt --check`、`cargo check`、`cargo check --tests` 全部通过；使用 `rust-lld`、`debuginfo=0` 与
  `/DEBUG:NONE` 链接后，完整 Rust tests **1761/1761** 通过。默认 MSVC PDB 链接仍可能触发 `LNK1318:
  LIMIT (12)`，属于本地工具链环境限制，不是产品代码失败。
- cancellation 仅提供当前进程内协作停止能力，不跨进程持久化，也不等于任意 token 位置 durable resume；
  G1 已于 2026-07-16 判定为 GO；同日 R7 收口为 PASS、G2 判定为 GO，后续顺序进入 `R8`。

#### G0 之后的产品能力顺序

1. **R3 Novel Workflow State Machine**：建立唯一小说级阶段事实，不复制后台任务状态；
2. **R4 Story Packet**：统一生成输入、版本、来源和兼容门面；
3. **R5 Role Model Policy 与 R6 Business Checkpoint 已完成**：已建立可追溯模型策略和可恢复业务边界；
4. **G1-Cancel 与 G1 已完成**：已为进入 Running 的长操作建立统一 cooperative cancellation，
   覆盖 token 传播、replacement cancellation、幂等清理、迟到结果拒绝、失败注入与 terminal ownership 竞态；
5. **R7 已完成**：受控 Tool Contract、`novel_autopilot` 最小任务/Coordinator、认证项目作用域控制 API、一次性人工 gate、Workflow Panel 人工确认启动与 owner-scoped readonly history 均已形成单次受控闭环；durable invocation audit 在启动前记录 queued、与 workflow CAS 同事务投影 succeeded、对失败/取消记录稳定脱敏码。该审计**不等于 durable invocation recovery**：`novel_autopilot` 仍为 `NonResumable`，未新增 Pause/Resume/Steer、checkpoint、重放或自动重试；
6. **G2 与 R8 已完成**：G2 证明 fixed sample、failure injection 和 readonly UI 仍满足安全 owner 边界；R8 已以脱敏 golden sample、运行指标、人工反馈和失败样本完成最小评测/档案闭环，未扩展 Autopilot Tool 或执行流程。

#### 硬性冻结与 No-Go

- 不执行生产数据库 migration、production downgrade 或超出既有授权边界的其他 Schema 变更；
- 未通过 R0.1-R0.3 和 G0，不进入 R3，也不以 UI/Agent 功能绕过生产可靠性门禁；
- G2 已判定为 GO，但其范围仅限当前受控单次 MVP；仍不开发无人值守整书或多卷 Autopilot，也不得由 R8、UI 或 Agent 绕过独立的后续能力门禁；
- 不新增第二套 task store、项目阶段状态、恢复协议或 Coordinator 业务事实；
- 不承诺任意 Token 位置断点续跑，不新增生产 downgrade CLI，不执行生产 downgrade；
- 性能优化必须由 profile、运行指标或真实失败证据驱动，不能用扩大 broadcast capacity 掩盖一致性问题。

这份决议是后续 Trellis 任务排序和范围审查的默认依据。只有满足 16.8 的路线变更条件并记录新的
兼容性、回滚方式和验收门禁后，才允许调整依赖顺序。


#### R8 完成证据与冻结边界（2026-07-16）

- 静态评测样本仅在 Rust `cfg(test)` 下使用，评测器只读取 generation contract/audit 的安全摘要：
  schema version、生成意图、执行角色和质量摘要是否存在。未知 schema 稳定拒绝，测试证明不输出
  Prompt、target/project/chapter 标识、digest、provider/model、fallback 或 endpoint 细节。
- `GET /projects/{project_id}/creative-archive` 复用既有 owner-scoped project export context，使用显式
  allowlist 导出生成时间、章节反馈关联、contract/audit 的版本与安全枚举、质量分数和门禁结论；
  legacy、非 JSON 或未知 history 一律 fail-closed 为缺失安全摘要，不复制原始历史或审计 read model。
- `GET /projects/{project_id}/runtime-metrics` 返回版本化
  `runtime-metrics/v1` / `derived_readonly` DTO。workflow 来自 canonical workflow service；task 来自
  runtime `TaskRegistry`；quality 来自项目质量趋势 read model；受控调用审计来自 durable invocation audit。
  端点先验证 project owner，任一子读取失败仅投影对应段为 `unavailable`，不泄露存储错误。
- `available`、`empty`、`unavailable` 只表示派生 read model 的读取语义：`empty` 表示当前无可观测记录，
  `unavailable` 表示读取失败，均不代表 workflow/task/audit 失败。task 与 invocation audit 为固定最多
  100 条的 runtime-observed 样本，不是 canonical history、恢复点或控制状态。
- 项目详情仅显示一次性请求的 typed readonly 摘要，明确“不自动刷新”；没有 R8 的 Pause/Resume/Steer、
  retry、replay、checkpoint/recovery 或 Autopilot 启动入口。前端 mock E2E 验证了 empty/unavailable
  展示、敏感哨兵字段不渲染及卡片内无控制按钮。
- focused 质量证据：R8 Rust focused tests **11/11**、`cargo check`、frontend `lint` 与 `build` 均通过；
  `project-workflow-state.spec.ts` **6/6** 通过。
- 路线级最终回归（2026-07-16）：Rust 全量 `cargo test` **1801 passed**；frontend 全量 E2E
  **14 passed / 13 skipped**。health readiness 的 migration revision count 断言从 Rust-owned canonical
  `postgres_revision_catalog().len()` 读取，避免新增 migration 后的硬编码计数漂移。
- 恢复语义 E2E 的 mock 终态时间使用动态近时刻，避免测试 fixture 被 production 的 12 小时 terminal-task
  retention 合法压缩；该修复不改变生产 retention、恢复策略、UI 或 API 合约。R8 **不授权**无人值守
  整书/多卷、多 Tool 或多步骤自治，也不放开 Provider/MCP runtime、真实 Prompt 评测、自动重试、恢复、
  重放或新的事实 owner。

---

## 18. 不建议照搬的设计

### 18.1 不把全部业务逻辑放入 Coordinator Prompt

原因：

- 规则不可静态检查；
- 调试和回归困难；
- Prompt 漂移影响范围大；
- 权限、事务和幂等难以保证。

MuMuNovel 应保持“Rust Service 拥有业务事实”。

### 18.2 不将 PostgreSQL 替换为本地文件

文件适合作为导出、审计和备份格式，不适合作为 MuMuNovel 多用户生产数据源。

### 18.3 不整体迁移到 Go

Rust runtime、路由和 workflow 已经形成大规模资产。语言迁移不能解决当前主要问题，只会
重新引入一次双栈迁移。

### 18.4 不复制 TUI 作为主界面

MuMuNovel 的核心优势是 Web 创作工作台。未来如需自动化，可增加 Headless API/CLI，
但不应替换浏览器界面。

### 18.5 不宣称所有任务都可断点继续

模型流式请求、外部检索和部分第三方调用天然无法从任意 Token 位置恢复。应明确任务的
恢复等级，而不是承诺统一“断点续跑”。

---

## 19. 主要风险与控制措施

| 风险 | 影响 | 控制措施 |
|---|---|---|
| Coordinator 过度集中 | Prompt 成为新的巨型业务层 | 只允许调用受控 Tool |
| 状态机与现有 workflow 重复 | 出现第二套状态事实 | Novel phase 只描述项目级阶段 |
| checkpoint 不幂等 | 重复章节、重复写入 | 幂等键、revision 和事务 |
| 自动驾驶越过人工意图 | 用户失去控制 | 默认人工门禁和 Pause |
| 模型策略配置过度复杂 | 普通用户难以使用 | 默认策略 + 高级设置 |
| 多 Provider fallback 隐藏质量变化 | 输出风格漂移 | 记录实际模型并提示切换 |
| 任务恢复泄露敏感信息 | API Key 或 Prompt 泄露 | checkpoint 脱敏和字段白名单 |
| CI 一次性收紧过多 | 历史 warning 阻塞开发 | 建立基线后增量收紧 |
| 与现有质量路线重复建设 | 两套 Story Packet/Quality Gate | 复用既有文档和 Service owner |

---

## 20. 成功标准

MuMuNovel 完成上述路线后，应具备以下能力：

### 工程层

- Rust 生产后端有完整 CI 和真实 E2E；
- 后台任务状态持久化具备崩溃一致性；
- 每类长任务都有明确恢复策略；
- 任务、模型、Prompt、checkpoint 和质量结果可追溯。

### 工作流层

- 小说级阶段状态统一；
- 生成输入统一为 Story Packet / Generation Intent；
- 章节、大纲和重生成使用一致的核心契约；
- 可恢复任务具备版本化、幂等的业务 checkpoint。

### AI 层

- 不同角色可以选择不同模型和 fallback；
- Coordinator 只编排受控业务能力；
- 用户可以暂停、继续和注入方向；
- 自动驾驶与人工工作台可以随时切换。

### 产品层

- 普通用户仍可按现有页面逐步创作；
- 高级用户可以启用自动驾驶；
- 页面关闭后长任务继续执行；
- 服务重启后行为可预测；
- 用户始终拥有最终确认和编辑权。

---

## 21. 最终建议

MuMuNovel 当前最需要的不是继续增加孤立的 AI 功能入口，而是完成以下能力收口：

```text
真实 Rust CI
  -> 可靠后台任务
  -> 明确恢复语义
  -> 小说级状态机
  -> 统一生成契约
  -> 角色模型策略
  -> 可选 Coordinator
  -> Eval 和反馈闭环
```

实施时应坚持以下顺序：

1. 先修工程可靠性，再做自治能力；
2. 先统一状态和契约，再引入 Coordinator；
3. 先复用现有 Service，再新增 Tool；
4. 先做小型 Autopilot 闭环，再扩展整书无人值守；
5. 始终保留人工门禁、兼容入口和回滚路径。

最终形态不应是“让 AI 取代作者”，而应是：

> 作者拥有控制权，Rust 工作流拥有业务事实，后台任务拥有执行状态，
> Coordinator 负责在受控边界内提高自动化效率。

---

## 22. 关键代码参考

### MuMuNovel

- `README.md:63`
- `README.md:263`
- `README.md:341`
- `README.md:409`
- `backend-rs/src/main.rs:28`
- `backend-rs/src/main.rs:63`
- `backend-rs/src/api/router.rs:233`
- `backend-rs/src/ai/service.rs:13`
- `backend-rs/src/ai/service.rs:183`
- `backend-rs/src/tasks/types.rs:6`
- `backend-rs/src/tasks/types.rs:40`
- `backend-rs/src/tasks/persistence.rs:19`
- `backend-rs/src/tasks/persistence.rs:43`
- `backend-rs/src/tasks/recovery.rs:8`
- `frontend/src/utils/taskPolling.ts:59`
- `.github/workflows/backend-ci.yml:3`
- `.github/workflows/e2e-smoke.yml:68`
- `.trellis/tasks/06-28-workflow-background-async-unification/prd.md:5`

### ainovel-cli

- `../ainovel-cli/go.mod:1`
- `../ainovel-cli/cmd/ainovel-cli/main.go:27`
- `../ainovel-cli/internal/host/host.go:36`
- `../ainovel-cli/internal/host/host.go:335`
- `../ainovel-cli/internal/agents/build.go:116`
- `../ainovel-cli/internal/agents/build.go:213`
- `../ainovel-cli/internal/agents/build.go:257`
- `../ainovel-cli/internal/agents/build.go:308`
- `../ainovel-cli/internal/domain/transitions.go:37`
- `../ainovel-cli/internal/domain/transitions.go:60`
- `../ainovel-cli/internal/store/io.go:36`
- `../ainovel-cli/internal/store/io.go:63`
- `../ainovel-cli/internal/store/checkpoints.go:16`
- `../ainovel-cli/internal/store/checkpoints.go:174`
- `../ainovel-cli/internal/bootstrap/configfile.go:53`
- `../ainovel-cli/internal/bootstrap/config.go:97`

---

## 23. 2026-07-19 可观察性补强：模型实际输出双通道

在既定 R0.1–R8 路线完成后，MuMuNovel 增加一项不改变业务事实 owner 的可观察性补强：生成进度界面可选展示模型正在生成的具体业务文本，以及 Provider API 明确返回的 reasoning/thinking 内容。

该能力沿用既有 Provider adapter、SSE、后台任务和通用进度 Modal，不建立新的 task/workflow/audit/checkpoint 系统。`reasoning_chunk` 与正文 `chunk` 独立，reasoning 不进入正式业务结果、任务快照或持久化数据。详细输出默认关闭，浏览器每通道仅保留最近 50,000 个字符，后台 transient queue 总量限制为 64 KiB。

这不是隐藏 chain-of-thought 提取能力，也不是新的 R9。完整实现边界与验收方法见：

- `docs/23-model-reasoning-content-stream-panel-acceptance.zh-CN.md`
