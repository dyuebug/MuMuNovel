# Implementation Plan: Durable Novel Autopilot Orchestrator

## Progress log — 2026-07-19 (current session)

- [x] 启动任务并加载 Trellis backend/shared specs、PRD、design 与 implementation plan。
- [x] 已落地 Run/Step 持久化类型、owner-scoped repository、version/epoch CAS、纯 Router、
  Run 控制 API、`novel_book_autopilot` task registration；旧 R7 `novel_autopilot` 保持
  `NonResumable` 且未修改其 executor。
- [x] 将 coordinator 调整为单 tick：Run/Step 作为唯一 durable checkpoint，使用
  `prepare_and_claim_step` 原子建档和 claim，不在 Background Task Future 中循环。
- [x] 修复暂停/取消的活动 Step cursor：同一事务把 running Step 终止为
  `stale/cancelled`，清空 Run cursor，resume 后可创建新 attempt；补充 SQLite 回归测试。
- [x] `cargo fmt --check` 与 `cargo check --tests` 已通过（存在项目既有 warning）。
- [x] 初始定向 `cargo test` 曾被 Windows/MSVC `LNK1318`（PDB LIMIT 12）阻塞；
  最终使用 `rust-lld` 实际执行断言，结果见本文件末尾审查回归记录。
- [x] 已增加 DB 驱动 startup reconciliation：启动时扫描 `queued/running` Run，
  将旧 running Step 标记为 `stale/service_restarted`，递增 epoch/version 后重新投递 tick。
- [x] 已接入真实业务事实 read model：从 project/career/character/organization/outline/chapter
  owner 表派生 planning readiness 与下一未完成章节；Run 不复制正文或大纲事实。
- [x] 已强化执行 fencing：task payload 必须与当前 Run version/epoch 严格相等；
  `complete_step` 同时校验 Run current_step、active task、Step key 与 Step task，防止迟到任务
  清理其他 Step 的 cursor。
- [x] 已将 Durable planning 调用接入 typed service adapters；模型输出由运行时
  `NovelAutopilotOutputObserver` 转发到 Background Task SSE，业务提交仍在 cancellation、
  version/epoch 与事实快照 fence 之后执行。
- [x] 已接入 `WorldDesign`、`CareerDesign`、`OrganizationDesign` 三个 typed durable adapter。
  `OrganizationDesign` 采用 generation-only service、业务快照与原子 CAS commit：已有人工组织
  内容或 generation 期间业务事实变化时均安全停靠到 `WaitingHuman`，不覆盖人工数据，也不持久化
  prompt、reasoning 或原始模型响应。
- [x] Organization SQLite 原子提交回归：`organization_commit_*` 2/2 通过（独立临时 target，
  使用 `rust-lld`）。标准 MSVC `link.exe` 的 `LNK1318 PDB LIMIT (12)` 属环境链接问题；随后
  全量 `novel_autopilot` 复验被范围外 `wizard_character_generation_service.rs` 缺失 `QueryFilter`
  import 阻塞，未修改 Character 范围代码。
- [x] Foundation、Character、Outline、budget guard、planning/chapter/completion adapters、
  正常路径 next-tick 自动调度、frontend workbench 与真实 HTTP smoke 均已完成。


## Phase 0 - Planning and contracts

- [x] 校验 `prd.md` / `design.md` / `implement.md`，启动 Trellis 任务。
- [x] 加载 backend/frontend Trellis 规范与相关 CLAUDE/AGENTS 规则。
- [x] 冻结旧 `novel_autopilot` NonResumable 契约并增加兼容回归测试。

## Phase 1 - Durable persistence and domain types

- [x] 新增 PostgreSQL Alembic migration：`novel_autopilot_runs` 与 `novel_autopilot_step_runs`。
- [x] 更新 migrator frozen models/registry 与 Rust migration metadata catalog/head/tests。
- [x] 新增 SeaORM Run/Step entities，并注册到 `models/mod.rs`。
- [x] 实现 status/phase/scope/human-gate/config/read-model 类型和严格 serde 校验。
- [x] 实现 repository：owner-scoped read、active-run uniqueness、CAS version/epoch、step claim/terminal、safe list projections。
- [x] 添加 SQLite repository/state-machine tests。

验证：

```powershell
python -X utf8 -m py_compile backend/alembic/postgres/versions/<new_revision>.py
cargo fmt --manifest-path backend-rs/Cargo.toml -- --check
cargo test --manifest-path backend-rs/Cargo.toml -j 1 novel_autopilot_run
cargo test --manifest-path backend-rs/Cargo.toml -j 1 schema_migration_metadata_service
```

回滚点：新增表尚未接入执行器，可安全 downgrade；不得修改现有业务表内容。

## Phase 2 - API, coordinator, background task, recovery

- [x] 新增 `novel_book_autopilot` task type，保持旧 `novel_autopilot` executor 不变。
- [x] 实现纯 Router、budget guard、one-step coordinator 和 stable step keys。
- [x] 实现创建/列表/详情/steps/pause/resume/cancel/decision API。
- [x] 接入 Project owner 404 边界、Cooperative Cancellation、Background Task SSE。
- [x] 在 recovery policy registry 注册 durable recovery；增加 startup reconciliation。
- [x] 添加同项目并发创建、状态转换、pause/cancel late result、restart recovery 测试。

验证：

```powershell
cargo test --manifest-path backend-rs/Cargo.toml -j 1 novel_book_autopilot
cargo test --manifest-path backend-rs/Cargo.toml -j 1 background_task_recovery
cargo check --manifest-path backend-rs/Cargo.toml -j 1
```

回滚点：禁用新 route/task registration；旧手工生成与 R7 不受影响。

## Phase 3 - Planning adapters

- [x] 审计 wizard/world/career/character/organization/outline 入口；确认需先把 SSE-only `()` 函数抽为 typed service executor。
- [x] 实现基础资料存在性/有效性 facts read model 和“已有人工内容默认跳过”路由依据。
- [x] 接入 `WorldDesign`、`CareerDesign`、`OrganizationDesign` 的 typed durable adapter；
  Organization 只生成 allowlist plan，并以 snapshot/CAS 原子写入组织、成员和关系。
- [x] Organization 默认不覆盖人工组织：已有组织或 legacy 组织角色时进入 `WaitingHuman`；
  generation 期间任何相关业务事实变化均拒绝提交并进入人工门。
- [x] Foundation、Character、Outline 已通过 typed durable adapter 接入；完整规划链路
  `foundation -> world -> careers -> characters -> organizations -> outline -> outline_expand` 已完成。
- [x] 每个 planning step 接入 epoch/budget/checkpoint/safe digest。
- [x] 添加空项目完整规划、已有资料跳过、禁止默认覆盖人工内容、planning_only 测试。

验证：

```powershell
cargo test --manifest-path backend-rs/Cargo.toml -j 1 novel_autopilot_planning
cargo test --manifest-path backend-rs/Cargo.toml -j 1 wizard
```

## Phase 4 - Chapter loop and quality closure

- [x] 建立下一未完成章节解析和大纲/章节一致性校验。
- [x] 复用 Generation Contract / Story Packet / Role Model Policy 启动单章生成。
- [x] 接入章节分析与质量门 decision adapter。
- [x] 实现 accept/auto_repair/retry/manual_review、attempt 限制、Pending Rewrite。
- [x] 更新 Run chapter cursor、字数、Token/成本、连续失败和质量趋势。
- [x] 实现 next_n_chapters、continue_from_current、complete_book chapter loop。
- [x] 添加三章真实 service 测试、重试耗尽、暂停/恢复、迟到结果测试。

2026-07-19 实施进度：

- ChapterGenerate generation-only facade 已接入 Durable Coordinator；真实 Gateway Config 沿
  Router / Background Task / Tick 透传，保持一个 Tick 一个 Step。
- accept 候选通过 Run/Step/BackgroundTask/Chapter Snapshot CAS 提交；零字正文被拒绝，
  retry/auto_repair/manual_review 不写 Chapter 正文。
- Business Facts 已从 `plot_analysis` 推导首个 pending analysis 与 6.0–8.0 分段的
  pending repair；ChapterAnalyze/Repair 的 generation-only facade 与事务提交仍未完成。
- `cargo check --manifest-path backend-rs/Cargo.toml -j 1 --tests` 通过；该阶段 Windows MSVC
  定向 `cargo test` 曾被 `LNK1318: PDB LIMIT (12)` 阻塞，最终已用 `rust-lld` 完成断言回归。

验证：

```powershell
cargo test --manifest-path backend-rs/Cargo.toml -j 1 novel_autopilot_chapter
cargo test --manifest-path backend-rs/Cargo.toml -j 1 chapter_single_generation
cargo test --manifest-path backend-rs/Cargo.toml -j 1 chapter_batch_generation
```

回滚点：暂停新 Run；已接受章节仍是合法业务数据，不做破坏性回滚。

## Phase 5 - Book completion

- [x] 实现全书缺章/顺序/大纲一致性 gate。
- [x] 聚合章节分析为全书 review summary，标记需返修章节。
- [x] 复用 polish owner 对受限章节执行润色并再次质量校验。
- [x] CAS 推进 Workflow reviewing -> polishing -> completed。
- [x] 复用项目导出，写 `final_export_ref`，不复制导出正文到 Run。
- [x] 添加缺章拒绝完成、review/repair/polish/export、完整完结测试。

验证：

```powershell
cargo test --manifest-path backend-rs/Cargo.toml -j 1 novel_autopilot_completion
cargo test --manifest-path backend-rs/Cargo.toml -j 1 polish
cargo test --manifest-path backend-rs/Cargo.toml -j 1 project_export
```

## Phase 6 - Frontend workbench

- [x] 新增 API types/service module 和 exhaustive status reducer。
- [x] 新增自动创作路由、ProjectDetail 菜单和工作台布局。
- [x] 实现创建配置表单、阶段/章节/预算/质量展示和 step timeline。
- [x] 实现暂停、恢复、取消、人工决定、后续指导交互。
- [x] 复用 Background Task SSE 与模型内容/reasoning 可选显示。
- [x] 保持 Runtime Metrics sticky 行为并验证各创作页面滚动。
- [x] 添加 reducer/component/E2E：创建、刷新恢复、控制、错误、空状态、输出开关。

验证：

```powershell
npm --prefix frontend run lint
npm --prefix frontend run build
npm --prefix frontend run test -- --run
```

## Phase 7 - Integration, smoke, docs

- [x] 增加真实 HTTP smoke：创建 Run -> 三章 -> pause -> restart -> resume -> complete/export。
- [x] 更新 deploy strangler gateway probes 和 smoke manifest（若新 route 属于部署门禁）。
- [x] 更新 `docs/` 优化路线/完成审计，明确“旧 R7 单 Tool”与“R9 Durable 一键成书”边界。
- [x] 运行格式、类型、focused Python tests、lint/build、migration metadata、工作台 E2E 和真实 HTTP 完整成书 smoke。
- [x] 执行 `trellis-check` 并记录风险和回滚方式；标准 Windows MSVC 链接器曾触发
  `LNK1318: PDB LIMIT (12)`，最终改用 `rust-lld` 执行定向 Rust 断言；`cargo check --tests` 通过。

最终验收命令：

```powershell
cargo fmt --manifest-path backend-rs/Cargo.toml -- --check
cargo check --manifest-path backend-rs/Cargo.toml -j 1
npm --prefix frontend run lint
npm --prefix frontend run build
python -X utf8 backend/tools/run_strangler_gateway_smoke.py --help
```

## Review gates

- [x] 不修改旧 `novel_autopilot` NonResumable 行为。
- [x] Run/Step 不成为正文、大纲、质量或后台任务第二事实 owner。
- [x] 所有 business commit 前重新校验 run epoch/status/cancellation。
- [x] 所有公开 DTO 均为 allowlist，不泄露 Prompt/reasoning/credentials/raw error。
- [x] 所有新状态在 Rust match 与 TypeScript reducer 中穷举。
- [x] 所有预算限制在调用前和提交后均检查。
- [x] 所有恢复路径都有幂等与迟到结果测试。

## 2026-07-19 最终完整成书验收回填

- [x] 修复 `book_polish` PostgreSQL `json = json` 不受支持导致的提交失败；保留 version/epoch、
  step/task/status fence，并在事务前验证 pending rewrite 队首。
- [x] 修复 Smoke 项目预填 foundation 字段导致 `foundation` 被合法跳过的问题。
- [x] 修复 Smoke 章节号解析误命中模板说明 `chapter_number为2或3` 的问题；优先匹配真实任务句和
  确定性正文 marker。
- [x] 修复 Smoke 成功摘要中 `paused_version` 未定义，并增加 guidance version fence 递增断言。
- [x] Python 定向测试：`6 passed`；两个修改文件均为 UTF-8 无 BOM。
- [x] `cargo fmt --manifest-path backend-rs/Cargo.toml -- --check` 通过。
- [x] `cargo check --manifest-path backend-rs/Cargo.toml -j 1 --tests` 通过；仅有既存 warning。
- [x] Rust 定向测试最初在 Windows MSVC 链接阶段触发 `LNK1318: PDB LIMIT (12)`；
  最终使用 `rust-lld` 实际执行，不再作为当前环境阻塞。
- [x] 真实 HTTP 完整成书 Smoke 通过：Run
  `8a9d4037-f6c7-4a62-a23f-8ddd5dbf21e9`，项目
  `c87bd818-e229-4643-b63c-825484e7682a`。
- [x] 完成 `foundation → planning → chapter loop → chapter repair → book review → book polish → export`。
- [x] Pause/Guidance/Restart/Resume、stale late result、Workflow 完结硬门均通过。
- [x] 真实 TXT 导出同时包含 `SMOKE_REPAIRED_CHAPTER_1` 与 `SMOKE_POLISHED_CHAPTER_2`；
  SHA-256 为 `sha256:8f2debb7decec5c702c463bab44b61c2985618ebdfaa23936638bb6ab5a68e7e`。

## 2026-07-19 输出、预算、续写与部署门禁回填

- [x] 前端工作台的“运行状态 / Provider 思考 / 生成内容”三个开关彼此独立，并使用
  localStorage 保存偏好；关闭任何展示开关都不会停止后台任务。
- [x] `reasoning_chunk` 只转发 Provider 明确返回的 reasoning/thinking；不伪造隐藏思维链，
  不写入 Run、Step、项目业务表或导出文件。
- [x] `used_tokens` 明确为可观察 content + reasoning 的保守输出估算，不宣称是 Provider
  input/output 计费 Token；缺少统一定价源时成本预算 fail-closed 到人工门。
- [x] `continue_from_current`、`next_n_chapters` 和 `planning_only` 只对当前 Run 新生成章节执行
  质量闭环，不扫描或返修旧人工章节；`complete_book` 保持全书质量闭环。
- [x] 工作台 Playwright E2E：`7 passed`，覆盖创建、刷新恢复、sticky 指标、输出开关、
  pause/resume/cancel、guidance/decision、成本预算说明与最终导出描述符。
- [x] 新增 `novel_autopilot` gateway route group 的 9 个无登录鉴权探针；9/9 经真实
  Nginx → Rust 网关验证通过，其中 Run 列表 GET 加入 `deploy` 快速门禁。
- [x] 真实模型完整成书仍保持为独立发布 Smoke，不加入每次 `redeploy-fast.ps1`，避免部署过程
  产生模型成本、长等待或业务数据副作用。

## 2026-07-19 PostgreSQL NULL fence 修复与最终质量门

- [x] 定位完整成书 Smoke 在 `foundation` 业务提交后出现 `stale_version`：Run 已清空
  `active_background_task_id`，后置 Token 用量持久化仍使用可空 `.eq(Option)` fence。
- [x] `wait_for_budget_owned` 与 `increment_estimated_usage_owned` 对可空 task fence 显式分支：
  `Some(task_id)` 使用等值条件，`None` 使用 `IS NULL`，保留 version/epoch/task CAS。
- [x] 新增 NULL 预算门与非空 Token fence 回归测试；`cargo check --tests` 通过。标准 Windows
  MSVC 链接器当时触发 `LNK1318: PDB LIMIT (12)`，最终已用 `rust-lld` 完成实际断言验证。
- [x] `redeploy-fast.ps1 -SkipFrontendBuild -NoPause -NonInteractive` 成功；deploy gateway
  smoke 全绿。
- [x] 修复后真实 PostgreSQL 完整成书 Smoke：Project
  `4882c6ca-2b6e-455d-97fe-af8a3aa63d48`，Run
  `119da4f2-1023-4d07-aa2e-04055698c20b`，`status=completed`。
- [x] 结果：`completed_chapters=3`、`failed_chapter_count=0`、
  `pending_rewrite_count=0`、`total_word_count=9562`、`used_tokens=20943`。
- [x] Pause/Guidance/Restart/Resume 与迟到结果隔离通过：`paused_epoch=1`、
  `paused_version=8`、`guided_version=9`、`stale_step_count_before_restart=1`。
- [x] TXT 导出：3 章、28,189 bytes，返修与润色 marker 均存在；SHA-256 为
  `sha256:fe2092fdf06935713e5da633506afc43754fda61900e775ccc2b1906a5a4394b`。
- [x] 最终质量门：Rust fmt/check 通过；frontend lint/build 通过；工作台 E2E
  `7 passed (8.6s)`；仅保留既存 warnings。

## 2026-07-19 API 边界回归与 18 项验收证据收口

- [x] `novel_autopilot_runs` API handler 新增 owner 列表/详情/步骤读取、非 owner 404、
  非 owner 创建拒绝、非法 pause/resume/decision、stale cancel version、合法 cancel 与 terminal
  重复 cancel 回归测试。
- [x] API 返回继续使用 allowlist DTO；测试确认不暴露 `config_snapshot`、
  `active_scope_key` 等私有 Durable 字段。
- [x] Python smoke 工具测试 `6 passed`，migration Python 编译通过，migrator model registry
  `1 passed`。
- [x] Rust `cargo fmt --check` 与 `cargo check -j 1 --tests` 通过；37 个 warning 均为既存 warning。
- [x] 前端 lint/build 通过，Novel Autopilot Workbench Playwright E2E `7 passed (8.8s)`。
- [x] PRD 18 项验收标准已逐项映射到生产实现、测试断言和运行时证据；矩阵见
  `docs/22-mumunovel-optimization-route-completion-audit.zh-CN.md` 第 9.5 节。

### 历史环境限制与回滚

- Windows MSVC `link.exe` 曾在生成测试二进制时触发 `LNK1318: PDB LIMIT (12)`；
  最终改用 Rust 工具链自带 `rust-lld.exe`、关闭测试 debuginfo/incremental 后实际执行断言。
  未执行 `cargo clean`、`cargo fix`，也未降低质量门。
- 若需关闭新能力，可撤销 `novel_book_autopilot` route/task registration；旧 R7
  `novel_autopilot` NonResumable 契约、手工创作和既有导出不受影响。

## 2026-07-20（星期一）Autopilot review remediation

- [x] 新增后续 PostgreSQL migration，将审计 `actor_user_id` 扩至 `VARCHAR(100)`，并同步
  migrator model；未修改已发布迁移，最新 catalog head 为
  `20260720_audit_actor_id_capacity`。
- [x] Coordinator 在推进 Run 前消费 `Accept` / `Retry` / `Repair`：Accept 提交最新
  Generate/Repair 人工候选；Retry 要求 facts router 仍选择相同 Step；Repair 显式路由到
  ChapterRepair；不支持或过期决定 fail-closed。
- [x] Accept 后应用 `every_chapter`、`every_n_chapters`、`high_risk_only` 人工门；历史
  `every_volume` 在缺少可靠卷边界时进入 `waiting_human`，新 Run 直接拒绝该模式。
- [x] Create、resume 与 startup reconciliation 可重新调度已持久化但无 live task 的
  `queued` Run；任务准备或绑定失败不会留下不可恢复的 orphan Run。
- [x] Readiness migration fixture 使用共享 `POSTGRES_ALEMBIC_HEAD`，不再锁定中间版本。
- [x] 新 Run 对 `regenerate_existing=true` fail-fast；前端创建 DTO 固定为 `false`，历史配置
  仍可读取展示。
- [x] 新 Run 仅允许 `export_format=txt`；`markdown`、`docx` 在调度前拒绝，避免完成阶段
  必然进入人工门。
- [x] 人工候选正文仅持久化于 `chapter_draft_attempts`，candidate id 等于 terminal Step id；
  Run/Step 仅保存安全摘要和 digest。
- [x] Accept 使用单事务 Run/Step/task/chapter snapshot CAS，写 generation history，并按
  Generate/Repair 契约更新章节数与总字数。
- [x] 新增 ChapterRepair accept 回归测试，验证章节数不变、总字数仅增加新旧正文差值。

### Verification

- [x] `cargo fmt --manifest-path backend-rs/Cargo.toml -- --check`。
- [x] `cargo check --manifest-path backend-rs/Cargo.toml -j 1 --tests`；仅有既存 warning。
- [x] `python -X utf8 backend/tools/check_alembic_revision_health.py`：迁移图健康，最新 head
  为 `20260720_audit_actor_id_capacity`。
- [x] `python -X utf8 -m pytest backend/tests/test_tools/test_migrator_model_registry.py -q`：
  `1 passed`。
- [x] `npm --prefix frontend run lint` 与 `npm --prefix frontend run build`。
- [x] 已有 Workbench Playwright E2E：`8 passed`。
- [x] 使用 Rust 工具链自带 `rust-lld.exe` 绕过 MSVC PDB 限制，实际执行
  `manual_review_candidate` 定向测试：`4 passed`。
- [x] 实际执行 `services::novel_autopilot` 完整 service 测试：`131 passed`。
- [x] 修复并执行 `api::novel_autopilot_runs::tests`：`8 passed`；另执行 readiness
  migration metadata 测试：`1 passed`。
- [x] 同步修复 4 条过期测试预期：Generate fixture 正文、规范化 analysis report、
  非 owner list 的 404 隐藏语义、queued orphan resume 的重新调度语义。

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

## 2026-07-23 Autopilot 可观测性界面比例修复

- [x] 压缩后台任务中心 `Task Dossier` 卡片的嵌套 padding、gap、圆角和阴影；
  `Current Focus` 与 `Execution Pulse` 改为稳定单栏，移除会在 440px Drawer 中形成
  高窄文本块的冗长说明，保留状态、进度条、检查点和操作能力。
- [x] 为自动创作“步骤时间线”设置固定列布局：步骤列 220px，更新时间列 156px，
  其余诊断列按内容压缩；总最小宽度从 980 调整为 856，避免桌面宽度下无意义横向滚动。
- [x] 步骤键和错误代码使用单行 ellipsis + tooltip；“章节分析”等主步骤标签保持完整，
  不通过隐藏质量决定或错误代码换取空间。
- [x] Workbench E2E fixture 覆盖 `chapter_analyze`，并验证步骤列/更新时间列比例、分析行高度、
  桌面无横向溢出以及长步骤键省略样式。

### Verification

- [x] `npm --prefix frontend run lint`；仅有仓库既存 Hook warnings。
- [x] `npm --prefix frontend run build`；TypeScript 与 Vite 构建通过，保留既存循环 chunk 警告。
- [x] `npm --prefix frontend exec playwright test e2e/novel-autopilot-workbench.spec.ts`：
  `8 passed (9.8s)`。
- [x] Chromium DOM 尺寸断言确认：步骤列不小于 210px、更新时间列为 145–190px、
  步骤列宽至少为更新时间列的 1.25 倍、章节分析行高不超过 64px，桌面表格无额外溢出。

## 2026-07-23 Autopilot 步骤时间线章节列紧凑化

- [x] 将“章节”列从 80px 收紧到 72px，正文统一为无空格的 `第2章` 形式并居中展示。
- [x] 章节值显式设置 `white-space: nowrap`，避免窄布局下拆成多行并抬高时间线行高。
- [x] 按真实列宽预算将 `scroll.x` 从 856 调整为 848，保留窄屏内部滚动和桌面无额外溢出。
- [x] Workbench E2E 新增章节列 65–90px 范围及章节文本不换行断言。

### Verification

- [x] `npx playwright test e2e/novel-autopilot-workbench.spec.ts`（在 `frontend/` 下执行）：
  `8 passed (9.3s)`。
- [x] `npm run lint`：通过，仅有仓库既存 Hook warnings。
- [x] `npm run build`：通过，仅有既存 circular chunk warning。

## 2026-07-23 Autopilot 步骤时间线尝试列对齐

- [x] 保留“尝试”列 64px 安全宽度，避免双汉字表头在字体缩放下贴边或换行。
- [x] 表头与 attempt 数值统一居中，并为数值设置 `white-space: nowrap`。
- [x] Workbench E2E 新增尝试列 55–75px、表头/单元格居中及数值不换行断言。

### Verification

- [x] `npx playwright test e2e/novel-autopilot-workbench.spec.ts`（在 `frontend/` 下执行）：
  `8 passed (9.8s)`。
- [x] `npm run lint`：通过，仅有仓库既存 Hook warnings。
- [x] `npm run build`：通过，仅有既存 circular chunk warning。

## 2026-07-23 Autopilot 步骤时间线状态列对齐

- [x] 保留“状态”列 92px 安全宽度，兼容“排队中”“运行中”等三字状态标签。
- [x] 状态表头与单元格统一居中；清除 Ant Design Tag 默认右外边距，避免视觉偏移。
- [x] 状态标签显式设置 `white-space: nowrap`，避免窄布局或字体缩放下换行。
- [x] Workbench E2E 新增状态列 84–104px、表头/单元格居中、Tag 单行及零右边距断言。

### Verification

- [x] `npx playwright test e2e/novel-autopilot-workbench.spec.ts`（在 `frontend/` 下执行）：
  `8 passed (9.7s)`。
- [x] `npm run lint`：通过，仅有仓库既存 Hook warnings。
- [x] `npm run build`：通过，仅有既存 circular chunk warning。

## 2026-07-23 Autopilot 步骤时间线质量决定可读化

- [x] 新增强类型 `QUALITY_DECISION_META`，完整覆盖 `accept`、`auto_repair`、`retry`、
  `manual_review`、`reject` 五个质量决定。
- [x] 将内部英文枚举转换为“通过、自动修复、重试、人工复核、拒绝”中文 Tag。
- [x] 保留 104px 安全宽度，表头/单元格居中，Tag 去除默认右外边距并禁止换行。
- [x] Workbench E2E 新增质量决定列 96–116px、中文标签、居中、单行和零右边距断言。

### Verification

- [x] `npx playwright test e2e/novel-autopilot-workbench.spec.ts`（在 `frontend/` 下执行）：
  `8 passed (9.8s)`。
- [x] `npm run lint`：通过，仅有仓库既存 Hook warnings。
- [x] `npm run build`：通过，仅有既存 circular chunk warning。

## 2026-07-23 Autopilot 步骤时间线错误代码诊断展示

- [x] 保留错误代码原始值，不做中文翻译或业务化替换，确保日志与后端诊断可直接关联。
- [x] 保留 140px 列宽并显式左对齐；长代码单行 ellipsis，悬停 Tooltip 展示完整值。
- [x] 空值通过相同 Typography 边界显示次要文本 `—`，保持单元格对齐一致。
- [x] Workbench E2E 新增长错误代码 fixture，并覆盖实际列宽、左对齐、`nowrap`、
  ellipsis 和完整 Tooltip 断言。

### Verification

- [x] `npx playwright test e2e/novel-autopilot-workbench.spec.ts`（在 `frontend/` 下执行）：
  `8 passed (9.8s)`。
- [x] `npm run lint`：通过，仅有仓库既存 Hook warnings。
- [x] `npm run build`：通过，仅有既存 circular chunk warning。

## 2026-07-23 Autopilot 章节 Step Key 去重展示

- [x] 保持后端稳定 Step Key（如 `chapter:0001:analyze`）和持久化幂等契约不变。
- [x] 对具有 `chapter_number` 的步骤，仅常驻显示中文步骤名称和独立章节列，隐藏重复的
  第二行机器键；悬停步骤名称仍可查看完整 Step Key。
- [x] 非章节步骤继续显示原始 Step Key，以区分多个规划或完结阶段步骤。
- [x] Workbench E2E fixture 对齐真实零填充键格式，并覆盖默认隐藏、Tooltip 完整值以及
  非章节标识继续可见的行为。

### Verification

- [x] `npx playwright test e2e/novel-autopilot-workbench.spec.ts`（在 `frontend/` 下执行）：
  `8 passed (10.2s)`。
- [x] `npm run lint`：通过，仅有仓库既存 Hook warnings。
- [x] `npm run build`：通过，仅有既存 circular chunk warning。

## 2026-07-23 Autopilot 尝试次数语义化展示

- [x] 将步骤时间线中的裸 attempt 数字由 `1` 调整为紧凑的 `1次`，明确其次数语义。
- [x] 保持底层 attempt 数值、重试预算、列宽 64px、居中与单行展示不变。
- [x] Workbench E2E 更新为 `1次` 文本断言，并继续覆盖列宽、居中和 `nowrap`。

### Verification

- [x] `npx playwright test e2e/novel-autopilot-workbench.spec.ts`（在 `frontend/` 下执行）：
  `8 passed (10.1s)`。
- [x] `npm run lint`：通过，仅有仓库既存 Hook warnings。
- [x] `npm run build`：通过，仅有既存 circular chunk warning。

## 2026-07-23 SPA 静态入口热更新一致性修复

- [x] 确认时间线源码与 E2E 已采用“章节分析 / 第1章 / 1次”，章节 Step Key 仅在
  Tooltip 中保留。
- [x] 定位运行实例仍展示旧格式的根因：`backend/static` 以只读 bind mount 挂载，
  `/assets` 实时读取新文件，但 Rust 在启动时缓存旧 `index.html`，形成入口与资源混用。
- [x] 将 SPA fallback 改为每次文档响应异步读取当前 `index.html`，保留静态文件与
  `/api/*` 404 契约。
- [x] 新增 `static_index_reload_observes_frontend_rebuilds` 回归测试，覆盖同一路径入口
  文件被前端构建替换后的重新读取。
- [x] 重启本地 `mumunovel-rust`（不重建镜像、不执行迁移），使当前实例重新加载入口；
  HTTP 与挂载文件均指向 `assets/index-D6FYkLyx.js`，`/health` 返回 200。

### Verification

- [x] `cargo fmt --check`：通过。
- [x] `cargo check --tests`：通过，仅有仓库既存 unused warnings。
- [ ] `cargo test static_index_reload_observes_frontend_rebuilds`：测试代码完成编译，
  但本机 MSVC linker 两次触发 `LNK1318: PDB LIMIT (12)`，未进入测试执行阶段。
- [x] `npx playwright test e2e/novel-autopilot-workbench.spec.ts`：`8 passed (9.8s)`。
- [x] `npm run lint`：通过，0 errors；仅有 33 个仓库既存 Hook warnings。
- [x] 运行态入口一致性：HTTP `/` 与 `/app/static/index.html` 均引用
  `assets/index-D6FYkLyx.js`。
