# 自动创作质量门、失败终态与时间修复实施计划

## 前置门禁

- [x] 用户审核并批准 `prd.md`、`design.md`、`implement.md`。
- [x] 获批后执行
  `python .trellis/scripts/task.py start .trellis/tasks/08-01-autopilot-analysis-time-fix`。
- [x] 进入实现前完整执行 `trellis-before-dev`，重新加载目标包规范和当前 Git 状态。
- [x] 保留工作区既有未提交文件，不回退、不批量格式化无关模块。

## Phase 1：锁定回归测试

- [x] 在 `story_repair_quality_context_owner.rs` 增加失败测试：
  `details.outline_alignment.applicable=false` 时，`outline_alignment_rate=0`
  不得进入 `failed_metrics`、`weakest_metric`、`weak_metric_count`、
  `focus_areas` 或 `repair_targets`。
- [x] 增加历史兼容测试：没有 `details` 或 `applicable` 的扁平指标仍按旧逻辑参与质量门。
- [x] 增加多指标边界测试：过滤不适用项后，其他真实弱项仍能触发 `auto_repair`。
- [x] 在 Autopilot Repository/Adapter 测试中先固定当前错误行为：第三次质量耗尽
  进入 `waiting_human` 时必须同时存在可操作候选，并能通过 terminal Step ID 关联。
- [x] 在 API 测试中增加 UTC RFC 3339 断言，在 Workbench E2E 中增加北京时间断言。

## Phase 2：修复质量指标适用性

- [x] 为 `QualityMetricDescriptor` 增加 `detail_key`，覆盖所有描述指标与
  `details` 子对象的映射。
- [x] 在 `collect_quality_metric_signals()` 统一判断 `applicable`，默认缺失为兼容的 `true`。
- [x] 确保 repair guidance 和 quality gate 只调用过滤后的信号集合，不在两个派生函数重复规则。
- [x] 验证没有适用指标时不会触发 `expect()` panic，并返回空失败项/空最弱项的明确边界。
- [x] 运行聚焦质量 owner 测试。

## Phase 3：建立安全失败诊断

- [x] 在现有 Novel Autopilot 服务边界新增小型共享诊断模块或等价局部类型，定义
  `source_code`、`category`、provider、model、HTTP 状态和 retryable 的 allowlist 结构。
- [x] 为 `ChapterAnalysisGenerationError` 和 `ChapterRepairGenerationError` 增加安全诊断投影；
  保留 `Display` 不暴露原始 `Generation(String)`。
- [x] 将 timeout、429、5xx、认证/配置、响应解析和 unknown 映射为稳定错误码；
  unknown 继续使用旧聚合码。
- [x] Provider/model 从执行配置读取并限制长度，不从 Prompt/正文/响应猜测。
- [x] 更新分析/返修 Adapter 的结构化日志，加入 Run、Step、attempt、task、category、
  provider、model 和可选 HTTP 状态；删除原始错误输出路径。
- [x] 添加 redaction 测试，证明 API Key、Prompt、正文、URL query 和响应体不会进入安全诊断。

## Phase 4：原子人工候选与无候选故障

- [x] 将 `chapter_generate::finish_quality_retry()` 的最终耗尽分支改为调用既有
  `persist_manual_review_candidate()`，不要再调用只切状态的 `finish_waiting_human()`。
- [x] 在一个事务内插入 `chapter_draft_attempts`、终结 Step、设置 Run
  `waiting_human` / `last_error_code` 并清理 active task/current step。
- [x] `chapter_repair` 最终质量耗尽继续调用 `persist_manual_review_candidate()`，
  并核对候选 ID、digest、质量摘要和章节快照作用域。
- [x] `chapter_analyze` 有有效分析结果且需要人工确认时保留 `waiting_human`；
  Provider/解析失败没有分析结果时不得伪造候选。
- [x] 保留返修预算内 `build_retry_draft_attempt()` 和作用域校验，确认下一次尝试仍消费最新重试证据。
- [x] 人工候选保存完整正文，Run/Step/Task/SSE/日志只保存 candidate ID、digest 和 allowlist 质量摘要。
- [x] 增加 CAS、事务回滚、候选可用性、无第 N+1 次尝试和历史重试证据隔离测试。
- [x] 增加 Accept/Retry/Repair/Stop 回归，证明候选缺失或章节快照变化时拒绝部分提交；
  构造无候选 Accept 请求时同步返回 `409 human_decision_candidate_unavailable`，且不修改
  guidance、Run version 或创建后台任务。

## Phase 5：后台任务状态一致性

- [x] 保留质量候选的 `NovelAutopilotTickOutcome::AwaitingHuman`，task result 增加
  candidate ID、稳定原因码和 `dispatch_status=waiting_human`。
- [x] 后台 Task 可以完成当前 tick，但 message 必须改为“候选已保存，等待人工复核”，
  不能使用笼统的“编排步骤已完成”。
- [x] Provider/配置/上下文/响应无效的无候选故障使用独立结果和文案，不携带 candidate ID；不可重试或预算耗尽时进入 `waiting_human`，仅允许 Retry/Repair/Stop。
- [x] 测试 Run、Step、Task result 和候选 ID 可关联，且正文不进入 Task/SSE。

## Phase 6：修复 API 时间契约

- [x] 在 `novel_autopilot_runs.rs` 引入 UTC `NaiveDateTime` -> RFC 3339 的局部序列化帮助函数。
- [x] `run_view()` 的 created/updated/started/paused/completed 全部使用同一帮助函数。
- [x] `step_view()` 的 created/updated/started/completed 全部使用同一帮助函数。
- [x] API 测试覆盖微秒精度、`Z` 后缀和 `null`，确认其他 DTO 字段不变。
- [x] UTC 显示修复不修改既有时间列语义，也不迁移、回填或平移历史数据库时间值。

## Phase 7：前端状态文案与时间回归

- [x] 让 `describeRunError` 区分“候选已保存等待复核”和“Provider 无候选故障”，
  不再对所有错误统一显示“运行进入人工处理”。
- [x] 为新稳定错误码补集中中文映射；未知码仍显示原码。
- [x] 保持 `formatTimestamp()` / `formatRuntime()` 使用标准 `Date` 解析，不添加固定偏移兼容代码。
- [x] 将 E2E 时间 fixture 改为 UTC `Z`，使用固定 `Asia/Shanghai` 时区断言创建、更新、Step 时间和运行时长。
- [x] 保留 waiting-human 决策测试，新增候选可用时启用人工决定、无候选时不显示
  “接受并继续”的测试。

## Phase 8：规范同步与质量门禁

- [x] 更新 `.trellis/spec/backend/durable-novel-autopilot.md`：补充质量耗尽必须先原子
  持久化可操作候选，Provider 无候选故障不得冒充候选人工复核。
- [x] 更新相关错误/日志/质量规范中的稳定诊断和 `applicable=false` 过滤约束；
  使用 `trellis-update-spec` 或项目等价流程保持可执行契约。
- [x] 使用 `trellis-break-loop` 复盘“适用性元数据在下游丢失”和“人工等待状态缺少候选不变量”两类根因。
- [x] 使用 `trellis-check` 执行规范一致性、复用、跨层数据流和缺失测试检查。

## Phase 9：补齐 ChapterGenerate 质量反馈闭环

- [x] 为 ChapterGenerate 定义独立 retry evidence source/state 和构造器，复用
  `chapter_draft_attempts` 的完整正文持久化/抽取契约，但不得获得 `waiting_human`
  人工候选的 Accept 语义。
- [x] evidence payload 写入并校验 `run_id`、`run_epoch`、chapter ID/number、
  `source_content_digest`、`candidate_content_digest` 和 `step_attempt`；候选 digest
  必须与完整正文一致，word count 和完整标志必须自洽。
- [x] 新增 Repository 原子入口，在同一事务内执行章节快照 CAS、插入 retry
  evidence、终结当前 Step、更新 Run 质量失败计数/version 并释放 current step/task
  fence；任一步骤失败时完整回滚。
- [x] 修改 `chapter_adapter::finish_quality_retry()` 的预算内分支调用该原子入口，
  Task result 仅返回 candidate ID/digest、attempt 和 `build_safe_quality_diagnostic()`
  allowlist，不返回正文、Prompt、reasoning 或原始质量 payload。
- [x] 在下一次 ChapterGenerate 调用 Provider 前加载 attempt 小于当前 attempt 的最新
  evidence；只接受同 Run/epoch/chapter/source digest 且 candidate digest 自洽的记录，
  将上一候选正文与有界质量反馈/修复方向注入受控生成输入。
- [x] 覆盖首轮无 evidence、第二轮正确消费、跨 Run/epoch/章节隔离、source digest
  变化、candidate digest 损坏、并发 CAS、事务回滚和正文不进入 Run/Step/Task/SSE/
  日志的回归测试。

## Phase 10：实现 Provider 持久化退避

- [x] 新增独立 PostgreSQL Alembic revision：upgrade 为
  `novel_autopilot_runs` 添加 UTC 语义 `next_attempt_at TIMESTAMP WITHOUT TIME ZONE
  NULL`，无 default/backfill；downgrade 删除该列。不要同时修改已经发布的基线
  revision 产生重复 `ADD COLUMN`。
- [x] 在 Rust `schema_migration_metadata_service` 注册同一 revision 的 upgrade/
  downgrade metadata；更新 SeaORM Run model 和冻结 Python migrator model。保持历史
  ee0a initial baseline 不变；fresh schema 验证通过完整 revision chain 先创建 Autopilot
  表、再由新 revision 添加列/index，并验证最终 metadata 的类型与 nullability，不重写
  历史 Run/Step 时间值。
- [x] 在 typed Provider generation error 边界提取并规范化 `Retry-After` delta-seconds 或
  HTTP-date，只透传有界秒数/时间提示，不保留原始 header、URL、响应体或异常文本；禁止
  从错误字符串或响应正文解析。
- [x] 新增纯函数退避 owner：有效 `Retry-After` 优先，否则使用 capped exponential；
  再以 `run_id/step_key/attempt` 生成 deterministic、非负、受限
  jitter，最终 delay 始终不超过统一 cap。测试使用固定时钟和稳定 seed。
- [x] 可重试 Provider 失败的 Repository 事务在终结 Step、增加失败计数、更新 Run
  version/状态和释放 task fence 的同时写 `next_attempt_at=now+delay`；不可重试或预算
  耗尽进入人工处理时写 `NULL`。
- [x] coordinator 连续 tick、API dispatch retry、孤儿 queued Run 修复和 startup
  reconciliation 统一检查数据库 `next_attempt_at`；未到期时通过 version/epoch/
  active-task CAS 至多创建并绑定一个 payload 携带同一 `not_before` 的持久化 pending
  Task，但到期前不标记 running、不 claim Step、不调用 Provider。重启后按数据库原
  due 重建 pending Task，到期后 claim 层 DB 围栏与多实例 CAS 只允许一个执行者获胜。
- [x] 在所有 Provider-backed Step 成功提交、进入 `waiting_human`/manual review、
  pause、cancel/stop 路径清空 `next_attempt_at`；resume 不恢复旧值，迟到旧 epoch/task
  不能覆盖或清除新值。
- [x] 增加 migration upgrade/downgrade/fresh-chain、frozen initial baseline 无新列、
  既有行 `NULL`、typed Retry-After 端到端透传与优先/cap、无效 hint fallback、jitter 重启复现、未到期单个 pending Task、到期前
  不 running/claim/调用 Provider、重启按原 due 重建、到期多实例单次 claim 和所有
  清理状态的回归测试。

## Phase 11：扩展规范同步与最终质量门禁

- [x] 将 ChapterGenerate retry evidence 与 Provider persistent backoff 的稳定契约
  同步到 `.trellis/spec/backend/durable-novel-autopilot.md` 和跨层思考指南，确保 task
  artifacts 与长期规范没有冲突。
- [x] 使用 `trellis-check` 重新检查 DB -> Repository -> Adapter -> Coordinator/API ->
  Task/SSE 的完整数据流、所有写/清理路径和同层一致性。
- [x] 重新执行 Rust migration metadata、Repository、startup reconciliation、
  ChapterGenerate、Provider failure、frontend lint/build 和聚焦 Playwright 验证；已实现的
  Phase 9/10 本地持久化退避范围全部通过，未实现项继续单独保持未勾选。

## 验证命令

具体测试过滤器在实现后按新增测试名补全，至少执行：

```powershell
cargo fmt --manifest-path "backend-rs/Cargo.toml" -- --check
cargo check --manifest-path "backend-rs/Cargo.toml"
cargo check --tests --manifest-path "backend-rs/Cargo.toml"
cargo test --manifest-path "backend-rs/Cargo.toml" story_repair_quality_context_owner
cargo test --manifest-path "backend-rs/Cargo.toml" novel_autopilot
cargo test --manifest-path "backend-rs/Cargo.toml" schema_migration_metadata
cargo test --manifest-path "backend-rs/Cargo.toml" chapter_generate_quality_retry
cargo test --manifest-path "backend-rs/Cargo.toml" provider_retry_backoff
npm --prefix "frontend" run lint
npm --prefix "frontend" run build
Push-Location "frontend"
npx playwright test "e2e/novel-autopilot-workbench.spec.ts"
Pop-Location
```

若 Windows MSVC 出现已知 `LNK1318: PDB LIMIT`，使用项目规范记录的
`rust-lld`、关闭 debuginfo 和 incremental 的方式重跑聚焦测试；必须分别报告
实际执行的测试与仅完成的 `cargo check --tests`，不能把编译通过写成测试通过。

## 风险文件与回滚点

| 风险点 | 文件 | 控制措施 |
| --- | --- | --- |
| 质量门行为面较广 | `story_repair_quality_context_owner.rs` | 源头过滤、历史 payload 兼容测试 |
| 人工候选原子事务 | `chapter_repository.rs` | 复用现有 CAS、候选插入失败和回滚测试 |
| ChapterGenerate retry evidence 串线或泄露 | `chapter_adapter.rs` / `chapter_repository.rs` / generation service | 完整作用域 + digest 校验；正文仅在 `chapter_draft_attempts` |
| 返修收敛能力 | `chapter_repair_adapter.rs` | 同时保留中间重试证据和最终人工候选 |
| 后台任务共享逻辑 | `api/background_tasks.rs` | 只调整 Autopilot outcome 文案和安全结果 |
| Provider 信息泄露 | generation error / Adapter logging | allowlist 构造、长度限制、敏感词回归 |
| 退避重启后失效或重复调度 | Run model/Repository/coordinator/API reconciliation | nullable `next_attempt_at` + DB not-before + version/epoch/CAS |
| schema owner 漂移 | Alembic revision / Rust migration metadata / SeaORM / frozen migrator model | upgrade/downgrade/fresh-chain metadata 一致性测试 |
| 时间重复转换 | `novel_autopilot_runs.rs` / Workbench | 后端唯一补时区，前端不手工偏移 |

业务代码可以按 Phase 独立回滚；`next_attempt_at` 使用 nullable 扩展，部署时先执行
upgrade 再发布新二进制。回滚时先切回不读取该列的旧二进制，再执行 downgrade；
downgrade 会丢失尚未到期的 not-before，因此回滚窗口必须限制 Provider 立即重试突刺。
历史已处于 `waiting_human` 的 Run 保持原样，既有时间值不做批量数据修正。

## 完成定义

- [x] PRD 全部验收项有对应自动化测试或明确人工验证证据。
- [x] 最新故障类型可从 Run/Step/Task/安全日志一致定位。
- [x] 不适用质量指标不再导致假返修。
- [x] 质量重试耗尽后的 `waiting_human` 一定存在完整、可操作的人工候选。
- [x] 最终候选可安全 Accept/Retry/Repair/Stop，中间自动重试仍能继承证据。
- [x] Provider 无候选故障不显示候选已保存，也不允许执行候选接受操作。
- [x] 所有 Autopilot 时间字段为 RFC 3339 UTC，页面在北京时间显示正确。
- [x] 安全扫描未发现 API Key、完整 Prompt、正文或原始 Provider 响应泄露。
- [x] Rust、frontend 和聚焦 E2E 验证结果已如实记录。
- [x] ChapterGenerate 预算内质量重试已形成完整、作用域正确、原子且不泄露正文的反馈闭环。
- [x] Provider retry 已在 typed header 边界端到端使用 capped Retry-After；缺失 hint 时使用 local backoff + deterministic jitter，并通过数据库 `next_attempt_at` 跨重启执行 not-before。
- [x] pause/cancel/manual/success 清理、迁移 upgrade/downgrade/fresh schema 和并发单次调度均有通过的自动化回归证据。

## 实际验证结果

以下为本轮实际结果。typed `Retry-After`、数据库持久化退避、ChapterGenerate 候选
反馈闭环、时间显示、无候选人工操作约束和并发/恢复矩阵已经通过自动化验证；真实
PostgreSQL upgrade、新 Rust 镜像、OpenAI-compatible HTTP transport、后端重启恢复和
三章完整整本创作也已在部署环境验证。生产数据库 downgrade 和第三方外部 Provider
故障注入没有执行，不能把聚焦测试扩张为外部网关实测结论：

- `cargo check`、`cargo check --tests`：通过。
- `cargo test ... story_repair_quality_context_owner -- --test-threads=1`：12 passed。
- `cargo test ... novel_autopilot -- --test-threads=1`：176 passed。
- `cargo test ... retry_after -- --test-threads=1`：6 passed。
- `cargo test ... schema_migration_metadata -- --test-threads=1`：40 passed；同步修复新增 revision 后遗留的 replay 期望列表和 SQL step count。
- `npm --prefix frontend run lint`：0 errors，33 个既有 Hook warnings。
- `npm --prefix frontend run build`：通过；保留既有 circular chunk warning。
- `npx playwright test e2e/novel-autopilot-workbench.spec.ts`：12 passed。
- `cargo fmt -- --check`、`git diff --check`、UTF-8 无 BOM 检查：通过。
- 部署前创建旧镜像回滚标签
  `mumunovel-rust:rollback-autopilot-20260807-103700`，并生成经过
  `pg_restore --list` 校验的 PostgreSQL custom-format 全量备份。
- PostgreSQL migration head 已升级为 `20260807_autopilot_retry_backoff`；
  `next_attempt_at` 为 nullable `timestamp without time zone`，并存在
  `(status, next_attempt_at)` 索引。
- Rust 容器已切换到镜像 `sha256:e372167dcb66ca730c6bb576fdbfb1329fa4f489b655fc7b00a45f1bab4b97ba`；
  `/health`、`/readyz` 和 10 个 gateway probes 通过。
- 历史卡死 Run `c6af04d5-2fe2-49d4-b6f5-3886a1b75cd1` 在启动恢复时把第三次
  running Step 收口为 `stale/service_restarted`，Run 转为无 active task 的
  `waiting_human/novel_autopilot_step_attempts_exhausted`，没有再次出现
  `invalid_config(candidate_chapter_status)` 或继续调用 Provider。
- 部署级 smoke Run `b42538ec-9e46-4647-96fd-9aae43daf1aa` 完成 3/3 章节、20 个
  completed Step 和 1 个预期 stale Step；覆盖生成、5 次分析、返修、书评、润色、
  暂停 fence、guidance、重启恢复及 TXT export digest，最终无失败章节、待返修或错误码。
- smoke 的 18 次 OpenAI-compatible HTTP 请求全部被分类，临时 Provider 设置在 finally
  中恢复；Run API 的 created/updated/started/completed 均带 RFC 3339 `Z`，并正确转换为
  `Asia/Shanghai` 的 `+08:00` 时间。
