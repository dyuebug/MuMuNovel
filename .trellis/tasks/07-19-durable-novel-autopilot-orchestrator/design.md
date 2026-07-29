# Design: Durable Novel Autopilot Orchestrator

## 1. Context and decision

MuMuNovel 已拥有 Novel Workflow、向导生成、章节批量/单章生成、质量门、Business Checkpoint、Cooperative Cancellation、Generation Contract、Runtime Metrics 和模型双通道输出。当前 R7 `novel_autopilot` 则被明确限定为一次人工确认、一次受控 Tool 调用、`NonResumable`。

本设计新增并行但不冲突的 `novel_book_autopilot` durable execution path。旧 R7 API、task type、audit schema 和 NonResumable 语义保持不变。

## 2. Ownership model

```text
novel_autopilot_runs
  owns: orchestration lifecycle, config snapshot, budget counters,
        current cursor, human gate, epoch/version, final export reference

novel_autopilot_step_runs
  owns: one logical step attempt, child task correlation, safe digests,
        decision/error/timestamps

existing business owners
  Project/NovelWorkflow -> project/workflow state
  Outline               -> volume/chapter plan
  Chapter               -> accepted chapter content
  Analysis/Plot          -> quality facts
  BackgroundTask         -> live execution/SSE/cancellation presentation
  Project Export         -> exported book payload/file
```

Run/Step 只保存指针和安全摘要，不复制正文、大纲、Prompt、reasoning 或质量原始事实。

## 3. Persistence schema

### 3.1 `novel_autopilot_runs`

核心列：

- identity/scope: `id`, `project_id`, `user_id`, `schema_version`;
- lifecycle: `status`, `current_phase`, `current_step`, `active_scope_key`;
- cursor: `current_chapter_id`, `current_chapter_number`, `total_chapters`, `completed_chapters`, `failed_chapters`, `pending_rewrites`, `total_word_count`;
- configuration: `execution_scope`, `human_gate_mode`, `gate_interval`, `config_snapshot`;
- budget: `max_chapters`, `max_tokens`, `max_estimated_cost`, `max_runtime_seconds`, `used_tokens`, `estimated_cost`, `started_at`;
- safety: `epoch`, `version`, `consecutive_provider_failures`, `consecutive_quality_failures`, `last_error_code`;
- control/result: `guidance_digest`, `active_background_task_id`, `final_export_ref`, `paused_at`, `completed_at`, `created_at`, `updated_at`.

`active_scope_key` 在活动状态下等于 `project_id`，终态为 NULL，并建立 unique index；由此在 PostgreSQL/SQLite 下都能阻止同项目双 Run。`version` 用于 CAS 更新，`epoch` 用于废弃旧 attempt 的迟到结果。

### 3.2 `novel_autopilot_step_runs`

核心列：

- `id`, `run_id`, `step_key`, `step_type`, `phase`, `chapter_id`, `chapter_number`;
- `attempt`, `run_epoch`, `status`, `background_task_id`;
- `input_digest`, `result_digest`, `quality_decision`, `error_code`;
- `started_at`, `completed_at`, `created_at`, `updated_at`.

约束：

- unique `(run_id, step_key, attempt)`；
- index `(run_id, status, created_at)`；
- FK run `ON DELETE CASCADE`，chapter `ON DELETE SET NULL`；
- 只有 `run_epoch == current run.epoch` 且 step 为 active 时才允许提交结果。

### 3.3 Migration integration

新增 PostgreSQL Alembic revision，并同步：

- `backend/migrator_app/models/` 冻结模型注册；
- `backend-rs/src/models/` SeaORM entities；
- `backend-rs/src/services/schema_migration_metadata_service.rs` catalog/head/DDL tests；
- SQLite test schema 使用 SeaORM entity 创建表。

不使用 runtime schema sync 修复生产表。

## 4. State machine

### 4.1 Run states

```text
queued -> running
running -> paused | waiting_human | completed | failed | cancelled
paused -> queued | cancelled
waiting_human -> queued | cancelled | failed
```

`pause` 是 cooperative：立即增加 epoch、设置 paused，阻止启动下一步；当前模型请求可以结束，但其结果只有在提交点重新校验 epoch/status 后才可写业务数据。`cancel` 同样增加 epoch，并触发现有 Cooperative Cancellation。

### 4.2 Phase router

```text
validate
-> foundation
-> world_building
-> career_design
-> character_design
-> organization_design
-> outline
-> chapter_loop
-> book_review
-> book_polish
-> export
-> completed
```

Router 是纯决策函数：读取 Run snapshot + 业务事实，返回下一个 `AutopilotStepPlan`；副作用由 Step Executor 执行。这样可单测所有路由分支并保证服务重启可 replay decision，而不是 replay side effects。

## 5. Service architecture

建议新增模块：

```text
backend-rs/src/services/novel_autopilot/
  mod.rs
  types.rs                 # enums/config/read models
  repository.rs            # Run/Step CAS queries
  router.rs                # pure next-step decision
  coordinator.rs           # one-step-at-a-time durable loop
  budget.rs                # before/after step guards
  recovery.rs              # startup scan and resume
  adapters/
    planning.rs            # wizard/world/career/character/org/outline
    chapter.rs             # chapter generation + quality closure
    completion.rs          # review/polish/export
```

API 建议：

```text
POST   /api/projects/:project_id/novel-autopilot-runs
GET    /api/projects/:project_id/novel-autopilot-runs
GET    /api/projects/:project_id/novel-autopilot-runs/:run_id
POST   /api/projects/:project_id/novel-autopilot-runs/:run_id/pause
POST   /api/projects/:project_id/novel-autopilot-runs/:run_id/resume
POST   /api/projects/:project_id/novel-autopilot-runs/:run_id/cancel
POST   /api/projects/:project_id/novel-autopilot-runs/:run_id/decision
GET    /api/projects/:project_id/novel-autopilot-runs/:run_id/steps
GET    /api/projects/:project_id/novel-autopilot-runs/:run_id/stream
```

API DTO 使用显式 allowlist。Run create 保存配置快照后创建 `novel_book_autopilot` Background Task；任务 executor 每次只执行一个 durable step，提交后重新排队/继续，避免一个不可恢复 Future 独占整本书生命周期。

## 6. Step execution protocol

每个步骤统一执行：

1. `load_owned_run` + CAS claim，写入 Step `running`；
2. 检查 run status、epoch、budget、workflow/business preconditions；
3. 调用现有业务 service；
4. 在业务提交前执行 cooperative cancellation + epoch/version recheck；
5. 业务写入与 Step/Run 游标更新尽可能在同一数据库事务；若现有生成服务已自行提交，则以其业务事实做幂等 reconciliation；
6. 写 Step terminal safe summary；
7. Router 计算下一步，进入人工门、终态或排队下一 step。

`step_key` 必须稳定，例如：

```text
planning:world_building
planning:outline
chapter:0001:generate
chapter:0001:analyze
chapter:0001:repair
completion:book_review
completion:book_polish
completion:export
```

恢复时先检查业务事实是否已经存在；存在则将遗留 step reconcile 为 completed，绝不重复覆盖。

## 7. Adapter strategy

### 7.1 Planning adapters

优先调用 `wizard_service`、`ProjectService`、outline execution service 等现有 Rust owner。若逻辑仍封装在 API handler 中，只做最小 route-to-service 抽取：handler 与 Autopilot 共用同一个 service function，禁止通过 localhost HTTP 自调用。

跳过规则：

- 现有人工资料非空且通过基本校验 -> skip；
- `regenerate_existing == false`（默认） -> 不覆盖；
- 明确开启重建 -> 新 step attempt，仍需业务 owner 自己写入。

### 7.2 Chapter adapter

复用当前章节生成 runtime 的 Generation Contract、Story Packet、质量上下文、checkpoint、retry routing 和 terminal semantics。Durable Orchestrator 不重新实现 Prompt 或质量公式。

首选按单章 durable step 驱动，以便每章之间检查暂停/预算/人工门；可以复用批量生成内部 owner，但不能启动一个覆盖整本书且无法逐章接管的黑盒任务。

### 7.3 Completion adapter

- Book review：基于现有章节分析/plot analysis 聚合，写安全 review summary；
- Polish：复用 `execute_polish_text_task` / `execute_polish_batch_task` 的 service owner，并限制返修章节；
- Workflow：通过 `NovelWorkflowService` CAS 推进 reviewing/polishing/completed；
- Export：复用 projects export context/serialization，保存文件引用或稳定 export descriptor，不复制导出正文到 Run。

## 8. Budget and human gates

`budget.rs` 在每个模型步骤前后执行。使用现有 execution audit / generation history / provider usage 能取得的 Token 与模型元数据；缺少精确 usage 时使用统一估算函数并标注 `estimated`，不能默认为零。

人工门由 Router 决定：

- every_chapter：每章 accept 后等待；
- every_n_chapters：完成数整除 interval 时等待；
- every_volume：卷末等待；
- high_risk_only：重试耗尽、连续低质量、预算接近阈值、外部配置变化时等待；
- fully_automatic：只有硬停止条件才等待/失败。

Decision API：`accept`、`retry`、`repair`、`skip_optional`、`stop`，并允许附带后续指导。指导原文只进入受保护配置/intent 边界；Run read model 仅返回 `has_guidance` 和 digest。

## 9. Background task and recovery

- 新 task type：`novel_book_autopilot`；旧 `novel_autopilot` 不变。
- 在 recovery policy registry 中注册为 resumable/reconstructable 类型。
- startup reconciliation 扫描活动 Run：
  - 若有合法 active child task，保持；
  - 若 task 丢失或 terminal 但 Run 仍 active，按最后 committed step 重建；
  - running step 无法证明业务提交时标记 interrupted，再由 router 幂等重试；
  - terminal Run 不恢复。
- SSE 使用 Background Task 通道发布阶段/章节/预算/质量和现有模型 output/reasoning；持久 API 是刷新后的事实源。

## 10. Frontend design

新增 feature：

```text
frontend/src/features/novel-autopilot/
  api/types.ts
  model/reducer.ts
  hooks/useNovelAutopilotRun.ts
  ui/NovelAutopilotWorkbench.tsx
  ui/RunCreateForm.tsx
  ui/RunProgress.tsx
  ui/RunControls.tsx
  ui/RunStepTimeline.tsx
```

项目路由增加 `/project/:projectId/autopilot`，ProjectDetail 创作管理菜单增加“自动创作”。

- server state 通过 service module 获取；
- 高频 stream 仅保存在 Hook/组件内存，不写 Zustand 持久 store；
- Run read model 用 reducer 统一处理状态映射；
- 复用 `useBackgroundTaskOutputStream` 和 ModelOutputPanel 的显示开关；
- Runtime Metrics 面板保持 sticky/fixed existing behavior。

## 11. Compatibility and rollout

- 第一阶段可通过 UI/配置开关隐藏入口，但 API 和数据模型按最终契约实现。
- 旧 R7 history panel 继续只显示单次工具审计，不混入 durable step history。
- 迁移 downgrade 只删除新增 Run/Step 表，不修改现有章节/项目表。
- 出现问题时可停止创建新 Run；已存在 Run 可暂停，现有手工创作功能保持可用。

## 12. Risks

- 现有部分生成入口仍偏 route-centric：需小范围抽取 service，避免内部 HTTP 调用。
- 模型调用可能先完成而数据库提交失败：必须以业务事实 reconciliation 和稳定 step key 保证幂等。
- 精确 Token/成本不一定所有 Provider 都返回：必须统一标记估算值并采用保守限制。
- 全书润色成本很高：默认只处理 review 标记章节，整书润色需显式预算允许。
- Windows Rust 铪接器可能出现 `LNK1318 PDB LIMIT`：验证命令采用 `-j 1` 和 focused tests，仍失败时记录环境证据但不能用其掩盖编译错误。
