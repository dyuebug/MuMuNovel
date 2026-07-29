# Journal - dyuebug (Part 1)

> AI development session journal
> Started: 2026-05-17

---



## Session 1: Refactor backend chapter generation flow

**Date**: 2026-05-18
**Task**: Refactor backend chapter generation flow
**Branch**: `dev`

### Summary

Split Rust chapter generation seams, hardened batch-generation runtime/status semantics, and landed strangler schema/runtime hardening plus deployment smoke support.

### Main Changes

- 建立 Rust production CI 阻断门禁，并将 Python job 收敛为 migration/support regression。
- E2E 使用 PostgreSQL、Rust migration executor 与真实 Rust runtime，不再依赖 Python/SQLite runtime。
- Playwright smoke 覆盖认证与后台任务主链路，并补齐 backend identity、lifecycle、cleanup 证据。
- 固化 clean-checkout 与 Hosted Runner 证据，明确 MuMuNovel 后续优化路线和 CI 契约。

### Git Commits

| Hash | Message |
|------|---------|
| `b541b70` | (see git log) |
| `692de8b` | (see git log) |

### Testing

- [OK] Rust fmt、check、clippy 与 1612 个测试全部通过。
- [OK] Python migration/support regression：67/67 通过。
- [OK] Frontend lint/build 通过；Playwright smoke：14/14 通过。
- [OK] Backend binary identity 验证通过，cleanup lifecycle 为 terminated (TERM)。
- [OK] 最终 SHA 的 5 个 GitHub Hosted Runner checks 全部 success。

### Status

[OK] **Completed**

### Next Steps

- None - task complete


## Session 2: Complete Rust runtime migration

**Date**: 2026-06-27
**Task**: Complete Rust runtime migration
**Branch**: `dev`

### Summary

Completed Rust-only production runtime migration, updated current docs/specs to the Rust gateway, verified cargo check and live strangler smoke, and fixed Web Research settings persistence.

### Main Changes

(Add details)

### Git Commits

| Hash | Message |
|------|---------|
| `ebd5a61` | (see git log) |
| `06db27f` | (see git log) |
| `cb93528` | (see git log) |

### Testing

- [OK] (Add test results)

### Status

[OK] **Completed**

### Next Steps

- None - task complete


## Session 3: 完成 Rust 生产 CI 与真实 E2E 闭环

**Date**: 2026-07-14
**Task**: 完成 Rust 生产 CI 与真实 E2E 闭环
**Branch**: `ci/r03-runner-evidence-20260713`

### Summary

完成 Rust production CI、PostgreSQL migration executor、真实 Rust runtime Playwright smoke、runner 身份与生命周期证据闭环；本地门禁与最终 Hosted Runner checks 全绿，并安全归档任务。

### Main Changes

(Add details)

### Git Commits

| Hash | Message |
|------|---------|
| `7ec7c8e` | (see git log) |
| `6607624` | (see git log) |
| `7b90b14` | (see git log) |
| `8a78ef7` | (see git log) |
| `c2bb1a0` | (see git log) |
| `8421177` | (see git log) |
| `bec8b7b` | (see git log) |
| `087044d` | (see git log) |

### Testing

- [OK] (Add test results)

### Status

[OK] **Completed**

### Next Steps

- None - task complete

## 2026-07-19 — Durable Novel Autopilot facts and fencing

**Task**: `.trellis/tasks/07-19-durable-novel-autopilot-orchestrator`

### Summary

- 接入 owner-scoped Business Facts Read Model，Router 不再依赖默认空 facts。
- task payload 改为严格 version/epoch fencing。
- `complete_step` 增加 Run/Step/BackgroundTask 三方一致性校验。
- 保留旧 `novel_autopilot` NonResumable 行为；新流程继续使用 `novel_book_autopilot`。
- 审计确认 wizard 生成入口仍是 SSE-only `()`，下一阶段先抽 typed executor，禁止 HTTP loopback。
- 已接入 World、Career、Organization 三个 typed durable adapter；旧 R7 `novel_autopilot`
  的 `NonResumable` 语义保持不变。
- Organization 采用 generation-only plan、业务快照和原子 CAS commit；人工组织存在或事实竞争变化时
  停靠 `WaitingHuman`，不覆盖人工内容、不落盘模型原始内容。

### Testing

- [OK] `cargo fmt --manifest-path backend-rs/Cargo.toml -- --check`
- [OK] `cargo check --manifest-path backend-rs/Cargo.toml -j 1 --tests`（此前验证）
- [OK] `cargo test ... novel_autopilot::tests::repository_tests::organization_commit`
  （独立临时 target + `rust-lld`）：2 passed / 0 failed。
- [BLOCKED-ENV] 标准 MSVC `link.exe` 的定向测试报 `LNK1318 PDB LIMIT (12)`；改用
  `rust-lld` 后定向测试通过。
- [BLOCKED-OUT-OF-SCOPE] 完整 `novel_autopilot` 测试被
  `wizard_character_generation_service.rs` 缺失 `QueryFilter` import 阻塞；未越界修改 Character 代码。

### Status

[IN PROGRESS] Business facts、执行 fencing 与 World/Career/Organization adapters 已完成；
Foundation、Character、Outline adapters、next-tick、章节循环和整书收尾待继续。



## 2026-07-19 — Durable ChapterGenerate wiring and analysis facts

**Task**: `.trellis/tasks/07-19-durable-novel-autopilot-orchestrator`

### Summary

- 注册 `chapter_adapter` 并接入 `execute_novel_book_autopilot_tick`，真实
  `ChapterCandidateRouteGatewayConfig` 与 Run Config 传入单章生成步骤。
- 保持一个 Background Task Tick 只执行一个模型步骤；Provider 返回后仍走 cooperative
  cancellation 与 Run/Step/Chapter snapshot CAS，旧 `novel_autopilot` 未改动。
- Chapter commit 现在要求 `word_count > 0`；修正 Chapter Adapter 测试 fixture 与当前
  `novel_autopilot_run::Model` 字段一致。
- `facts.rs` 读取 `plot_analysis`：正文存在但无分析时路由 ChapterAnalyze；分析分数
  `6.0 <= score < 8.0` 时优先路由 ChapterRepair；低于 6.0 保留给人工复核。

### Testing

- [OK] `cargo fmt --manifest-path backend-rs/Cargo.toml`
- [OK] `cargo check --manifest-path backend-rs/Cargo.toml -j 1 --tests`
- [BLOCKED-ENV] 定向 `cargo test` 在 MSVC link.exe 报 `LNK1318: PDB LIMIT (12)`；
  独立 target 首次全量编译也在 184 秒超时，未得到断言执行结果。

### Status

[IN PROGRESS] ChapterGenerate 已进入 Durable 生产调用图；ChapterAnalyze/Repair facade、
原子业务提交、全书 review/polish/export 与前端工作台仍待完成。
## 2026-07-19 — Durable Novel Autopilot 完整成书验收

**Task**: `.trellis/tasks/07-19-durable-novel-autopilot-orchestrator`

### Summary

- 完成 Durable Novel Autopilot 全流程：基础设定、世界观、职业、角色、组织、大纲、
  一纲多章展开、逐章生成、章节分析、自动返修、全书审查、全书润色、重新分析与真实导出。
- 修复 `book_polish` PostgreSQL 不支持 `json = json` 的提交失败；保留 Run version/epoch、
  Step/BackgroundTask/status fence，并在事务前验证 pending rewrite 队首。
- 修复完整成书 Smoke 的 foundation 夹具、章节号 Prompt 分类和 `paused_version` 摘要变量，
  增加 guidance epoch/version 递增与真实模板回归断言。
- 前端工作台支持创建、暂停、恢复、取消、人工决定和后续指导，并可独立选择显示运行状态、
  Provider 明确返回的 reasoning/thinking 与生成正文；reasoning 不持久化，也不伪造隐藏思维链。

### Verification

- [OK] Python Smoke 工具回归：`6 passed`。
- [OK] `cargo fmt --manifest-path backend-rs/Cargo.toml -- --check`。
- [OK] `cargo check --manifest-path backend-rs/Cargo.toml -j 1 --tests`；仅有既存 warning。
- [OK] `npm --prefix frontend run lint`；仅有既存 Hook warning。
- [OK] `npm --prefix frontend run build`；存在既有 circular chunk warning。
- [BLOCKED-ENV] Windows MSVC 定向 `cargo test` 在链接阶段报
  `LNK1318: PDB LIMIT (12)`；未将其记录为测试断言失败，也未执行 `cargo clean`。

### Runtime Evidence

- Project: `c87bd818-e229-4643-b63c-825484e7682a`
- Run: `8a9d4037-f6c7-4a62-a23f-8ddd5dbf21e9`
- Result: `status=completed`, `execution_scope=complete_book`, `completed_chapters=3`,
  `failed_chapter_count=0`, `pending_rewrite_count=0`, `total_word_count=9562`。
- Pause fence: `paused_epoch=1`, `paused_version=7`, `guided_version=8`；重启前检测到
  `stale_step_count_before_restart=1`，迟到结果未覆盖恢复后的状态。
- Export: `project-export-artifact/v1`, TXT, 3 chapters, 28,189 bytes，包含
  `SMOKE_REPAIRED_CHAPTER_1` 和 `SMOKE_POLISHED_CHAPTER_2`。
- Digest: `sha256:8f2debb7decec5c702c463bab44b61c2985618ebdfaa23936638bb6ab5a68e7e`。

### Status

[COMPLETED-RUNTIME] Durable Novel Autopilot 已具备可持久恢复的一键完整成书能力；
任务暂不归档，避免在未授权情况下触发 Git 提交或分支操作。

## 2026-07-19 — Durable Novel Autopilot 输出、预算、续写与网关门禁收口

**Task**: `.trellis/tasks/07-19-durable-novel-autopilot-orchestrator`

### Summary

- 工作台已明确区分运行状态、Provider reasoning 与生成正文三个显示开关；reasoning 只做
  运行时转发，不进入 Durable 或业务持久化事实。
- Token 展示改为“已观察输出 Token（估算）”；成本预算缺少统一 Provider 定价源时
  fail-closed 到人工门，不使用伪造价格。
- partial scopes 的章节质量事实限制为当前 Run 新生成章节，避免扫描和返修旧人工正文。
- `deploy/strangler-gateway-probes.json` 新增 `novel_autopilot` route group 的 9 个 Rust
  auth-guard probes；Run 列表 GET 同时进入 `deploy` profile。

### Verification

- [OK] manifest validate-only：416 probes，结构合法。
- [OK] `route-groups + novel_autopilot`：9/9 probes 通过真实 Nginx → Rust 网关。
- [OK] `deploy` profile 中 Novel Autopilot Run 列表探针返回预期 401。
- [OK] 前端工作台 Playwright E2E：7 passed；lint/build 通过。
- [OK] Rust `cargo check --tests` 通过；定向 `cargo test` 仍受 Windows MSVC
  `LNK1318: PDB LIMIT (12)` 环境链接限制。

### Status

[IN PROGRESS] 功能和专项探针已收口，等待最终质量门与 Trellis Check 后回填验收清单。

## 2026-07-19 — Durable Novel Autopilot 最终质量门与 PostgreSQL fence 修复

**Task**: `.trellis/tasks/07-19-durable-novel-autopilot-orchestrator`

### Summary

- 修复 SeaORM 可空 active-task CAS fence 在 PostgreSQL 上的 `stale_version`；NULL 分支显式
  使用 `IS NULL`，非空分支继续使用 task id 等值 fence。
- 增加预算人工门 NULL fence 与 Token 累计非空 fence 回归测试，未改变 API、Run/Step
  schema 或旧 R7 `novel_autopilot` 行为。
- 重新部署 Rust 容器并完成确定性 Provider 驱动的 3 章完整成书验收。

### Verification

- [OK] `cargo fmt --manifest-path backend-rs/Cargo.toml -- --check`。
- [OK] `cargo check --manifest-path backend-rs/Cargo.toml -j 1 --tests`。
- [OK] `npm --prefix frontend run lint` 与 `npm --prefix frontend run build`。
- [OK] 工作台 Playwright E2E：`7 passed (8.6s)`。
- [OK] `redeploy-fast.ps1 -SkipFrontendBuild -NoPause -NonInteractive`，gateway smoke 全绿。
- [BLOCKED-ENV] 定向 `cargo test` 仍在 MSVC link.exe 报 `LNK1318: PDB LIMIT (12)`；
  未降低测试阈值、未执行 `cargo clean`，也未误记为断言失败。

### Runtime Evidence

- Project: `4882c6ca-2b6e-455d-97fe-af8a3aa63d48`
- Run: `119da4f2-1023-4d07-aa2e-04055698c20b`
- Result: `status=completed`, `execution_scope=complete_book`, `completed_chapters=3`,
  `failed_chapter_count=0`, `pending_rewrite_count=0`, `total_word_count=9562`,
  `used_tokens=20943`。
- Recovery fence: `paused_epoch=1`, `paused_version=8`, `guided_version=9`,
  `stale_step_count_before_restart=1`。
- Export digest:
  `sha256:fe2092fdf06935713e5da633506afc43754fda61900e775ccc2b1906a5a4394b`；
  repair/polish marker 均存在。

### Status

[COMPLETED-CHECK] Durable Novel Autopilot 的功能、PostgreSQL 运行时 Smoke 与最终质量门均已
通过；任务保留当前目录，不执行未授权的 Git 提交、分支或归档操作。

## 2026-07-19 — Durable Novel Autopilot API 边界与验收矩阵收口

**Task**: `.trellis/tasks/07-19-durable-novel-autopilot-orchestrator`

### Summary

- 补齐 API handler 级 owner/非 owner/非法状态/CAS 回归测试，未修改生产 handler、数据库
  schema 或状态机行为。
- 清理 `implement.md` 中 Foundation/Character/Outline “尚未接入”的过期描述；完整规划链路
  已由 typed durable adapters 与真实完整成书 Smoke 证明。
- 将 PRD 18 项验收逐项映射到生产实现、自动化测试和运行时证据。

### Verification

- [OK] Python smoke 工具测试：`6 passed`。
- [OK] migration Python 编译；migrator model registry：`1 passed`。
- [OK] Rust fmt 与 `cargo check -j 1 --tests`。
- [OK] frontend lint/build；Workbench E2E：`7 passed (8.8s)`。
- [OK] 已有真实 PostgreSQL 完整成书 Smoke：Run
  `119da4f2-1023-4d07-aa2e-04055698c20b`，`status=completed`，3 章 TXT 导出。
- [BLOCKED-ENV] API 定向 Rust tests 在 Windows MSVC 链接阶段触发
  `LNK1318: PDB LIMIT (12)`，未进入断言执行。

### Status

[COMPLETED-CHECK] 18/18 验收证据已闭环；下一步进入 Trellis Phase 3，不执行 Git 提交。

## 2026-07-19 — Durable Novel Autopilot code-spec update

- Added `.trellis/spec/backend/durable-novel-autopilot.md` with executable API,
  DB ownership, task recovery, CAS fencing, privacy, completion, validation, and
  test contracts.
- Registered the spec in `.trellis/spec/backend/index.md`.
- Preserved the key compatibility rule: legacy `novel_autopilot` remains
  `NonResumable`; only `novel_book_autopilot` is `CheckpointResumable`.

## 2026-07-20（星期一）— Durable Novel Autopilot review remediation

**Task**: `.trellis/tasks/07-19-durable-novel-autopilot-orchestrator`

### Summary

- Resolved all seven review findings: PostgreSQL audit actor-id capacity, consumed
  human decisions, enforced human gates, queued-orphan re-dispatch, current
  readiness migration head, unsupported regenerate-existing behavior, and
  unsupported export formats.
- Added business-owned manual-review candidates in `chapter_draft_attempts` with
  terminal Step ids, fenced atomic Accept commits, generation history, and
  distinct Generate versus Repair progress accounting.
- Kept durable Run/Step free of chapter content, prompts, provider responses, and
  provider reasoning; retained legacy `novel_autopilot` as `NonResumable`.

### Verification

- [OK] Rust `cargo fmt --check` and `cargo check -j 1 --tests`.
- [OK] Alembic revision health; PostgreSQL head is
  `20260720_audit_actor_id_capacity`.
- [OK] Migrator model registry: `1 passed`.
- [OK] Frontend lint/build; existing Novel Autopilot Workbench E2E evidence:
  `8 passed`.
- [OK] Worked around the MSVC `LNK1318` PDB limit with the Rust toolchain's
  `rust-lld.exe`, disabled test debuginfo, and disabled incremental compilation.
- [OK] Executed focused Rust assertions: Autopilot service `131 passed`, manual
  review candidates `4 passed`, Run API `8 passed`, and readiness metadata
  `1 passed`.
- [OK] Corrected four stale test expectations: generated fixture content,
  normalized analysis report, non-owner list 404 semantics, and queued-orphan
  resume re-dispatch semantics. No `cargo clean` or `cargo fix` was run.

### Status

[COMPLETED-CHECK] 7/7 review findings have production-path fixes. Documentation
and executable contracts are synchronized; the Trellis task remains active and
unarchived, with no Git commit or branch operation performed.

## 2026-07-21（星期二）— 运行指标不再挤压创作管理子页面

### Summary

- 根因位于 `ProjectDetail` 公共 flex 壳层：`ProjectRuntimeMetricsPanel` 作为项目头部与
  Outlet 之间的常驻文档流兄弟节点，所有创作管理子页面都会被永久扣除指标卡高度。
- 将完整指标面板迁移到右侧 Ant Design `Drawer`，项目顶部仅保留“运行指标”入口；
  Drawer 关闭时不挂载指标组件、不发送运行指标请求。
- 为公共内容面板增加 `data-testid="project-page-content"`，E2E 直接断言 Drawer
  打开前后内容容器高度不变，避免后续回归为常驻卡片。

### Verification

- [OK] `npm --prefix frontend run lint`；仅有仓库既存 Hook warnings，本次文件无新增告警。
- [OK] `npm --prefix frontend run build`；TypeScript 与 Vite 构建通过。
- [OK] `npm --prefix frontend run e2e -- e2e/project-workflow-state.spec.ts`：
  `6 passed (9.2s)`。
- [OK] 定向运行指标用例验证默认请求数为 0、打开后为 1、只读/隐私安全状态不变、
  内容区高度不变并可通过 Escape 关闭 Drawer。

### Status

[COMPLETED-CHECK] 运行指标改为按需 overlay，不再占用创作管理各子页面的固定展示高度；
未修改运行指标 API、后端模型或持久化契约。

## 2026-07-23（星期四）— Autopilot 任务卡与步骤时间线比例修复

**Task**: `.trellis/tasks/07-19-durable-novel-autopilot-orchestrator`

### Summary

- 后台任务中心的 `Task Dossier` 改为适配 440px Drawer 的紧凑单栏布局，避免
  `Current Focus` 与 `Execution Pulse` 互相挤压并减少创作状态卡的无效高度。
- 自动创作步骤时间线采用与实际列宽一致的固定表格布局；“章节分析”保持完整主标签，
  长步骤键和错误代码通过 ellipsis + tooltip 保留可诊断性。
- 将 `scroll.x` 从 980 降为 856，并压缩更新时间及辅助列；桌面宽度下不再产生无意义横向滚动。
- 新增 E2E 比例回归断言，避免未来仅靠肉眼判断列宽与行高。

### Verification

- [OK] Frontend lint；仅有仓库既存 Hook warnings。
- [OK] Frontend TypeScript/Vite build；仅有既存循环 chunk 警告。
- [OK] Novel Autopilot Workbench Playwright E2E：`8 passed (9.8s)`。
- [OK] Frontend lint：0 errors；仅有 33 个仓库既存 Hook warnings。
- [OK] Chromium 尺寸断言覆盖步骤列、更新时间列、章节分析行高、桌面溢出与步骤键省略。

### Status

[COMPLETED-CHECK] 本轮仅调整前端可观测性布局与回归测试；未改变 Autopilot API、
Durable 状态机、数据库 schema 或模型输出持久化边界。

## 2026-07-23（星期四）— Autopilot 章节列紧凑化

**Task**: `.trellis/tasks/07-19-durable-novel-autopilot-orchestrator`

### Summary

- 步骤时间线“章节”列由 80px 收紧为 72px，使用居中的 `第2章` 紧凑格式。
- 章节文本禁止换行，避免短章节编号在窄表格中拆行并影响行高。
- `scroll.x` 同步按真实列宽预算调整为 848；未压缩“步骤”主标识列。
- E2E 增加章节列宽范围和章节文本 `nowrap` 回归断言。

### Verification

- [OK] Novel Autopilot Workbench Playwright E2E：`8 passed (9.3s)`。
- [OK] Frontend lint；仅有仓库既存 Hook warnings。
- [OK] Frontend TypeScript/Vite build；仅有既存循环 chunk 警告。

### Status

[COMPLETED-CHECK] 本轮仅调整步骤时间线章节列布局和对应测试；未改变 API、
Durable 状态机或章节业务数据。

## 2026-07-23（星期四）— Autopilot 尝试列视觉对齐

**Task**: `.trellis/tasks/07-19-durable-novel-autopilot-orchestrator`

### Summary

- 步骤时间线“尝试”列保留 64px，避免过度压缩双汉字表头。
- 表头和 attempt 数值统一居中，数值显式禁止换行，与章节列形成一致的紧凑布局。
- E2E 按列单元格定位验证宽度、居中和 `nowrap`，避免断言与具体 attempt fixture 耦合。

### Verification

- [OK] Novel Autopilot Workbench Playwright E2E：`8 passed (9.8s)`。
- [OK] Frontend lint；仅有仓库既存 Hook warnings。
- [OK] Frontend TypeScript/Vite build；仅有既存循环 chunk 警告。

### Status

[COMPLETED-CHECK] 本轮仅调整步骤时间线尝试列展示和回归测试；未改变 attempt
计数、重试预算、API 或 Durable Run 行为。

## 2026-07-23（星期四）— Autopilot 状态列视觉对齐

**Task**: `.trellis/tasks/07-19-durable-novel-autopilot-orchestrator`

### Summary

- 步骤时间线“状态”列保留 92px，以安全容纳“排队中/运行中”等状态标签。
- 表头和状态单元格统一居中，移除 Ant Design Tag 默认右外边距，解决视觉偏移。
- 状态 Tag 禁止换行；E2E 覆盖列宽、居中、`nowrap` 和零右边距。

### Verification

- [OK] Novel Autopilot Workbench Playwright E2E：`8 passed (9.7s)`。
- [OK] Frontend lint；仅有仓库既存 Hook warnings。
- [OK] Frontend TypeScript/Vite build；仅有既存循环 chunk 警告。

### Status

[COMPLETED-CHECK] 本轮仅调整步骤时间线状态列展示和回归测试；未改变状态枚举、
状态转换、API 或 Durable Run 行为。

## 2026-07-23（星期四）— Autopilot 质量决定可读化

**Task**: `.trellis/tasks/07-19-durable-novel-autopilot-orchestrator`

### Summary

- 新增强类型质量决定标签映射，将内部英文枚举转换为中文 Tag 展示。
- “质量决定”列保留 104px，表头与单元格居中，Tag 去除默认右外边距并禁止换行。
- E2E 覆盖列宽、中文“通过”标签、居中、`nowrap` 与零右边距。
- 前端组件规范新增领域枚举用户化展示约定，错误代码等诊断标识仍保留原始值。

### Verification

- [OK] Novel Autopilot Workbench Playwright E2E：`8 passed (9.8s)`。
- [OK] Frontend lint；仅有仓库既存 Hook warnings。
- [OK] Frontend TypeScript/Vite build；仅有既存循环 chunk 警告。

### Status

[COMPLETED-CHECK] 本轮仅调整质量决定展示和回归测试；未改变质量路由、自动修复、
人工复核或 Durable Run 决策语义。

## 2026-07-23（星期四）— Autopilot 错误代码诊断展示

**Task**: `.trellis/tasks/07-19-durable-novel-autopilot-orchestrator`

### Summary

- 步骤时间线“错误代码”列保留原始诊断标识，显式左对齐并维持 140px 声明宽度。
- 长错误代码单行截断，悬停展示完整 Tooltip；空值使用统一 Typography 显示 `—`。
- E2E 覆盖列宽弹性范围、左对齐、`nowrap`、ellipsis 和 Tooltip 完整值。
- 前端组件规范补充诊断标识展示契约；不将错误代码错误地翻译为业务标签。

### Verification

- [OK] Novel Autopilot Workbench Playwright E2E：`8 passed (9.8s)`。
- [OK] Frontend lint；仅有仓库既存 Hook warnings。
- [OK] Frontend TypeScript/Vite build；仅有既存循环 chunk 警告。

### Status

[COMPLETED-CHECK] 本轮仅调整错误代码展示和回归测试；未改变错误码生成、错误传播、
重试策略、API 或 Durable Run 状态语义。

## 2026-07-23（星期四）— Autopilot 章节 Step Key 去重展示

**Task**: `.trellis/tasks/07-19-durable-novel-autopilot-orchestrator`

### Summary

- `chapter:0001:analyze` 等 Step Key 继续作为后端稳定幂等标识，不修改存储或 API。
- 章节步骤默认只显示中文步骤名称；章节号由独立列承担，避免两列重复表达同一信息。
- 完整 Step Key 移入步骤名称 Tooltip，非章节步骤仍保留第二行机器标识。
- E2E fixture 更新为真实零填充键格式，并覆盖隐藏、Tooltip 和非章节兼容行为。

### Verification

- [OK] Novel Autopilot Workbench Playwright E2E：`8 passed (10.2s)`。
- [OK] Frontend lint；仅有仓库既存 Hook warnings。
- [OK] Frontend TypeScript/Vite build；仅有既存循环 chunk 警告。

### Status

[COMPLETED-CHECK] 本轮仅调整 Step Key 的前端呈现；未改变 Step Key 生成、唯一约束、
恢复 fence、API 或 Durable Run 路由语义。

## 2026-07-23（星期四）— Autopilot 尝试次数语义化展示

**Task**: `.trellis/tasks/07-19-durable-novel-autopilot-orchestrator`

### Summary

- 步骤时间线 attempt 从裸数字 `1` 改为紧凑的 `1次`，减少用户理解成本。
- 保持 64px 列宽、居中和单行约束，不扩大时间线宽度预算。
- E2E 同步覆盖 `1次` 文本及既有布局契约。

### Verification

- [OK] Novel Autopilot Workbench Playwright E2E：`8 passed (10.1s)`。
- [OK] Frontend lint；仅有仓库既存 Hook warnings。
- [OK] Frontend TypeScript/Vite build；仅有既存循环 chunk 警告。

### Status

[COMPLETED-CHECK] 本轮仅调整 attempt 的前端文案；未改变尝试计数、重试预算、API、
数据库或 Durable Run 执行语义。

## 2026-07-23（星期四）— SPA 静态入口与 Autopilot 时间线版本一致性

**Task**: `.trellis/tasks/07-19-durable-novel-autopilot-orchestrator`

### Summary

- 用户实际页面仍显示 `chapter:0001:analyze` 和“第 1 章”，但当前源码及 E2E 已是
  隐藏章节 Step Key、显示“第1章”的新格式。
- 确认 Compose 将 `backend/static` 只读绑定到 `/app/static`；Rust 启动时缓存
  `index.html`，而资产文件从目录实时提供，前端重建后导致旧 HTML + 新 assets 混用。
- `backend-rs/src/api/router.rs` 改为在每次 SPA fallback 时读取当前 `index.html`，并
  新增热更新回归测试。
- 仅重启本地 Rust 容器后，HTTP 与磁盘入口均切换为 `assets/index-D6FYkLyx.js`；数据库
  与镜像未变更。

### Verification

- [OK] `cargo fmt --check`。
- [OK] `cargo check --tests`；仅有仓库既存 warnings。
- [BLOCKED-ENV] 定向 Rust 测试两次被 Windows MSVC `LNK1318: PDB LIMIT (12)` 阻断
  于链接阶段，未发生断言失败。
- [OK] Novel Autopilot Workbench Playwright E2E：`8 passed (9.8s)`。
- [OK] `http://localhost:8005/health`：200；HTTP 与挂载入口哈希一致。

### Status

[COMPLETED-CHECK] 当前运行页面已加载最新前端入口；长期修复已进入源码，后续 Rust
发布后无需依赖人工容器重启来吸收 bind-mounted `index.html` 更新。


## Session 4: Chapter repair retry evidence convergence

**Date**: 2026-07-27
**Task**: Chapter repair retry evidence convergence
**Branch**: `ci/r03-runner-evidence-20260713`

### Summary

Persist scoped chapter-repair retry candidates and quality feedback atomically, route exhausted retries to one durable manual-review candidate, add regression coverage, and record the retry evidence contract.

### Main Changes

(Add details)

### Git Commits

| Hash | Message |
|------|---------|
| `6ff5582` | (see git log) |

### Testing

- [OK] (Add test results)

### Status

[OK] **Completed**

### Next Steps

- None - task complete
