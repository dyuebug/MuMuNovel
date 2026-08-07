# Bug Analysis: 自动创作质量、失败路由与时间契约反复漂移

## 1. Root Cause Category

- **Category**: B / E / D - 跨层契约、隐式假设、测试覆盖缺口。
- **Specific Cause**: `waiting_human` 同时表示有候选质量耗尽和无候选运行故障，
  但 UI 曾把状态隐式等同于“可接受候选”，且隐藏 Accept 按钮没有撤销后端接受
  能力；分析/返修边界又把配置、上下文、响应无效和 Provider 故障压成同一个
  字符串错误。数据库时间是 UTC 语义的 `NaiveDateTime`，API 未携带时区，浏览器
  只能按本地时间猜测。

## 2. Why Fixes Failed

1. **Surface Fix**: 只调整聚合错误文案，无法区分是否存在候选，也无法阻止错误 Accept。
2. **Incomplete Scope**: 只在 Adapter 增加重试判断，没有保留 typed HTTP 状态，
   导致端口、请求 ID 或模型构建号中的数字可能被当成 HTTP 状态。
3. **Change Propagation Failure**: 完整质量对象进入 Task/SSE，候选 digest 只在单一
   位置校验，跨存储边界后仍可能出现状态、正文和摘要不一致。
4. **Test Coverage Gap**: 单元测试未同时覆盖候选存在/不存在、Generate Repair、
   北京时间显示和 Task 安全投影，跨层回归只能在真实运行后暴露。
5. **UI-only Mitigation**: 前端根据 `candidate_id` 隐藏 Accept 只能改善界面，构造 API
   请求仍可能绕过 UI；操作权限必须由 API 和协调器基于持久化证据共同强制执行。

## 3. Prevention Mechanisms

| Priority | Mechanism | Specific Action | Status |
|---|---|---|---|
| P0 | Architecture | 使用 typed runtime error 和 `FailureCounterKind::{Provider, Quality, None}` | DONE |
| P0 | Runtime invariant | 候选持久化校验正文摘要，Accept 校验正文/私有 payload/Step 三方 digest | DONE |
| P0 | Capability enforcement | API 预检候选/错误证据，协调器最终检查；无候选 Accept 返回 409 且不产生副作用 | DONE |
| P1 | Data minimization | Task/SSE 使用 `quality_diagnostics` allowlist，不复制完整质量上下文 | DONE |
| P1 | Boundary contract | API 统一输出 RFC 3339 UTC `Z`，前端只做标准本地时区显示 | DONE |
| P1 | Test coverage | Rust 聚焦测试和 Workbench E2E 覆盖有/无候选、Repair、时间与脱敏 | DONE |

## 4. Systematic Expansion

- **Similar Issues**: 其他 Autopilot Adapter 仍可能用聚合字符串分类 Provider 故障，
  后续修改应复用安全诊断与显式计数模式，而不是复制关键词判断。
- **Design Improvement**: 将“运行状态”和“允许操作”分离；状态描述流程位置，
  `candidate_id`、版本、epoch、digest 和 owner row 决定能力；UI 仅呈现能力，API 与
  服务端必须独立校验并 fail-closed。
- **Process Improvement**: 跨存储、服务、Task/SSE、API、UI 的修复必须同时包含
  数据流审查与 E2E，不能用 `cargo check --tests` 代替实际测试执行。

## 5. Knowledge Capture

- [x] 更新 `.trellis/spec/backend/durable-novel-autopilot.md` 的可执行契约。
- [x] 更新 `.trellis/spec/guides/cross-layer-thinking-guide.md` 的状态/能力检查项。
- [x] 当前任务 PRD、设计和实施记录同步最终人工复核决策。
- [x] 检查模板同步目录；本仓库不存在 `src/templates/markdown/spec`，无需同步。
- [ ] Git 提交：未执行；项目规则要求用户再次明确确认后才能 commit。

## Follow-up: retry/draft 候选导致生产 Run 卡死

### 1. Root Cause Category

- **Category**: B / D / E - 跨层契约、组合测试缺口、隐式状态语义。
- **Specific Cause**: 章节生成 lifecycle 对质量重试正确输出
  `attempt_state=retry`、`chapter_status=draft`，但 Autopilot Adapter 把该值当作
  人工接受后的章节目标状态传给 Repository；Repository 只接受 `completed`，因而
  返回 `invalid_config(candidate_chapter_status)`。旧 Repository fixture 直接构造
  `completed`，没有经过真实 runtime lifecycle，导致分层测试无法暴露冲突。

### 2. Why The Previous Fix Failed

1. 最终质量耗尽已经改为调用人工候选事务，但没有组合真实 runtime 输出与事务输入。
2. `chapter_status` 字段名没有表达“当前候选状态”还是“接受后章节目标状态”。
3. Adapter/Repository 错误通过 `.await?` 传播后只失败内存 Task，数据库 Run/Step
   保持 `running`，使页面更新时间停在最后一次成功持久化。

### 3. Prevention Mechanisms

| Priority | Mechanism | Specific Action | Status |
|---|---|---|---|
| P0 | Compile-time boundary | 从 `NovelAutopilotManualReviewCandidate` 删除调用方提供的 `chapter_status`，由 Repository 固定 Accept 目标 `completed` | DONE |
| P0 | Runtime convergence | claimed tick 异常时按 version/epoch/task/current-step 围栏原子终结 Step 并进入无候选 `waiting_human` | DONE |
| P0 | Integration test | 使用数据库测试覆盖候选目标状态和 Run/Step 原子收敛、时间推进 | DONE |
| P1 | UI capability | 继续依据 Step API 的真实 `candidate_id` 显示 Accept；执行失败只显示 Retry/Repair/Stop | DONE |
| P1 | Retry feedback | 为 ChapterGenerate 增加 scoped retry candidate 与质量反馈消费，避免相同输入盲重试 | DONE |
| P1 | Durable backoff | 为 Provider 瞬时故障增加持久化 `next_attempt_at`/退避，而不是当前 tick 立即调度 | DONE |
| P1 | Typed Provider hint | 在 Provider transport boundary 解析并透传 `Retry-After`，禁止从错误字符串猜测 | DONE |
| P1 | Integration coverage | 覆盖并发单次绑定、重启按原 due 重建、due 前不进入执行链和所有清理路径 | DONE |

### 4. Systematic Expansion

- 返修和生成路径现在都具备 scoped retry baseline；生成路径的正文只进入
  `chapter_draft_attempts`，编排状态只保留 digest 与 allowlist 质量摘要。
- `ainovel-cli` 的 rewrite brief 证明“为什么改、改哪里”必须进入下一轮上下文，但
  MuMuNovel 应复用数据库 Run/epoch/章节快照契约，不能照搬本地文件队列。
- Provider/model fallback 必须服从用户 settings 与 role-model policy；当前没有跨
  Provider fallback 列表契约，不能硬编码替换用户 Provider。

## Follow-up: ChapterGenerate 盲重试与 Provider 重试突刺

### 1. Root Cause Category

- **Category**: B / D / E - 跨层契约、组合测试缺口、隐式恢复假设。
- **Specific Cause**: ChapterGenerate 第 N 次质量失败只在 Step/Task 保存 digest 和安全
  摘要，没有保存下一次可消费的完整候选；同时 retryable Provider 失败只表达
  `retry_scheduled`，没有数据库级 due time。结果分别是无上下文重新生成，以及进程内
  连续调度或重启恢复立即再次调用 Provider。

### 2. Why The Previous Fix Failed

1. 只保存 digest 能证明候选不同，却不能构造“候选 -> 反馈 -> 定向改写”的输入闭环。
2. 只在内存 Task 中延迟无法跨重启，也无法成为多实例共同遵守的时间围栏。
3. 单元测试分别验证生成、Repository 和调度 helper，没有覆盖跨层 evidence 消费与
   startup reconciliation 的完整恢复链。

### 3. Prevention Mechanisms

| Priority | Mechanism | Specific Action | Status |
|---|---|---|---|
| P0 | Durable evidence | 完整候选写入独立 source/state，并用 Run/epoch/Step/chapter snapshot/digest 校验 | DONE |
| P0 | Transaction invariant | evidence、Step 终态、Run 计数/version 与 task fence 在同一事务提交 | DONE |
| P0 | Database time fence | `next_attempt_at` 由 failure transaction 写入，Task pending wait 与 claim CAS 共同执行 | DONE |
| P1 | Provider boundary | typed 解析 `Retry-After` 并透传有界 hint | DONE |
| P1 | Cross-layer tests | 补齐重启、双调度器、due 前不进入执行链和清理矩阵 | DONE |

### 4. Systematic Expansion

- **Similar Issues**: 其他 Provider-backed Step 若仍只返回聚合错误码，应先确认是否需要
  同一 durable retry contract，不能复制字符串重试逻辑。
- **Design Improvement**: retry evidence 与人工候选共享存储能力但使用不同 source/state，
  避免中间候选意外获得 Accept 能力；数据库 due 是恢复唯一事实，Task 只执行等待。
- **Process Improvement**: 新增自动重试必须同时审查“上一轮数据是否可消费”和“重启后
  何时允许再次执行”，并将 Storage -> Repository -> Task -> API/UI 纳入同一检查。

### 5. Knowledge Capture

- [x] 更新 Durable Novel Autopilot spec 的 retry evidence 与 persistent backoff 契约。
- [x] 更新 cross-layer thinking guide 的正文隔离、时间围栏和恢复检查项。
- [x] typed `Retry-After` 与完整并发/恢复集成覆盖已经完成并通过聚焦测试。

## Follow-up: 线上旧镜像与 migration replay 测试漂移

- 2026-08-06 的 `invalid_config` 对应 Run `c6af04d5-2fe2-49d4-b6f5-3886a1b75cd1`：
  第 1 章前两次为 `chapter_quality_retry`，第三次 Step 保持 `running`，Provider/质量失败
  计数均为 0，且没有写入人工候选；证据与候选 lifecycle `draft` 被误当成接受后章节目标
  状态的跨层契约错误一致。
- 运行容器仍是 2026-08-06 构建，线上 schema 没有 `next_attempt_at`；工作区修复尚未部署，
  因此页面继续表现为旧逻辑不是当前源码回归证据。
- 新增 retry-backoff revision 后，三条 Rust replay 测试仍保留旧 revision 列表和 SQL
  step count。质量检查捕获后已补入 `20260720_audit_actor_id_capacity`，并将总 step count
  从 131 更新为 133；`schema_migration_metadata` 40 项测试全部通过。
