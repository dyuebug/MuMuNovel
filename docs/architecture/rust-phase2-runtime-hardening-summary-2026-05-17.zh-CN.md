 # Rust Phase 2 运行时收口总结（2026-05-17）

 ## 1. 阶段目标

 本阶段目标不是继续扩张 Rust 端点覆盖率，而是把 strangler 迁移推进到一个更稳定的治理状态：

 - 部署链路中的 schema 变更从应用启动副作用中剥离出来
 - shared DB 下的高风险运行时表开始形成一致的默认值与非空语义
 - Rust backend 不再通过启动建表掩盖 schema ownership 问题
 - 后续 Phase 3 进入章节域/任务域边界重构前，先把运行时基础设施收口到可验证状态

 ---

 ## 2. 本阶段完成的主要成果

 ### 2.1 迁移控制面完成显式化

 已完成：

 - 新增 gateway probe manifest：`deploy/strangler-gateway-probes.json`
 - 新增结构化 smoke：`backend/tools/run_strangler_gateway_smoke.py`
 - `deploy-strangler.ps1` 集成 through-gateway smoke，并在失败时保留诊断

 结果：

 - strangler deploy 不再只依赖 Rust `/health`
 - Python fallback 与 Rust 路径至少具备最小 through-gateway 验证能力
 - smoke 结果可落盘到 `tmp/smoke/`

 ### 2.2 schema mutation 已从 app startup 中剥离

 已完成：

 - 新增显式迁移脚本：`backend/scripts/run_migrations.sh`
 - `docker-compose.strangler.yml` 新增 `db-migrator`
 - `backend/scripts/entrypoint.sh` 支持 `RUN_DB_MIGRATIONS_ON_STARTUP=false`
 - `deploy-strangler.ps1` 在启动 Python / Rust 前显式执行 migration step

 结果：

 - Python backend 不再是“启动时顺手迁移 schema”
 - Rust backend 不再依赖 Python 迁移是否刚好跑完
 - strangler 部署顺序从“容器启动 + 隐式变更”变成“先迁移、后启动”

 ### 2.3 Rust startup schema sync 已停止承担部署期 ownership

 已完成：

 - `backend-rs/src/main.rs` 移除 startup 自动建表逻辑
 - `backend-rs/src/config.rs` 将 `ENABLE_STARTUP_SCHEMA_SYNC` 默认值改为 `false`
 - `docker-compose.strangler.yml` 显式给 Rust 服务设置 `ENABLE_STARTUP_SCHEMA_SYNC=false`

 结果：

 - Rust 不再在 strangler 部署里偷偷修表/建表
 - schema owner 边界更清晰：当前仍是 Python Alembic，而不是 Rust runtime

 ---

 ## 3. 已完成的审计资产

 本阶段不仅做了代码修改，也沉淀了三份关键文档：

 1. `docs/architecture/rust-strangler-refactor-plan-2026-05-17.zh-CN.md`
    - 总体规划与 Phase 划分
 2. `docs/architecture/rust-schema-ownership-audit-2026-05-17.zh-CN.md`
    - schema owner、Rust model 覆盖矩阵、shared DB 风险结论
 3. `docs/architecture/rust-runtime-table-field-matrix-2026-05-17.zh-CN.md`
    - 第一批运行时热点表字段级差异矩阵与修复状态

 这三份文档的价值在于：

 - 后续推进不再依赖“上下文记忆”
 - 风险点有明确书面锚点
 - 可以把“哪些已经收口，哪些尚未收口”讲清楚

 ---

 ## 4. 本阶段已经开始收口的运行时热点表

 本阶段优先处理了三张最容易在 shared-DB strangler 下产生语义漂移的运行时表：

 ### 4.1 `analysis_tasks`

 已完成：

 - Python ORM 收紧 `status/progress`
 - Postgres / SQLite 双迁移回填 `NULL` 并收紧为非空 + server default
 - Rust model 与目标语义保持一致（`String` / `i32`）

 当前状态：

 - 仓库层第一轮已收口
 - 主要剩余风险转移到“目标环境是否已真实执行迁移”

 ### 4.2 `batch_generation_tasks`

 已完成：

 - Python ORM 收紧：
   - `target_word_count`
   - `enable_analysis`
   - `status`
   - `total_chapters`
   - `completed_chapters`
   - `failed_chapters`
   - `current_retry_count`
   - `max_retries`
 - Postgres / SQLite 双迁移回填 `NULL` 并补 server default / 非空约束
 - Rust model 已同步收紧核心字段
 - `backend-rs/src/api/chapter_batch_generation.rs` 的关键读写链已适配非可空语义

 当前状态：

 - 第一轮默认值语义收口基本完成
 - 剩余问题主要是 API 输入层仍保留兼容性 `Option` 参数，以及迁移落地执行问题

### 4.3 `regeneration_tasks`

 已完成：

 - Python ORM 收紧：
   - `target_word_count`
   - `status`
   - `progress`
   - `version_number`
 - Postgres / SQLite 双迁移已新增
 - Rust model 已同步收紧为非 `Option<T>`

 当前状态：

 - 仓库层第一轮已收口
- Rust API 层显式消费点较少，目前主要剩余问题仍是迁移执行是否已落到目标环境

### 4.4 `projects`（中风险表安全切片）

已完成：

- Python ORM 收紧：
  - `target_words`
  - `current_words`
  - `status`
  - `wizard_status`
  - `wizard_step`
  - `outline_mode`
  - `character_count`
- Postgres / SQLite 双迁移已新增

当前状态：

- 已完成一组相对安全的默认值/非空语义收口
- 暂未扩大到 `projects` 上更高耦合的流程偏好字段（如 `default_*` 系列）
- 这是从运行时热点表过渡到中风险共享表的第一刀，范围仍保持克制

### 4.5 `settings`（第二刀）

已完成：

- Python ORM 继续收紧：
  - `provider_type`
  - `fallback_strategy`
- 这两个字段的 Alembic 增量迁移原本已具备 `server_default`，本轮重点是把 ORM 语义补齐到与 DB / Rust 一致

当前状态：

- `settings` 的核心默认值字段已形成更一致的 ORM / Alembic / Rust 语义
- 该表当前不再适合作为优先收口对象继续扩大范围，后续可延后处理 `api_key` / `api_base_url` / `preferences` 这类允许为空的配置字段

---

 ## 5. 当前阶段风险已经发生了哪些转移

 本阶段最大的价值，不只是“修了几个字段”，而是让风险类型发生了迁移。

 ### 5.1 之前的主要风险

 - Python 启动时隐式迁移 schema
 - Rust 启动时自动建表
 - 运行时表默认值只存在于 ORM 层，不存在于数据库层
 - Rust model / Python ORM / Alembic 三方语义不一致

 ### 5.2 当前的主要风险

 现在更主要的风险已经转向：

 - 某些目标环境是否真的跑到了最新 Alembic head
 - 历史数据是否已经通过 migration 回填了 `NULL`
 - 剩余未处理的中等复杂度表是否会出现同类语义漂移

 这是一个积极信号，因为它说明：

 > 风险已经从“代码定义层和启动期副作用层”的混乱，转向“部署执行层和剩余增量表治理”的可管理问题。

 ---

 ## 6. 当前仍未完成的事项

 ### 6.1 schema owner 仍然不是 Rust

 虽然 Rust 已经停止启动建表，但当前 schema owner 仍然是：

 - Python Alembic

 也就是说：

 - Phase 2 解决的是 ownership 边界清晰化
 - 不是 ownership 真正切换

 ### 6.2 Rust 自有 migration pipeline 仍未建立

 当前还没有：

 - SeaORM migration pipeline
 - Rust 侧 schema drift 检查机制
 - Rust model 与 Alembic 的自动一致性校验

 ### 6.3 仍有未进入收口批次的中等复杂度表

 当前还未纳入本轮收口重点的典型表包括：

 - `projects`
 - `settings`
 - `characters`
 - 其他共享前台 CRUD 影响面较大的表

 这些表不适合在当前阶段草率扩大范围，因为它们的前端耦合面更广，收口风险更高。

 ---

 ## 7. 为什么本阶段适合在这里形成里程碑

 到当前为止，Phase 2 已经形成一个比较完整的小闭环：

 1. 迁移控制面显式化
 2. schema startup 副作用收敛
 3. schema ownership 审计落档
 4. 运行时热点表字段矩阵建立
 5. 三张高风险运行时表开始完成第一轮默认值/非空语义收口

 这意味着现在非常适合把阶段状态固化为：

 > **Phase 2 已经把 strangler 从“隐式迁移 + 启动建表 + 语义漂移不可见”推进到了“显式迁移 + 边界清晰 + 热点运行时表开始收口”的状态。**

 ---

 ## 8. 下一阶段建议

 ### 8.1 短期建议

 下一步不要急着继续扩大 Rust 接口覆盖面，优先建议：

 1. 在真实目标环境执行最新迁移并做落地验证
 2. 继续围绕剩余中风险表做字段级收口，而不是扩新业务面
 3. 对 `deploy-strangler.ps1` 的 migration + smoke 证据链再做一次实跑验证

 ### 8.2 中期建议

 在进入 Phase 3 之前，建议满足以下条件：

 - 关键运行时热点表已经完成第一轮收口
 - 迁移执行链稳定
 - Rust 不再依赖 runtime schema 修补
 - 文档和证据链可支持后续更高风险的章节域边界重构

 ### 8.3 Phase 3 进入条件

 只有当上述条件满足后，才建议把重心从 schema/runtime hardening 转向：

 - `chapter_*` 域的边界拆分
 - workflow / service / compat adapter 进一步下沉
 - Rust 内部高复杂度域的结构性收缩

 ---

 ## 9. 结论

 本阶段可以给出如下阶段性结论：

 1. **strangler 部署的 schema mutation 已从 app startup 副作用中剥离。**
 2. **Rust runtime 已停止承担部署期自动建表职责。**
 3. **`analysis_tasks`、`batch_generation_tasks`、`regeneration_tasks` 三张高风险运行时表已经开始完成第一轮默认值语义收口。**
 4. **当前最大的剩余风险，已从“代码定义混乱”转向“迁移是否在真实环境已执行”和“其余表是否继续存在同类漂移”。**
 5. **这使得 Phase 2 足以形成一个可暂停、可交接、可继续推进的运行时收口里程碑。**
