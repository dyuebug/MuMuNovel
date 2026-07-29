# Implementation Plan：后台任务恢复策略注册表

## Steps

1. 读取 backend/Trellis 规范，复核 `tasks/recovery.rs`、`tasks/types.rs`、generic dispatcher、
   snapshot serde 和前端终态字段消费链路。
2. 在 `tasks/recovery.rs` 新增四级 `TaskRecoveryPolicy`、静态 23 项策略注册表、未知类型
   fallback 和 registry 唯一性/覆盖测试。
3. 在 `TaskRecord` 增加四个 serde-compatible optional 恢复字段，并保持 `TaskRecord::new()`
   签名和 active 初始化语义不变。
4. 提取 orphan recovery projection：按 policy 和 checkpoint 可用性生成 error/message、
   terminal reason/label、review/can-resume 标志。
5. 改造 `recover_orphan_tasks()`：只处理 active 记录，保留 result/progress/已有 checkpoint
   字段，并写入结构化 recovery diagnostics 和安全日志。
6. 更新 `api/background_tasks.rs` 的测试 TaskRecord fixture，并添加 payload 序列化兼容测试。
7. 在 `tasks/recovery.rs` 增加 pending/running、终态不变、四策略、checkpoint 降级、字段保留
   和未知类型测试。
8. 运行 targeted tests、完整 Rust 门禁和前端 build；检查 UTF-8 无 BOM 与 diff whitespace。
9. 将恢复策略 registry 契约写入 backend quality spec，并更新 R2 任务记录和优化路线状态。

## Expected Code Scope

- `backend-rs/src/tasks/recovery.rs`
- `backend-rs/src/tasks/types.rs`
- `backend-rs/src/api/background_tasks.rs`（仅 fixture/serialization test，除非实现发现接口缺口）
- `.trellis/spec/backend/quality-guidelines.md`
- `.trellis/tasks/07-12-background-task-recovery-policy-registry/*`
- `docs/15-ainovel-cli-comparison-and-mumunovel-optimization.zh-CN.md`

前端生产代码原则上无需修改，因为现有 model 已消费四个恢复字段；若验证证明恢复 label
不可见，再做最小 presentation helper 调整，不创建新 store。

## Validation

```powershell
cargo fmt --manifest-path "backend-rs/Cargo.toml" -- --check
cargo test --locked --manifest-path "backend-rs/Cargo.toml" tasks::recovery::tests -- --nocapture
cargo test --locked --manifest-path "backend-rs/Cargo.toml" api::background_tasks::tests -- --nocapture
cargo check --locked --manifest-path "backend-rs/Cargo.toml"
cargo test --locked --manifest-path "backend-rs/Cargo.toml"
cargo clippy --locked --manifest-path "backend-rs/Cargo.toml" --all-targets -- -D clippy::correctness -D clippy::suspicious
npm --prefix frontend run build
E:/Code/SoftWare/Tools/Git/cmd/git.exe diff --check
```

## Review Gates

- [x] 注册表正好覆盖 23 个已知类型且没有重复项。
- [x] unknown/default 永远不会得到 `can_resume=true`。
- [x] checkpoint_resumable 只有在 checkpoint 为非空 object 时可恢复。
- [x] manual_confirmation 不会触发自动执行或 generic resume。
- [x] recovery 只更新 active 状态，terminal records 完全不变。
- [x] 旧 snapshot 缺少新增字段时能正常加载。
- [x] API success/data 兼容壳不变。
- [x] 日志不包含 payload、result、checkpoint 或用户内容。
- [x] 没有改动 R1 文件协议或章节数据库 runtime-state owner。

## Rollback Points

- 可单独回滚 `recovery.rs` 策略投影而不影响 R1 快照原子性。
- 四个 `TaskRecord` 字段均为 optional，回滚不需要数据迁移。
- 不新增 crate/npm 依赖，不修改 Cargo.lock/package-lock。
- 不通过自动重放 payload 或将未知类型误标为 restartable 来扩大能力。

## Planning Decision

R2 不自动重启 generic 任务。当前 snapshot 没有原始 payload，自动重放既无法完整构造输入，
也可能对已部分写入数据库的任务产生重复副作用。MVP 只负责策略分类、可操作终态投影和
现有 resume 能力发现；真正的业务 checkpoint 标准化留给 R6。


## 实施记录（2026-07-12）

### 实际代码结构

- `backend-rs/src/tasks/recovery.rs` 新增四级 `TaskRecoveryPolicy`、23 项唯一静态注册表、
  `recovery_policy_for()` 安全 fallback 和集中式 orphan recovery projection。
- 所有 active 孤儿任务继续投影为 `failed`；按策略写入 `terminal_reason`、
  `terminal_label`、`review_required` 和 `can_resume`，不增加 paused/recovering 状态。
- `checkpoint_resumable` 仅在 checkpoint 为非空 JSON object 时声明可恢复；null、scalar、
  array、空 object 和缺失 checkpoint 全部降级为 `checkpoint_missing`。
- 恢复诊断复用 `touch_checkpoint()`；对象型 checkpoint 保留自定义字段，非对象 checkpoint
  替换为安全诊断对象。`result`、`progress` 和既有 `started_at` 保持不变。
- 恢复日志只记录 task id、task type、policy 和 projected status，不记录 payload、result、
  checkpoint 内容或用户文本。
- `TaskRecord` 增加四个 serde optional 字段，`TaskRecord::new()` 签名、snapshot version 1、
  API route 和 success/data 兼容壳均保持不变。
- 前端 API 类型和 background task model 原本已消费四个 snake_case 字段；本轮新增共享
  `getTaskRecoveryGuidance()`，任务中心可展示 restart、resume、checkpoint missing、manual
  review 和 non-resumable 的明确操作指引。
- “继续”按钮仍由既有 `canResumeTask()` 控制，只对章节批量/单章生成开放；未新增 generic
  resume API，也未改变章节数据库 runtime-state owner。

### 新增测试

1. 23 项注册表数量、唯一性及 5/2/16 策略分布。
2. unknown/future task type 的 `non_resumable` fallback。
3. 四种策略的终态语义和 `can_resume`/`review_required` 组合。
4. checkpoint 缺失、null、scalar、array、空 object 和非空 object 判定。
5. pending/running 恢复及 completed/failed/cancelled 完全不变。
6. result、progress、自定义 checkpoint 和既有 started_at 保留。
7. 非法 checkpoint 转换为安全诊断对象。
8. 旧 version-1 TaskRecord 缺少新增字段时反序列化兼容。
9. API top-level 与 `data` 同时暴露恢复语义字段。

### 验证结果

- `tasks::recovery::tests`：7/7 通过。
- `tasks::types::tests`：2/2 通过。
- `api::background_tasks::tests`：17/17 通过。
- `cargo fmt --check`：通过。
- `cargo check --locked`：通过。
- `cargo test --locked`：1543/1543 通过。
- Clippy `-D clippy::correctness -D clippy::suspicious`：通过。
- `npm --prefix frontend run build`：通过。
- `npm --prefix frontend run lint`：通过。
- UTF-8 无 BOM、`git diff --check`：通过。

### 实现偏差与剩余风险

- 设计初稿误认为 `TaskRegistry::update()` 会自动刷新 `updated_at`；实际实现显式写入当前
  时间，设计文档已同步修正。
- R2 只投影恢复能力，不自动重放 generic payload，也不执行章节 resume 命令；真正的统一
  业务 checkpoint schema、幂等键和恢复执行协议仍属于 R6。
- R0 本地真实链路已通过 PostgreSQL migration、Rust server 和 `/health`，但 auth 因
  `user_passwords.password_hash VARCHAR(64)` 无法容纳 Argon2 哈希而返回 HTTP 500。必须先在
  用户确认后完成 R0.1 Schema 兼容修复，再通过本地真实 E2E 和 GitHub runner 证据；因此
  G0 尚未满足，R3 不应提前进入实施。

## 可靠性审计补充（2026-07-13）

### 问题与裁决

启动流程原先在 `load_from_disk()` 和 `recover_orphan_tasks()` 后直接启动 1.5 秒周期保存。
恢复投影只存在于内存；若服务在首次周期保存前再次退出，磁盘仍可能保留旧 active snapshot，
导致下次启动重复投影恢复诊断。采用 KISS 修复，不改变 snapshot version 或持久化错误契约：

- `recover_orphan_tasks()` 返回实际恢复数量；空注册表返回 `0`。
- 仅当恢复数量大于 `0` 时，`main.rs` 在启动周期保存和对外服务前调用现有
  `save_to_disk()`，立即写入原子快照。
- `save_to_disk()` 继续沿用 best-effort 日志错误策略；本轮不引入启动 fail-closed，避免扩大
  R2 范围或改变既有可用性契约。

### 新增验证

- 空注册表返回 `0`。
- pending + running + completed + failed + cancelled 混合集合返回 `2`，且只修改 active 记录。
- checkpoint 可恢复与非法 checkpoint 场景分别返回 `1`。
- `production_ci_contract_tests` 固化
  `load -> recover -> conditional save -> periodic workers -> router` 启动顺序。
- `.trellis/spec/backend/quality-guidelines.md` 已同步返回签名、即时保存、错误边界和测试合同。
- `tasks::recovery::tests`：9/9 通过。
- 新增启动持久化源码合同测试：1/1 通过。
- `cargo check --locked`：通过。
- `cargo test --locked`：1589/1589 通过。
- Clippy `-D clippy::correctness -D clippy::suspicious`：通过。
- `npm --prefix frontend run build`：通过。
- UTF-8 无 BOM、LF、无尾随空白，`git diff --check`：通过。

### 剩余边界

即时快照写入失败时会记录错误但不阻止服务启动。这与现有周期保存契约一致；若未来需要把
恢复快照持久化升级为启动硬门禁，应单独设计可观测错误返回、降级行为与运维恢复流程，不能
在 R2 内隐式改变生产启动语义。

## 注册表覆盖防漂移审计（2026-07-13）

### 生产集合审计

- 通用 `execute_task()` 实际包含 20 个唯一字符串任务类型，全部已在恢复策略注册表中显式登记。
- 注册表额外 3 项不是遗漏或死分支：`chapter_single_generate`、`chapters_batch_generate` 由既有
  章节数据库 runtime-state/resume owner 管理；`chapter_analysis` 由章节分析 API 状态投影到
  前端任务中心。
- 创建 API 对 unknown task type 的既有兼容行为保持不变；unknown 仍执行后失败并在重启恢复时
  使用 `NonResumable` 安全 fallback。本轮不新增 allowlist、不改变 API 或任务 Schema。

### 实现裁决

- `tasks::recovery` 新增 `has_explicit_recovery_policy()`，只读查询现有静态注册表，用于区分
  “显式登记为某策略”和“未登记后落入安全 fallback”。
- `production_ci_contract_tests` 直接解析真实 `execute_task()` 顶层 match arm 的字符串模式，
  断言执行器类型唯一且每项都具有显式恢复策略。
- 合同不维护第二份 20 项手写清单，也不把执行器重构为动态 handler registry，保持 KISS 和
  当前生产分发行为。

### 验证结果

- 生产执行器集合：20 项；执行器未登记项：0；注册表额外业务 owner 类型：3 项。
- `tasks::recovery::tests`：9/9 通过。
- 新增执行器覆盖防漂移合同：1/1 通过。
- `production_ci_contract_tests`：13/13 通过。
- `cargo fmt -- --check`：通过。
- `cargo check --locked`：通过。
- `cargo test --locked`：1590/1590 通过。
- Clippy `-D clippy::correctness -D clippy::suspicious`：通过；只有既有普通 warning。
- UTF-8 无 BOM、LF、无尾随空白，`git diff --check`：通过。

## 最终 PRD 验收矩阵（2026-07-13）

- **策略完整性**：`TASK_RECOVERY_POLICIES` 包含 23 个唯一生产类型，分布为 restartable 5、
  checkpoint-resumable 2、manual-confirmation 16；unknown 通过 `recovery_policy_for()` 安全降级为
  `NonResumable`。证据：`registry_contains_exactly_23_unique_known_task_types`、
  `unknown_task_type_uses_non_resumable_fallback`。
- **执行器防漂移**：源码合同直接提取通用 `execute_task()` 顶层字符串 match arm，20 个实际执行
  类型全部满足 `has_explicit_recovery_policy()`；不维护第二份手写执行器清单。证据：
  `generic_background_task_executor_types_have_explicit_recovery_policies`。
- **恢复投影**：四种策略、checkpoint 可用/缺失、pending/running 与三种既有终态均有定向测试；
  所有恢复结果继续使用兼容的 `failed` 状态和可操作终态字段。
- **数据保留与兼容**：恢复保留 result、progress、现有 checkpoint 自定义键和已有 started_at；
  `TaskRecord::new()` 保持签名，新字段为 optional/None，旧 version-1 JSON 可反序列化。
- **API 与前端**：现有 `/background-tasks` 兼容 payload 在顶层和 `data` 同时携带非空恢复字段；
  前端复用 `backgroundTaskPresentation` 与既有章节 resume owner，没有第二套 task store/endpoint。
- **启动耐久性**：启动顺序固定为 load → recover → conditional atomic save → periodic workers →
  router；即时保存失败继续遵循现有 best-effort 日志边界。
- **安全与非目标**：日志不输出 payload/result/checkpoint 内容；未新增 migration、数据库 Schema、
  TaskStatus、snapshot version、自动重放线程或 generic resume endpoint。
- **质量门禁**：Rust 定向测试、生产合同、fmt/check/全量测试、Clippy correctness/suspicious、前端
  build/lint 和文件格式检查作为最终审查证据；本任务保持 `in_progress`，不在本轮归档。

## 最终质量审查结果（2026-07-13）

- 设计合同已同步：补充 `has_explicit_recovery_policy()`、20 个通用执行器类型与 3 个独立
  owner 类型的边界、源码驱动覆盖合同，以及启动恢复即时落盘顺序。
- `implement.jsonl`、`check.jsonl` 已删除模板行，并各自登记 6 个真实 backend/frontend
  规范上下文；JSONL 结构、UTF-8 无 BOM、LF 和无尾随空白检查通过。
- `cargo fmt -- --check`：通过。
- `tasks::recovery::tests`：9/9 通过。
- `production_ci_contract_tests`：13/13 通过。
- `cargo check --locked`：通过。
- `cargo test --locked`：1590/1590 通过。
- `cargo clippy --locked --all-targets -- -D clippy::correctness -D clippy::suspicious`：通过；
  仅保留既有普通 warning，不在 R2 扩大清理。
- `npm --prefix frontend run build`：通过，包括 service facade、可见文本编码、TypeScript 和
  Vite 生产构建。
- `npm --prefix frontend run lint`：通过，0 error；33 个既有 Hook warning 不属于本轮改动。
- 本轮未修改数据库 Schema、snapshot version、API endpoint、TaskStatus 或章节 resume owner；
  当前 Trellis 任务继续保持 `in_progress`，不归档、不提交。

## 浏览器恢复语义合同补充（2026-07-13）

- 新增 `frontend/e2e/background-task-recovery-semantics.spec.ts`，使用 Playwright API mock
  保留真实 `ProtectedRoute`、service 轮询、Zustand persist/归一化和任务中心 UI 链路。
- 从 localStorage 中四个重启前 `running` 任务出发，覆盖 `restart_required`、
  `resume_available`、`manual_review` 和 unknown `non_resumable` 四类 Rust 风格恢复终态。
- 显式断言章节批量任务最终保留 `checkpoint`、`terminalReason=resume_available`、
  `reviewRequired=false`、`canResume=true`，并在对应任务卡片显示恢复说明和单任务“继续”按钮。
- 定向 Playwright：1/1 通过；`npm run lint`：0 error（仅既有 Hook warning）；
  `npm run build`：通过（仅既有 circular chunk warning）；UTF-8 无 BOM、LF、无尾随空白，
  `git diff --check`：通过。
- 该证据不启动 Rust server、不连接 PostgreSQL，不替代 R0.2 本地真实 E2E；未修改数据库
  Schema、snapshot version、API endpoint、TaskStatus 或章节 resume owner。当前任务继续保持
  `in_progress`，不归档、不提交。

## 恢复日志隐私合同补充（2026-07-13）

- 新增 `orphan_recovery_log_contract_exposes_only_safe_metadata` 源码合同测试，直接定位
  `"Recovered orphan task"` 对应的 `info!` block，避免另建 tracing 捕获依赖或改变生产日志行为。
- 恢复日志只允许 `task_id`、`task_type`、`recovery_policy`、`projected_status` 四个 metadata
  字段；显式拒绝整条 `record` 以及 `result`、`checkpoint`、`payload` 内容进入日志。
- `cargo fmt -- --check`：通过；`tasks::recovery::tests`：9/9 通过；
  `cargo check --locked`：通过，仅保留既有 dead-code warning。
- 本轮未修改生产恢复投影、数据库 Schema、migration、snapshot version、API endpoint 或
  TaskStatus；当前 Trellis 任务继续保持 `in_progress`，不归档、不提交。

## 恢复写入原子复核与幂等补充（2026-07-13）

- `recover_orphan_tasks()` 的只读快照现在只保留 active 候选 task id；单条恢复在
  `TaskRegistry::update()` 写锁内重新检查最新状态，避免陈旧候选覆盖已经 completed、failed 或
  cancelled 的终态记录。
- 恢复策略、checkpoint 合并、result 存在性和进度诊断均读取写锁内的最新 `TaskRecord`；只有
  实际从 active 投影为 failed 的记录才增加 `recovered_count` 并输出单条恢复日志。
- 新增 `stale_orphan_candidate_does_not_overwrite_terminal_record` 和
  `repeated_recovery_is_idempotent`，分别保护并发终态不被覆盖以及重复恢复不改写时间戳、消息、
  错误和 checkpoint。
- `cargo fmt -- --check`：通过；`tasks::recovery::tests`：11/11 通过；
  `cargo check --locked`：通过；`cargo test --locked`：1593/1593 通过；Clippy
  correctness+suspicious：通过，仅保留既有普通 warning。
- 本轮未修改数据库 Schema、migration、snapshot version、API endpoint、TaskStatus 或启动
  best-effort 持久化边界；当前 Trellis 任务继续保持 `in_progress`，不归档、不提交。

## 前端任务类型与 Rust Owner/恢复策略跨层合同（2026-07-13）

- 新增 `FRONTEND_BACKGROUND_TASK_TYPES_SOURCE` 和
  `extract_single_quoted_union_literals()`，在 Rust production CI test 中直接读取前端
  `BackgroundTaskType` 字符串联合，不维护第二份 20 项执行器手写清单。
- 新增 `frontend_background_task_types_match_rust_execution_and_recovery_owners`：前端 24 个唯一
  类型必须由 23 个 known 类型加唯一 `unknown` sentinel 组成；23 个 known 类型必须与 Rust
  恢复策略注册表完全相等。
- 合同继续从真实 `execute_task()` match 提取 20 个通用执行器类型，并断言其余类型只能是
  `chapter_analysis`、`chapter_single_generate`、`chapters_batch_generate` 三个明确独立 owner。
- `unknown` 不得被显式登记，必须继续通过 `recovery_policy_for()` 投影为 `NonResumable`，保持
  现有创建 API 的安全兼容 fallback。
- `cargo fmt -- --check`：通过；production CI contracts：14/14 通过；
  `cargo check --locked`：通过；`cargo test --locked`：1594/1594 通过；Clippy
  correctness+suspicious：通过，仅保留既有普通 warning。
- 本轮未修改前端运行时代码、任务创建 API、数据库 Schema、migration、snapshot version 或
  TaskStatus；当前 Trellis 任务继续保持 `in_progress`，不归档、不提交。

## 恢复时间线语义修正（2026-07-13）

- 审计发现启动恢复会在 `started_at` 缺失时写入恢复时间，从而把从未进入 running 的 pending
  孤儿任务误报为已经开始执行；正常生命周期中只有 `mark_task_running()` 拥有初始化
  `started_at` 的职责。
- `recover_orphan_task()` 现在只刷新 `completed_at` 和 `updated_at`，并精确保留 `started_at`：
  既有时间不变，缺失值继续为 `None`。该修复不改变终态投影、恢复策略、checkpoint、result、
  progress 或 API 结构。
- 现有 active/terminal 状态矩阵新增 `started_at=None` 保持为空的断言；既有
  `recovery_preserves_result_progress_custom_checkpoint_and_existing_started_at` 继续保护非空开始
  时间不被改写。
- `cargo fmt -- --check`：通过；`tasks::recovery::tests`：11/11 通过；production CI contracts：
  14/14 通过；`cargo check --locked`：通过；`cargo test --locked`：1594/1594 通过；Clippy
  correctness+suspicious：通过，仅保留既有普通 warning。
- 本轮未修改数据库 Schema、migration、snapshot version、API endpoint 或 TaskStatus；当前 Trellis
  任务继续保持 `in_progress`，不归档、不提交。

## 恢复事实时间一致性补强（2026-07-13）

- 审计发现 `touch_checkpoint()` 与 `recover_orphan_task()` 分别采样 `Utc::now()`，使同一次原子恢复
  投影中的 checkpoint `updated_at` 与任务 `completed_at`/`updated_at` 存在微秒级漂移，削弱审计
  时间线的一致性。
- `tasks/checkpoint.rs` 新增可注入时间的 `touch_checkpoint_at()`；既有 `touch_checkpoint()` 签名和
  普通调用行为保持不变，仅作为使用当前时间的兼容 wrapper。
- 孤儿恢复现在先采样唯一 `now`，再同时用于 checkpoint `updated_at`、任务 `completed_at` 和
  `updated_at`。现有保留性测试解析 RFC3339 checkpoint 时间并断言三个字段表示完全相同的时间点。
- `cargo fmt -- --check`：通过；`tasks::recovery::tests`：11/11 通过；production CI contracts：
  14/14 通过；`cargo check --locked`：通过；`cargo test --locked`：1594/1594 通过；Clippy
  correctness+suspicious：通过，仅保留既有普通 warning。
- 本轮未修改数据库 Schema、migration、snapshot version、API endpoint、TaskStatus、恢复策略或
  启动持久化边界；当前 Trellis 任务继续保持 `in_progress`，不归档、不提交。

## Generic TaskRecord 终态单调性补强（2026-07-13）

- `TaskRegistry` 新增 `update_if(predicate, updater)`，在同一写锁内完成条件判断和更新，消除
  生命周期 owner 的 `get() -> update()` TOCTOU。
- `mark_task_running()` 仅允许 `Pending -> Running` 并返回执行准入；所有 generic spawn 在未取得
  准入时立即退出，因此已经取消的 pending task 不会开始业务执行。
- `complete_task()` 与 `fail_task()` 仅允许 active record 进入终态；`cancel_active_task()` 在单一写锁
  内完成用户归属检查、active 检查、checkpoint 与 `Cancelled` 投影。三个终态 owner 均复用单一
  事实时间，迟到 executor 结果不再覆盖 `Cancelled` 或 recovered `Failed`。
- channel state bridge 仅更新 active record；channel `success` 不再直接拥有 `Completed`，最终 result、
  `completed_at` 和 `updated_at` 统一由 `complete_task()` 写入。terminal record 不再接收迟到
  progress/message/result。
- 新增 4 个行为测试：
  `cancelled_task_is_not_reactivated_or_overwritten_by_executor_completion`、
  `recovered_failed_task_remains_terminal_with_recovery_semantics`、
  `channel_success_waits_for_complete_task_to_own_terminal_projection`、
  `channel_state_sync_does_not_mutate_cancelled_terminal_record`。
- 验证结果：`cargo fmt -- --check` 通过；`api::background_tasks::tests` 21/21；
  `tasks::recovery::tests` 11/11；production CI contracts 14/14；`cargo check --locked` 通过；
  `cargo test --locked` 1598/1598；Clippy correctness+suspicious 通过，仅保留既有普通 warning。
- 文件质量：目标 Rust 文件均为 UTF-8 无 BOM、LF、无尾随空白，目标范围 `git diff --check` 通过。
- 剩余边界：已进入 Running 的底层操作尚无统一 cooperative cancellation token；本轮只保证状态和
  迟到 registry 写入安全。未修改数据库 Schema、migration、snapshot version、API endpoint 或
  TaskStatus；当前 Trellis 任务继续保持 `in_progress`，不归档、不提交。

## 启动恢复原子 owner 统一（2026-07-13）

- 生命周期 owner 全量审计未发现新的终态回退；`update_workflow_state()` 只写 workflow metadata，
  不修改 `TaskStatus`，没有在缺少独立 API 合同证据时扩大 active-only 限制。
- `recover_orphan_task()` 改为 `TaskRegistry::update_if(task_id, |task| task.status.is_active(), ...)`；
  active predicate 与恢复投影统一在同一 registry 写锁内执行。
- 删除普通 `update()` 闭包内早退与闭包外 mutable metadata 回传；恢复日志 metadata 改从
  `update_if()` 返回的最新记录派生，terminal/stale candidate 通过 predicate 失败直接保持不变。
- 恢复策略、单一事实时间、checkpoint、日志隐私、API、Schema、TaskStatus 与启动持久化边界均未
  改变；不新增 generic payload 持久化、自动重放、章节 resume endpoint 或 cooperative cancellation。
- 新增 `background_task_startup_recovery_uses_atomic_update_if_owner` 源码防漂移合同：只截取
  `recover_orphan_task()` 函数体，断言 `update_if()`、active predicate 和 updated-record metadata
  派生顺序，并拒绝普通 `update()` 或闭包外 `recovered_metadata` 回传。
- `TaskRegistry::update_if()` 新增 4 条 primitive 单元合同：missing task 不执行任何 callback、predicate
  拒绝时不执行 updater 且记录不变、接受时各执行一次并返回最新记录、两个并发 Pending 准入只能
  有一个成功；直接证明 predicate 与 updater 由同一 registry 写锁串行化。
- 验证结果：`tasks::registry::tests` 4/4、`tasks::recovery::tests` 11/11、
  `api::background_tasks::tests` 21/21、production CI contracts 15/15、完整 locked Rust 测试
  1603/1603、`cargo fmt -- --check`、`cargo check --locked` 与 Clippy correctness+suspicious 全部通过；
  仅保留既有普通 warning。
- 文件质量：`recovery.rs`、`registry.rs`、`background_tasks.rs`、`checkpoint.rs` 均为 UTF-8 无 BOM、
  LF、无尾随空白，目标范围 `git diff --check` 通过。当前任务继续保持 `in_progress`，不归档、不提交。

## Channel bridge 终止合同补强（2026-07-13）

- 复核 `spawn_channel_progress_bridge()` 生命周期后确认：terminal record 拒绝迟到 channel 更新时，
  `TaskRegistry::update_if()` 返回 `None`，bridge 随即 `break`，生产实现已符合质量规范；本轮不修改
  生产逻辑。
- 新增 `channel_progress_bridge_stops_after_terminal_update_is_rejected` 行为测试：构造已取消记录与
  `done=false` 的迟到 progress/message/status，使用 1 秒 timeout 直接等待 bridge handle，证明首次
  250ms 轮询发现 terminal record 后自行结束，并断言状态、消息、进度、时间戳和 result 均未变化。
- 验证结果：精确测试 1/1、`api::background_tasks::tests` 22/22、`tasks::registry::tests` 4/4、
  `tasks::recovery::tests` 11/11、production CI contracts 15/15、完整 locked Rust 测试 1604/1604、
  `cargo fmt -- --check`、`cargo check --locked` 与 Clippy correctness+suspicious 全部通过；仅保留既有
  普通 warning。
- 本轮未修改数据库 Schema、migration、snapshot version、API endpoint、TaskStatus、恢复策略或
  cooperative cancellation 边界；当前 Trellis 任务继续保持 `in_progress`，不归档、不提交。

## Running 准入与取消并发合同补强（2026-07-13）

- 复核 `mark_task_running()` 与 `cancel_active_task()` 后确认：两者均通过
  `TaskRegistry::update_if()` 在同一 registry 写锁内完成 predicate 与状态投影，生产实现已具备原子
  transition owner；本轮不修改生产逻辑。
- 新增 `concurrent_running_admission_and_cancellation_leave_task_cancelled` 并发行为测试：使用
  `tokio::sync::Barrier(3)` 从 `Pending` 同时释放 running admission 与 cancellation，覆盖两种合法锁
  顺序。无论 admission 还是 cancellation 先取得写锁，取消都必须成功并拥有最终 `Cancelled` 终态，
  checkpoint `event` 必须为 `cancelled`，后续 running admission 必须失败；若 admission 先成功，允许
  `started_at` 保留为真实执行历史。
- 验证结果：精确并发测试 1/1、`api::background_tasks::tests` 23/23、
  `tasks::registry::tests` 4/4、`tasks::recovery::tests` 11/11、production CI contracts 15/15、
  完整 locked Rust 测试 1605/1605、`cargo fmt --all -- --check`、`cargo check --locked` 与
  Clippy correctness+suspicious 全部通过；仅保留既有普通 warning。
- 本轮未修改数据库 Schema、migration、snapshot version、API endpoint、`TaskStatus`、恢复策略或
  cooperative cancellation 边界；当前 Trellis 任务继续保持 `in_progress`，不归档、不提交。

## 终态拒绝事件 SSE 静默合同补强（2026-07-13）

- 审计 `mark_task_running()`、`complete_task()`、`fail_task()`、`cancel_active_task()` 与 channel
  bridge 后确认：通用 lifecycle owner 只在 `TaskRegistry::update_if()` 返回最新记录时 fanout，生产
  实现已经保证被 stale/terminal predicate 拒绝的 transition 不广播事件；本轮不修改生产逻辑。
- 扩展 `cancelled_task_is_not_reactivated_or_overwritten_by_executor_completion`：取消完成后先订阅
  `TaskStreamHub`，再依次触发迟到 running admission、completion 和 failure，除继续断言 registry 终态
  与恢复字段不变外，还要求 receiver 返回 `TryRecvError::Empty`，直接证明不会向客户端伪造
  progress/result/error 事件。
- 验证结果：精确测试 1/1、`api::background_tasks::tests` 23/23、
  `tasks::registry::tests` 4/4、`tasks::recovery::tests` 11/11、production CI contracts 15/15、
  完整 locked Rust 测试 1605/1605、`cargo fmt --all -- --check`、`cargo check --locked` 与
  Clippy correctness+suspicious 全部通过；仅保留既有普通 warning。
- 本轮未修改生产逻辑、数据库 Schema、migration、snapshot version、API endpoint、`TaskStatus`、
  恢复策略或 cooperative cancellation 边界；当前 Trellis 任务继续保持 `in_progress`，不归档、不提交。

## TaskStreamHub 订阅与 fanout 原子性补强（2026-07-13）

- `TaskStreamHub::subscribe()` 改为在单一 sender-map 写锁内完成查找和首次 channel 创建，消除
  并发首次订阅者分别创建 sender、后写覆盖前写并断开早期 receiver 的竞态。
- `TaskStreamHub::fanout()` 改为异步等待 sender-map 读锁，克隆 sender 后释放 guard 再发送；锁竞争
  不再通过 `try_read()` 静默丢弃 progress/result/error/done 事件。
- SSE `stream_task()` 在完成授权后先建立订阅，再重新读取 `TaskRegistry` 构造 connected 快照；授权
  与订阅之间发生的终态转换必须由最新 connected 快照或已排队 broadcast 事件之一覆盖。
- `TaskStreamHub::fanout_terminal()` 在同一 sender-map 写锁内移除 sender 并取得最终发送 owner；
  `done`、`error`、`cancelled` 已切换到该路径。既有 receiver 能消费缓冲终态并随后关闭，后续重连
  创建新 channel 并由 connected 快照读取终态，避免已完成任务的 sender 常驻进程生命周期。
- 新增 `fanout_waits_for_sender_map_lock_instead_of_dropping_event`、
  `concurrent_first_subscribers_share_one_broadcast_channel`、
  `terminal_fanout_delivers_then_releases_sender_for_reconnect` 和
  `task_stream_subscription_refreshes_snapshot_after_authorization_gap`，分别锁定锁竞争交付、并发首次
  订阅复用、终态回收/重连与授权—订阅间隙补偿合同。
- 验证结果：`tasks::stream::tests` 3/3、`api::background_tasks::tests` 24/24、
  `tasks::registry::tests` 4/4、`tasks::recovery::tests` 11/11、production CI contracts 15/15、
  完整 locked Rust 测试 1609/1609、`cargo fmt --all -- --check`、`cargo check --locked` 与 Clippy
  correctness+suspicious 全部通过；仅保留项目既有普通 warning。
- 本轮未修改数据库 Schema、migration、snapshot version、API endpoint、`TaskStatus`、恢复策略或
  cooperative cancellation 边界；当前 Trellis 任务继续保持 `in_progress`，不归档、不提交。

## 优化路线最终裁决（2026-07-13）

- 当前任务继续保持 `in_progress`，仅允许完成一个剩余有界收口：`BroadcastStream` 发生 lag 时，
  从 `TaskRegistry` 读取最新记录并复用既有 `connected` 事件发送状态快照；不得通过单纯扩大
  broadcast capacity 掩盖一致性问题。
- 该收口通过定向与完整 Rust 门禁后冻结 R2。已经进入 `Running` 的底层 AI/数据库操作缺少统一
  cooperative cancellation token，属于跨业务执行架构，必须另立 `G1-Cancel` Trellis 任务，不得混入
  当前恢复策略注册表任务。
- 主优化链固定为 `R0.1 -> R0.2 -> R0.3 -> G0 -> R3 -> R4 -> (R5 + R6) -> G1-Cancel
  -> G1 -> R7 -> G2 -> R8`。R0.1 已在明确授权后完成，当前应立即进入 R0.2 本地真实 E2E。
- 本次授权不覆盖生产数据库 migration、真实 downgrade、production downgrade CLI、其他表字段或
  历史 revision；未过 G0 不进入 R3，未过 G1 不开发 Autopilot。
- 路线决议已同步到 `docs/15-ainovel-cli-comparison-and-mumunovel-optimization.zh-CN.md` 的
  “17.1 最终执行路线决议”，作为后续任务排序、范围审查和 No-Go 判断的默认依据。

## SSE broadcast lag 快照重同步与 R2 冻结（2026-07-13）

- generic background-task SSE 不再通过 `BroadcastStream::filter_map()` 静默忽略 lag。新增
  `TaskStreamState` 与 `next_task_stream_data()`，直接拥有 `broadcast::Receiver` 的接收循环。
- receiver 发生 `Lagged(skipped)` 时，只记录 `task_id` 与跳过数量，随后对同一 channel 调用
  `resubscribe()` 跳到当前尾部，再读取 `TaskRegistry` 并复用既有 `connected` 事件发送最新快照。
  该顺序既丢弃所有快照前旧缓冲，也覆盖 resubscribe 与 registry 读取之间的 lifecycle 竞态。
- active 快照发出后继续接收 resubscribe 之后的新事件；terminal 快照发出后关闭 stream，避免旧
  progress 在最新终态后回放，也避免终态任务重新创建 sender-map 常驻条目。registry 记录已被 TTL
  清理时不伪造新协议事件，继续等待现有 channel 的关闭或后续事件。
- 新增 `lagged_task_stream_resynchronizes_and_drops_stale_buffer`，使用容量 2 的真实 broadcast
  channel 制造两次 lag，覆盖 running 快照、旧缓冲丢弃、新事件继续、completed 快照与终态关闭。
- 验证结果：精确 lag 测试 1/1、`tasks::stream::tests` 3/3、
  `api::background_tasks::tests` 25/25、`tasks::registry::tests` 4/4、
  `tasks::recovery::tests` 11/11、production CI contracts 15/15、完整 locked Rust 测试
  1610/1610、`cargo fmt --all -- --check`、`cargo check --locked`、Clippy
  correctness+suspicious 与 `git diff --check` 全部通过；仅保留项目既有普通 warning。
- R2 的恢复策略、终态单调性、sender 并发/生命周期、授权—订阅窗口和慢订阅者 lag 一致性均已
  形成稳定合同，功能范围据此冻结。cooperative cancellation 继续保留为独立 `G1-Cancel` 任务；
  当前 Trellis 任务状态保持 `in_progress`，不归档、不提交。

## R0.1 Auth Schema Compatibility 实施与验证（2026-07-13）

- 用户明确回复“确认授权后续直接开发”，按 `docs/16-r0.1-auth-schema-authorization-package.zh-CN.md`
  的最小范围实施；未修改其他表字段、历史 19 个 revision 或 production downgrade 边界。
- Rust migration head 更新为 `20260712_password_hash_phc_text`，catalog/executable catalog 追加第 20
  revision；upgrade 仅执行 `VARCHAR(64) -> TEXT` 与注释更新，guarded downgrade 在任何 verifier
  长度大于 64 时显式失败。production executor 继续只遍历 `upgrade_steps`。
- 同步 initial schema、Python frozen Alembic source-map、Python migrator metadata、health/readiness
  revision fixtures 和固定合同测试。
- 静态与单元证据：Alembic revision health 通过；migration metadata 33/33；readiness 6/6；health
  13/13；auth 7/7；password hash 10/10；完整 locked Rust 1612/1612；fmt、check 和 Clippy
  correctness+suspicious 全部通过。
- 隔离 PostgreSQL 18 fresh database 执行 20 个 revision、120 个 step，最终 head 和 `TEXT NOT NULL`
  正确；old-head database 只执行新 revision 的 2 个 step，legacy SHA256 逐字节不变。
- 实际本地登录成功后 verifier 升级为 97 字符 Argon2 PHC；guarded downgrade 返回非零并保持
  `TEXT` 与 verifier 不变；`release-readiness-preflight` 返回 `release_ready=true`。
- R0.1 至此完成，下一路线阶段为 R0.2 本地 PostgreSQL + Rust + Playwright 真实 E2E。生产数据库
  migration、真实 downgrade 和 G0 放行仍未授权、未执行。
