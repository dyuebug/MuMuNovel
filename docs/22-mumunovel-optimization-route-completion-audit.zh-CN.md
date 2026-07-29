# MuMuNovel 优化路线完成审计（R0.1–R9）

> 初始审计日期：2026-07-16
> R9 最终复验日期：2026-07-19
> 审计对象：当前 `MuMuNovel` 工作树、Trellis 任务材料与真实 HTTP 验证输出
> 结论：**R0.1–R8 的原验收结论保持有效；后续独立授权的 R9 Durable Novel Autopilot 已完成，并通过暂停、重启恢复、质量闭环与真实导出的完整成书 Smoke。**

---

## 1. 审计范围与判定原则

本次只核验已授权的优化路线：R0.1、R0.2、R0.3、G0、R1、R2、R3、R4、R5、R6、
G1-Cancel、G1、R7、G2 与 R8。判定依据以当前源码、测试、验证材料和明确的受控边界为
准，不把 Trellis 任务的归档状态或 Git 提交状态误判为运行时功能未实现。

路线总览已将 R8 定义为已授权终点，并明确禁止未立项的无人值守、多步骤 Autopilot 扩展。
参见 `docs/15-ainovel-cli-comparison-and-mumunovel-optimization.zh-CN.md:93-120`。

## 2. 阶段证据矩阵

| 阶段 | 已实现能力 | 当前证据与验证 |
|---|---|---|
| R0.1 | PostgreSQL auth schema 兼容、密码 verifier 容量与 migration 合同 | 当前 catalog 复验为 21 revisions、123 条 upgrade SQL、head=`20260716_autopilot_invocation_audit`；见 `docs/16-r0.1-auth-schema-authorization-package.zh-CN.md:446-467`。 |
| R0.2 / R0.3 / G0 | 本地真实 PostgreSQL/Rust/Playwright E2E 与 Hosted Runner 证据 | 路线保留历史真实链路与 G0 GO 结论；它们是历史验证材料，不被误写为本次重新触发的远端运行。 |
| R1 | 后台任务快照原子写入、损坏恢复与跨端一致性 | PRD 的 11 项验收均已回填；当前路线全量 Rust 回归已覆盖。 |
| R2 | 单一恢复策略 registry、孤儿处理、持久化和前端操作指引 | 生产 task type 当前为 24 个；`backend-rs/src/tasks/recovery.rs:328-335` 与任务 PRD 已一致。 |
| R3 | 九态 workflow、唯一 `projects.status` owner、CAS 转换与 workflow UI | `novel_workflow_service.rs` 的 conditional update/CAS；本轮真实 PostgreSQL 并发测试为 1 passed，详见下节。 |
| R4 | Story Packet / Generation Intent、canonical digest 与安全 snapshot | `generation_contract_service` 的 schema/canonical/snapshot owner 与定向测试覆盖。 |
| R5 | 角色级模型策略、解析追溯与 generation execution audit | `role_model_policy_service` 与 `generation_execution_audit_service` 的版本化 policy 和 allowlist 审计。 |
| R6 | Business Checkpoint、幂等键、状态持久化与 resume 校验 | `business_checkpoint_service` 以及章节批量生成 resume 链路的 DB-backed 覆盖。 |
| G1-Cancel / G1 | cooperative cancellation、终态 owner 与统一合同门禁 | cancellation service、后台任务 terminal 约束，以及 G1 review/路线回归索引。 |
| R7 / G2 | 单次人工确认 Autopilot、受控 Tool、durable audit 与安全门禁 | `api/autopilot.rs`、Coordinator、Tool Contract 与 readonly history allowlist；无 retry/recovery/replay 或 unattended 扩展。 |
| R8 | 静态 Eval、创作档案安全投影、owner-scoped 运行指标 | `creative_archive_service`、`runtime_metrics_service`、R8 eval fixture/service 和只读项目 UI。 |

## 3. R3 真实 PostgreSQL CAS 复验

默认单元测试不连接外部数据库。为确认真实 PostgreSQL 下的并发行为，本轮在随机隔离的
本机 Docker 数据库中执行：

```text
postgres_concurrent_transitions_with_same_expected_phase_change_at_most_once
result: 1 passed; 0 failed; 0 ignored; 1801 filtered out
```

测试验证两个使用同一 `expected_phase` 的竞争转换最多只有一个能够成功写入。临时 role 与
数据库已删除；没有对生产环境执行操作。完整摘要见
`.trellis/tasks/07-14-novel-workflow-state-machine/validation/r3-postgres-cas-20260716.md`。

## 4. 当前工作树质量门

以下命令已针对当前工作树实际成功完成：

```text
cargo test --manifest-path backend-rs/Cargo.toml -j 1 -- --nocapture
  => 1801 passed / 1 ignored

npm --prefix frontend run e2e
npm --prefix frontend run lint
npm --prefix frontend run build
```

前端 E2E 成功；此前完整执行记录为 14 passed / 13 skipped。lint 与 build 均成功，输出中仅有
既有 React Hook dependency warning 和 circular chunk warning，未形成失败或路线功能缺口。
此外已通过：

```text
rustfmt --edition 2021 --check backend-rs/src/services/novel_workflow_service.rs
npm --prefix frontend run validate:text
npm --prefix frontend run e2e -- e2e/project-workflow-state.spec.ts
  => 6 passed
```

### 4.1 当前工作树复验（2026-07-16）

在本审计轮次中，已再次对当前工作树执行以下验证；结果均为成功：

```text
cargo test --manifest-path backend-rs/Cargo.toml -j 1 -- --nocapture
  => 1801 passed / 0 failed / 1 ignored

cargo test --manifest-path backend-rs/Cargo.toml -j 1   schema_migration_metadata_service::tests -- --nocapture
  => 34 passed / 0 failed

cargo test --manifest-path backend-rs/Cargo.toml -j 1   postgres_concurrent_transitions_with_same_expected_phase_change_at_most_once   -- --ignored --nocapture
  => 1 passed / 0 failed / 1801 filtered out

npm --prefix frontend run e2e
  => 14 passed / 13 skipped
npm --prefix frontend run lint
npm --prefix frontend run build
python backend/tools/check_alembic_revision_health.py
  => PostgreSQL head = 20260716_autopilot_invocation_audit
```

R3 测试仅使用新建的随机临时 PostgreSQL role/database，并在测试完成后删除。上述复验不连接或
改动生产数据库，也不对路线范围外的能力作出授权。

同一轮静态边界核验还确认：R7 API 仅保留 `actions` 与 `invocations`，请求 DTO 拒绝未知字段，
Tool Contract 仅解析 `transition_project_workflow`，`novel_autopilot` 为 `NonResumable`；R8
`creative-archive` 与 `runtime-metrics` 仍是只读路由。检查结果：
`AUTOPILOT_SCOPE_BOUNDARY=PASS`、`R8_READONLY_ROUTE_CONTRACT=PASS`。

## 5. 冻结边界与不在范围内的能力

当前完成状态不意味着下列能力已获授权或可隐式加入：Provider/MCP runtime、真实 prompt
网络评测、Pause/Resume/Steer、retry/replay/checkpoint/recovery、多个 Tool、多步骤自治，或
无人值守 Autopilot。R7 保持单次、人工确认且 `NonResumable`；R8 仅提供脱敏只读投影，
不成为 task/workflow/audit/checkpoint 的第二事实 owner。

上述内容是 2026-07-16 时点的冻结边界。2026-07-19 用户明确授权并建立独立任务
`.trellis/tasks/07-19-durable-novel-autopilot-orchestrator` 后，R9 才进入实施；它通过独立
Run/Step、后台任务类型、恢复策略与 API 实现，不修改旧 R7 `novel_autopilot` 的
`NonResumable` 单次 Tool 契约，也不倒灌为 R0.1–R8 的隐含验收条件。

## 6. 交付追踪与版本控制说明

部分 `.trellis/tasks/*/task.json` 仍显示 `in_progress`，包括当前 R8 任务。这反映的是当前
工作树尚未执行版本控制归档的生命周期状态，并不推翻已由源码、测试和验证材料支持的功能
完成结论。当前 Trellis archive 流程会自动创建 Git commit；由于本轮未取得 Git 提交授权，
审计保留这些元数据，不自行归档、不执行 `git add`、`git commit`、`git push` 或 reset。

工作树还包含跨阶段的未提交源码、前端、文档与 Trellis 文件。后续如需版本控制收口，应在
用户明确授权后，先按阶段审查 diff、拆分可审计提交，再执行归档；这属于交付治理工作，
不是优化路线的剩余功能开发。

## 7. 最终结论

- **功能开发剩余量：R0.1–R9 的既定运行时功能项为 0。**
- **当前可交付状态：R0.1–R8、G0、G1-Cancel、G1、G2 与独立 R9 Durable Novel Autopilot 的实现和验证证据已经形成闭环。**
- **后续自然动作：仅在获得明确 Git 授权后进行提交/归档与发布流程收口。**

---

## 8. 2026-07-19 补充说明：输出可观察性不构成 R9

R0.1–R8 的“无剩余路线阶段”结论保持不变。2026-07-19 新增的模型输出面板属于对既有 AI stream 和后台任务的 additive 可观察性投影：用户可选择查看生成内容与 Provider 明确返回的 reasoning/thinking，但该输出不成为新的持久事实，不改变任务恢复、业务 checkpoint、Workflow State Machine、Autopilot 或 R8 指标边界。

实现与验收说明见 `docs/23-model-reasoning-content-stream-panel-acceptance.zh-CN.md`。

---

## 9. 2026-07-19 最终结论：R9 Durable Novel Autopilot 已完成

`07-19-durable-novel-autopilot-orchestrator` 是在 R0.1–R8 完成结论之后独立授权的
可暂停恢复小说编排器。它没有改写旧 R7，而是新增 `novel_book_autopilot` Durable Run：

```text
基础设定 → 世界观 → 职业 → 角色 → 组织 → 大纲
→ 一纲多章展开 → 逐章生成 → 章节分析 → 自动返修
→ 全书审查 → 逐章润色 → 重新分析 → 完结硬门 → 真实导出
```

关键兼容边界：

- 旧 R7 `novel_autopilot` 仍是单次人工确认、单 Tool、`NonResumable`。
- R9 使用独立 Run/Step 持久化、version/epoch CAS、恢复策略和后台任务类型。
- 项目、章节、大纲、角色、组织、质量结果与导出仍由既有业务模型拥有。
- Provider 明确返回的 reasoning/thinking 仅用于可选前端展示，不写入 Run、Step、日志、
  checkpoint 或项目业务数据，也不展示或伪造隐藏思维链。

### 9.1 真实完整成书 Smoke 证据

2026-07-19 在最新 Rust 镜像与 PostgreSQL 容器上执行：

```powershell
python -X utf8 backend/tools/run_novel_autopilot_smoke.py `
  --base-url http://localhost:8005 `
  --username admin `
  --password admin123
```

结果：`ok=true`，Project `4882c6ca-2b6e-455d-97fe-af8a3aa63d48`，Run
`119da4f2-1023-4d07-aa2e-04055698c20b` 完成。

- `status=completed`，`execution_scope=complete_book`。
- `completed_chapters=3`，`failed_chapter_count=0`，`pending_rewrite_count=0`。
- `total_word_count=9562`，`used_tokens=20943`；这里的 Token 是可观察输出的保守估算。
- Pause/Guidance fence：`paused_epoch=1`、`paused_version=8`、`guided_version=9`。
- 重启前存在 1 个 stale step；旧 attempt 的迟到结果未覆盖新 epoch 状态。
- 21 个 Step 覆盖全部 planning、章节生成/分析/返修、全书审查/润色与导出。
- Provider 共接收 18 次确定性请求，三章均完成分析，章节 1 完成自动返修闭环。
- TXT 导出 3 章、28,189 bytes，SHA-256：
  `sha256:fe2092fdf06935713e5da633506afc43754fda61900e775ccc2b1906a5a4394b`。
- `repair_marker_present=true` 且 `polish_marker_present=true`，证明返修和润色正文进入最终导出。

本轮同时修复了后置 Token 用量持久化的 PostgreSQL NULL CAS fence：业务提交清空
`active_background_task_id` 后，仓库层必须使用 `IS NULL`，不能依赖可空 `.eq(None)` 的
跨数据库生成行为。修复后完整成书 Smoke 从 `foundation` 正常继续到 `export`。

### 9.2 如何判断“一键完整成书”成功

不能只看进度条到 100%。必须同时满足：

1. Run 为 `completed`，范围为 `complete_book`，无失败章节和待返修项。
2. 必需步骤类型全部完成，尤其是 `chapter_repair`、`book_review`、`book_polish`、`export`。
3. Pause/Restart/Resume 后 epoch/version fence 与 stale step 行为正确。
4. 章节正文、分析、返修和润色写入既有业务表，Run/Step 不保存正文副本。
5. `final_export_ref` 指向可下载的真实导出，下载内容摘要与描述符 SHA-256 一致。
6. reasoning 与正文双通道隔离；reasoning 只在 Provider 显式返回且用户开启显示时可见。

因此，MuMuNovel 当前已经具备类似 `ainovel-cli` 的自动化全流程串联能力，并额外具备
持久恢复、人工指导、质量闭环和可审计导出边界。

### 9.3 运行输出、reasoning 与预算语义

自动创作工作台提供三个互不依赖的显示开关：`运行状态`、`Provider 思考` 和`生成内容`。
开关只影响当前浏览器的可观察性，不控制后台 Run；用户关闭页面或关闭输出显示后，Durable Run
仍按持久化 checkpoint 继续执行。

正文以 `chunk` 事件实时展示。思考内容只在 Provider 明确返回 reasoning/thinking 字段时以
`reasoning_chunk` 事件展示；系统不推测、不补写模型隐藏思维链。reasoning 仅存在于运行时
SSE/浏览器内存，不写入 Run、Step、业务表、日志审计字段或最终导出。

预算字段需要按下列口径判断：

- `used_tokens` 是当前可观察 content + reasoning 的保守输出 Token 估算，不是 Provider 精确的
  input + output usage，也不能作为账单依据。
- 章节数、估算输出 Token、运行时、步骤尝试、连续 Provider 失败和连续质量失败均有调用前与
  提交后的限制检查。
- 当前没有统一 Provider 定价源。配置成本预算后，系统以
  `novel_autopilot_cost_estimation_unavailable` 安全进入人工门，不使用伪造价格继续运行。

### 9.4 续写保护与部署门禁

`continue_from_current` 会从第一个未完成章节继续到现有大纲末尾；`next_n_chapters` 只生成指定
数量。两种 partial scope 以及 `planning_only` 的质量事实仅指向当前 Run 刚生成的章节，不会因
历史分析缺失或低分而自动返修旧人工正文。只有 `complete_book` 执行全书级分析、审查、润色和导出。

部署 manifest 已增加 `novel_autopilot` route group，共 9 个无副作用鉴权探针，覆盖 Run 的
创建、列表、详情、步骤、暂停、恢复、取消、指导和人工决定。全部探针通过真实 Nginx → Rust
网关验证；其中 Run 列表探针属于 `deploy` profile，可在快速重部署时验证路由和鉴权边界。

完整三章成书 Smoke 继续作为独立发布门禁，不进入每次快速重部署。这样既保留真实模型全链路
验收，又避免部署阶段产生模型成本、长时间等待和测试项目副作用。

### 9.5 R9 的 18 项验收证据矩阵

判定规则：每项必须同时具备生产实现与自动化测试；涉及完整成书、恢复和导出的项目还必须有
真实 HTTP + PostgreSQL 运行证据。不能仅依据 PRD 复选框或前端进度条判定完成。

| # | 验收项 | 生产实现 | 测试/运行证据 | 结论 |
|---:|---|---|---|---|
| 1 | Run/Step migration、索引、外键、可逆 downgrade | `backend/alembic/postgres/versions/20260719_1200_durable_novel_autopilot.py`；`backend-rs/src/services/schema_migration_metadata_service.rs:1427` | migration Python 编译通过；migrator registry `1 passed` | 通过 |
| 2 | 同项目唯一活动 Run | `backend-rs/src/services/novel_autopilot/repository.rs:424` | `backend-rs/src/services/novel_autopilot/tests.rs:485` | 通过 |
| 3 | 创建/读取/控制 API 的 owner、非 owner、非法状态 | `backend-rs/src/api/novel_autopilot_runs.rs:188` | `backend-rs/src/api/novel_autopilot_runs.rs:1012`、`:1096`、`:1131` | 通过 |
| 4 | 重启后从 committed step 恢复 | `backend-rs/src/tasks/recovery.rs:33`；`backend-rs/src/services/novel_autopilot/recovery.rs:1` | `backend-rs/src/services/novel_autopilot/tests.rs:1430`；Smoke 检测到 1 个 stale step 后恢复完成 | 通过 |
| 5 | version/epoch/task/step fence 隔离迟到结果 | `backend-rs/src/services/novel_autopilot/repository.rs:667` | `backend-rs/src/services/novel_autopilot/tests.rs:540`、`:782`；Smoke `paused_epoch=1` | 通过 |
| 6 | 空项目自动完成规划资料 | `backend-rs/src/services/novel_autopilot/coordinator.rs:179` | planning adapter/repository tests；Smoke 从 `foundation` 推进到 `outline_expand` | 通过 |
| 7 | 至少三章生成、分析和进度累计 | `backend-rs/src/services/novel_autopilot/chapter_adapter.rs:510` | `backend-rs/src/services/novel_autopilot/chapter_repository_tests.rs:323`、`:637`；Smoke 完成 3 章、9562 字 | 通过 |
| 8 | accept/repair/retry/manual/exhausted 质量分支 | `backend-rs/src/services/novel_autopilot/chapter_quality_adapter.rs:1` | `backend-rs/src/services/novel_autopilot/chapter_repository_tests.rs:698`、`:971`；章节 1 自动返修进入导出 | 通过 |
| 9 | 四种 execution scope | `backend-rs/src/services/novel_autopilot/router.rs:1` | `backend-rs/src/services/novel_autopilot/tests.rs:25`、`:66`、`:150`、`:215` | 通过 |
| 10 | 全书审查、受限润色、完结硬门和导出 | `backend-rs/src/services/novel_autopilot/completion_gate_service.rs:21` | `book_review_tests.rs:224`、`book_polish_tests.rs:272`、`completion_gate_tests.rs:197`、`export_tests.rs:173`；Smoke 导出 3 章 TXT | 通过 |
| 11 | Token/成本/时长/章节/Provider 失败/质量失败预算 | `backend-rs/src/services/novel_autopilot/budget_guard.rs:1` | `budget_guard.rs:224`、`:236`、`:248`、`:268`；`tests.rs:2084`、`:2151`、`:2191` | 通过 |
| 12 | Pause/Resume/Cancel 不破坏业务事实 | `backend-rs/src/services/novel_autopilot/coordinator.rs:116` | `backend-rs/src/services/novel_autopilot/tests.rs:175`、`:540`、`:782`；Smoke Pause/Restart/Resume 成功 | 通过 |
| 13 | Guidance/Human gate 仅影响后续步骤且公开 DTO 不泄密 | `backend-rs/src/services/novel_autopilot/types.rs:239`；`backend-rs/src/api/novel_autopilot_runs.rs:604` | `backend-rs/src/api/novel_autopilot_runs.rs:823`、`:1012`、`:1131` | 通过 |
| 14 | 工作台创建、控制与刷新恢复 | `frontend/src/features/novel-autopilot/useNovelAutopilotWorkbench.ts:55` | `frontend/e2e/novel-autopilot-workbench.spec.ts:358`、`:449`、`:515` | 通过 |
| 15 | 阶段、章节、预算、质量、错误、导出可见 | `frontend/src/features/novel-autopilot/NovelAutopilotWorkbench.tsx:648` | Workbench E2E `7 passed (8.8s)` | 通过 |
| 16 | 状态/reasoning/正文独立开关 | `frontend/src/features/novel-autopilot/NovelAutopilotWorkbench.tsx:61`；`backend-rs/src/services/novel_autopilot/output_observer.rs:34` | `frontend/e2e/novel-autopilot-workbench.spec.ts:390`；`output_observer.rs:96`、`:124` | 通过 |
| 17 | Runtime Metrics、后台任务与 SSE 识别 Durable 类型 | `backend-rs/src/api/background_tasks.rs:79`；`frontend/src/services/modules/backgroundTaskTypes.ts:1` | `frontend/e2e/novel-autopilot-workbench.spec.ts:358`；gateway route probes 9/9 | 通过 |
| 18 | 格式、类型、测试、迁移、E2E、真实 HTTP Smoke | 跨层质量门 | Rust fmt/check、service `131 passed`、API `8 passed`、readiness `1 passed`、Python `6 passed + 1 passed`、frontend lint/build、E2E 7 passed、完整成书 Smoke `ok=true` | 通过；Windows 使用 `rust-lld` 实际执行 Rust 断言 |

综合结论：**18/18 项均具备可追溯证据**。因此 MuMuNovel 当前可以像
`ainovel-cli` 一样自动串联全流程生成一本小说，并且增加了 Durable checkpoint、暂停恢复、
人工指导、预算门、质量闭环和可验证导出。文学质量仍取决于模型、提示词、设定质量和人工审校，
不能把“流程完成”误认为“达到出版质量”。

### 9.6 2026-07-20（星期一）Autopilot 审查修复

本轮针对 Durable Novel Autopilot 的 7 条审查意见完成生产路径修复。审查结果不改变
R9 已具备完整成书能力的结论，但补齐了 PostgreSQL 审计容量、人工决定、人工门、失败重调度
和公开配置边界，避免核心工作流在特定输入或故障路径下失效。

| 审查问题 | 修复 | 验证 | 状态 |
|---|---|---|---|
| 审计 `actor_user_id` 小于本地用户 ID 容量 | 追加迁移扩为 `VARCHAR(100)`，同步 migrator model 和 Alembic head | 迁移健康检查通过；model registry `1 passed` | 已修复 |
| `Accept` / `Retry` / `Repair` 写入后未被执行 | Coordinator 消费 latest terminal Step 决定；Accept 原子提交候选，Retry 校验同路由，Repair 显式进入返修 | Autopilot service `131 passed`；人工候选 `4 passed` | 已修复 |
| Accept 后未执行人工门策略 | Accept 路径应用 every-chapter/every-N/high-risk；历史 every_volume 无边界时 fail-closed | Autopilot service `131 passed`，覆盖 every-chapter/every-N/high-risk 路由 | 已修复 |
| Run 持久化后调度失败会永久搁置 | Create、resume、startup reconciliation 识别无 live task 的 queued orphan 并重调度 | Run API `8 passed`，含 queued orphan resume 重调度与调度失败补偿 | 已修复 |
| readiness fixture 仍使用中间 migration head | fixture 改用共享 `POSTGRES_ALEMBIC_HEAD` | readiness `1 passed`；Alembic graph head 为 `20260720_audit_actor_id_capacity` | 已修复 |
| `regenerate_existing=true` 被接受但无运行语义 | 新 Run 在校验阶段返回 `not_supported`；前端创建值固定为 `false` | Autopilot service `131 passed`，含公开配置拒绝测试；前端 lint/build | 已修复 |
| `markdown` / `docx` 被接受但导出服务仅支持 TXT | 新 Run 仅允许 `txt`；不支持格式在调度前拒绝 | Autopilot service `131 passed`，含导出格式拒绝测试；前端 lint/build | 已修复 |

人工候选正文继续由业务表 `chapter_draft_attempts` 持有，候选 ID 与 terminal Step ID
确定性绑定。Run/Step 不持久化正文、Prompt 或 Provider reasoning。Accept 在单事务内执行
Run version/epoch/status/task fence、Step 与候选校验、Chapter 快照 CAS、历史写入、候选状态
更新和进度更新；Generate 候选增加完成章节数，Repair 候选仅调整新旧字数差。

质量证据：Rust `cargo fmt`、`cargo check -j 1 --tests`、Python Alembic 健康检查、migrator
registry、前端 lint/build 均通过。Windows 标准 MSVC `link.exe` 的 `LNK1318` PDB 限制已通过
Rust 工具链自带 `rust-lld.exe` 绕过；实际执行 Autopilot service `131 passed`、人工候选
`4 passed`、Run API `8 passed`、readiness migration metadata `1 passed`。同时修复了
Generate fixture 正文、规范化 analysis report、非 owner list 404 语义和 queued orphan
resume 重调度语义共 4 条过期测试预期。

结论：**7/7 审查意见已在生产路径修复并完成实际 Rust 断言验证**。当前没有由这 7 条
审查意见产生的已知功能代码缺口；后续仍需持续保留完整成书 Smoke 与跨层回归门禁。
