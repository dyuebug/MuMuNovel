 # Rust Schema Ownership 审计（2026-05-17）

 ## 1. 目标

 本文档用于承接 `docs/architecture/rust-strangler-refactor-plan-2026-05-17.zh-CN.md` 中的 Phase 2，明确当前 strangler 部署下：

 - 谁是 **schema mutation owner**
 - Rust `backend-rs/src/models/` 声明了哪些表
 - 这些表当前是否已经由 Python Alembic 迁移覆盖
 - 当前还缺失哪些“Rust 接管 schema”前的关键治理动作

 本文档聚焦 **shared DB + strangler deploy** 语境，不讨论未来完全去 Python 化之后的最终态。

 ---

 ## 2. 当前结论

 ### 2.1 当前 schema owner

 目前的 schema owner 仍然是 **Python Alembic**，而不是 Rust。

 证据：

 - `backend/scripts/run_migrations.sh` 通过 `python scripts/migrate.py upgrade head` 执行迁移。
 - `docker-compose.strangler.yml` 新增 `db-migrator`，其运行命令是 `/app/run_migrations.sh`。
 - `backend/scripts/entrypoint.sh` 已支持 `RUN_DB_MIGRATIONS_ON_STARTUP=false`，说明 Python app startup 不再是唯一迁移入口，但迁移能力本身仍来自 Python。

 ### 2.2 当前 Rust 的位置

 当前 Rust 已经：

 - 通过 `backend-rs/src/models/mod.rs` 声明了一批 SeaORM entities。
 - 在 `backend-rs/src/main.rs` 中移除了 strangler 部署里的自动建表逻辑。
 - 在 `backend-rs/src/config.rs` 中把 `ENABLE_STARTUP_SCHEMA_SYNC` 默认值调整为 `false`。

 这说明 Rust 的当前定位是：

 > **消费既有 schema，而不是在 strangler 部署里主动塑造 schema。**

 ---

 ## 3. 审计范围与证据来源

 ### 3.1 Rust 侧

 - `backend-rs/src/models/mod.rs`
 - `backend-rs/src/models/*.rs`
 - `backend-rs/src/main.rs`
 - `backend-rs/src/config.rs`

 ### 3.2 Python / Schema 侧

 - `backend/scripts/migrate.py`
 - `backend/scripts/run_migrations.sh`
 - `backend/alembic/postgres/versions/*.py`

 ### 3.3 Strangler deploy 侧

 - `docker-compose.strangler.yml`
 - `deploy-strangler.ps1`

 ---

 ## 4. 当前 schema ownership 状态

 ## 4.1 部署链路

 当前 strangler 部署的 schema 相关顺序已经变为：

 1. `postgres` 启动
 2. `db-migrator` 运行 `/app/run_migrations.sh`
 3. Alembic 升级到 `head`
 4. `python-backend` / `rust-backend` 再启动

 这比旧模式更安全，因为：

 - Python app startup 不再必须隐式改 schema
 - Rust app startup 不再建表
 - schema mutation 被收敛到独立 migration step

 ## 4.2 当前仍未完成的部分

 目前仍未建立：

 - Rust 自有 migration pipeline
 - Rust 对数据库结构的正式 ownership
 - Rust model 与 Alembic schema 的自动一致性校验

 因此 Phase 2 目前完成的是：

 > **把 schema 变更从后端启动副作用中剥离出来**，而不是 **把 schema 所有权迁给 Rust**。

 ---

 ## 5. Rust model 覆盖矩阵

 说明：

 - “Alembic 覆盖”表示当前 PostgreSQL 迁移目录中存在创建该表或显式后续变更的证据。
 - “状态”用于表达当前 Rust entity 与 Python Alembic schema 的关系，不代表业务逻辑完整度。

 | Rust entity | 表名 | Alembic 覆盖 | 证据 | 状态 |
 |---|---|---|---|---|
 | `user` | `users` | 是 | 初始迁移创建 | 基础表，已覆盖 |
 | `user_password` | `user_passwords` | 是 | 初始迁移创建 | 基础表，已覆盖 |
 | `project` | `projects` | 是 | 初始迁移创建 + 20260322/20260323 后续字段变更 | 已覆盖，仍由 Python 管理演进 |
 | `settings` | `settings` | 是 | 初始迁移创建 + `20260222_add_api_compatibility_fields.py` | 已覆盖 |
 | `mcp_plugin` | `mcp_plugins` | 是 | 初始迁移创建 | 已覆盖 |
 | `prompt_template` | `prompt_templates` | 是 | 初始迁移创建 | 已覆盖 |
 | `relationship_type` | `relationship_types` | 是 | 初始迁移创建 | 已覆盖 |
 | `career` | `careers` | 是 | 初始迁移创建 | 已覆盖 |
 | `organization` | `organizations` | 是 | 初始迁移创建 | 已覆盖 |
 | `outline` | `outlines` | 是 | 初始迁移创建 | 已覆盖 |
 | `writing_style` | `writing_styles` | 是 | 初始迁移创建 + 预置/更新迁移 | 已覆盖 |
 | `chapter` | `chapters` | 是 | 初始迁移创建 | 已覆盖 |
 | `character` | `characters` | 是 | 初始迁移创建 + `20260212...` 状态字段迁移 | 已覆盖 |
 | `project_default_style` | `project_default_styles` | 是 | 初始迁移创建 | 已覆盖 |
 | `character_career` | `character_careers` | 是 | 初始迁移创建 | 已覆盖 |
 | `relationship` | `character_relationships` | 是 | 初始迁移创建 | 已覆盖 |
 | `batch_generation_task` | `batch_generation_tasks` | 是 | 初始迁移创建 | 已覆盖 |
 | `analysis_task` | `analysis_tasks` | 是 | 初始迁移创建 | 已覆盖 |
 | `generation_history` | `generation_history` | 是 | 初始迁移创建 | 已覆盖 |
 | `plot_analysis` | `plot_analysis` | 是 | 初始迁移创建 | 已覆盖 |
 | `regeneration_task` | `regeneration_tasks` | 是 | 初始迁移创建 | 已覆盖 |
 | `story_memory` | `story_memories` | 是 | 初始迁移创建 | 已覆盖 |
| `organization_member` | `organization_members` | 是 | 初始迁移创建 | 已覆盖，但字段级一致性需单独审计 |
 | `foreshadow` | `foreshadows` | 是 | `20260119_1729_6a73f37e9adb_添加伏笔管理表.py` | 后续迁移补建 |
 | `prompt_submission` | `prompt_submissions` | 是 | `20260127_1404_421237957b27_添加提示词工坊相关表结构.py` | 后续迁移补建 |
 | `prompt_workshop_item` | `prompt_workshop_items` | 是 | `20260127_1404_421237957b27_添加提示词工坊相关表结构.py` | 后续迁移补建 |
 | `prompt_workshop_like` | `prompt_workshop_likes` | 是 | `20260127_1404_421237957b27_添加提示词工坊相关表结构.py` | 后续迁移补建 |
 | `chapter_draft_attempt` | `chapter_draft_attempts` | 是 | `20260325_0900_batch_runtime_store.py` | 后续迁移补建 |
 | `batch_generation_snapshot` | `batch_generation_snapshots` | 是 | `20260325_0900_batch_runtime_store.py` + `20260325_2210_batch_workflow_runtime_state.py` | 后续迁移补建 |

 ---

 ## 6. 重点发现

 ## 6.1 大部分 Rust entities 已经能在 Alembic 中找到表级证据

 这说明：

 - Rust 不是在完全脱离 Python schema 的情况下“猜表结构”
 - shared DB strangler 至少在“已有实体大多已有表”这一点上是可持续的

 但这不代表：

 - Rust entity 字段与当前 Alembic head 完全等价
 - 关系、索引、约束、nullable、默认值等细节已经全量核实

 因此目前可以称为：

 > **表级覆盖大体成立，字段级与约束级一致性审计仍未完成。**

## 6.2 `organization_members` 已有表级覆盖，但仍是字段级高风险项

补充核对后，当前证据链里：

 - Rust entity：`backend-rs/src/models/organization_member.rs`
 - Python ORM：`backend/app/models/relationship.py`
- PostgreSQL 初始迁移 `20251226_1008_ee0a189f1532_初始数据库结构.py` 已显式 `create_table('organization_members', ...)`

这意味着它应被列为：

- **字段级一致性重点审计项**
- 在 Rust 进一步放大组织域/关系域写路径前应优先核对 nullable/default 与 Rust model 的一致性

 ## 6.3 任务/运行时表已经开始偏离“初始库结构”

 特别是：

 - `chapter_draft_attempts`
 - `batch_generation_snapshots`
 - `workflow_runtime_state`

 这说明 schema 演进热点已经从基础 CRUD 表，转向：

 - 批量生成运行时
 - 候选稿/草稿尝试
 - 工作流恢复状态

 这类表也是未来 Rust 真正接管 migration 时最适合优先切入的区域，因为：

 - 它们更靠近 Rust 正在重构的任务/工作流域
 - 对共享前台 CRUD 的耦合相对更可控

 ---

 ## 7. 当前风险评估

 ## 7.1 当前最大的风险不是“没表”，而是“字段/约束漂移不可见”

 目前最现实的问题已经不再是：

 - Rust entity 对应不到数据库表

 而更可能是：

 - Python Alembic 新增了字段，Rust model 还没同步
 - Rust model 声明了字段，但默认值/nullable/约束理解不一致
 - 后续某次 migration 修改了索引/唯一约束，但 Rust 侧没有显式意识

 ## 7.2 `ENABLE_STARTUP_SCHEMA_SYNC=false` 是必要的，但不是终点

 现在关闭 Rust 启动建表是对的，但它只解决了：

 - Rust 不再偷偷改 schema

 它没有解决：

 - Rust 如何正式接管 migration
 - Rust 如何验证自己消费的 schema 是兼容的
 - Rust 如何对模型变更建立审计纪律

 ---

 ## 8. 建议的下一步

 ## 8.1 先补字段级一致性审计，而不是立刻做 Rust migration

 下一步建议不是马上引入 SeaORM migration 接管全部表，而是先做：

 1. **高风险表字段比对**：
    - `batch_generation_tasks`
    - `batch_generation_snapshots`
    - `chapter_draft_attempts`
    - `analysis_tasks`
    - `regeneration_tasks`
    - `projects`
    - `settings`
    - `characters`

 2. **待确认表专项核对**：
    - `organization_members`

 ## 8.2 先选“运行时热点表”作为 Rust migration 接管试点

 如果后续要让 Rust 逐步拥有 schema ownership，建议优先从这些表开始：

 - `batch_generation_snapshots`
 - `chapter_draft_attempts`
 - 其他与 Rust 任务/工作流域强绑定的新运行时表

 原因：

 - 它们已经是后续迁移新增热点
 - 对前台通用 CRUD 表的历史兼容压力更小
 - 更适合作为 expand/switch/contract 试点

 ## 8.3 给 Rust model 审计建立固定制度

 建议后续新增一份固定检查清单：

 - 新增 Rust entity 时，必须标注 Alembic 覆盖状态
 - 新增 Alembic migration 时，若涉及 Rust 已消费表，必须同步检查 Rust model
 - Phase 3 开始前，章节域与任务域相关表必须完成字段级一致性审计

 ---

 ## 9. 结论

 当前 strangler 阶段可以明确下结论：

 1. **Schema owner 仍然是 Python Alembic。**
 2. **Rust 已停止在部署时自动建表，这是正确的阶段性收口。**
 3. **Rust entities 与 Alembic 表级覆盖大体匹配，但字段级一致性尚未系统审计。**
4. **`organization_members` 已有 Alembic 表级覆盖，但字段级一致性风险较高。**
5. **下一步应该先做字段级审计与运行时热点表试点，而不是立刻全量迁移 schema ownership。**

---

## 10. 2026-05-31 收口补充

本轮又补了一条与 schema owner 直接相关的启动期约束：

1. `backend-rs/src/config.rs` 现在会在 non-development 模式下拒绝
   `ENABLE_STARTUP_SCHEMA_SYNC=true`。
2. development 模式下即使显式设置该标志，也只会告警并归一化为 `false`。
3. `backend-rs/src/main.rs` 不再保留“看到该标志仅 warning 后继续启动”的
   软处理路径。

这意味着当前 shared-db strangler 的 schema ownership 不仅在部署链路上
由 `db-migrator -> Python Alembic` 显式承担，在 Rust 启动配置层也已经有了
对应的防漂移约束。

这仍然 **不是** Rust migration ownership 的接管完成；它只是进一步明确：

- Rust 当前角色是消费既有 schema
- Python Alembic 仍然是唯一被允许的显式 schema mutation owner
- 后续若要推进 Rust migration 试点，应以新的独立 migration pipeline
  和更细粒度字段审计为前提，而不是重新打开启动期 schema sync
