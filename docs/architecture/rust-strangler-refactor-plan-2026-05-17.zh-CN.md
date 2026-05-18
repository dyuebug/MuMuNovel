 # Rust Strangler 重构规划（2026-05-17）

 ## 1. 目标

 本规划文档用于约束 MuMuNovel 从 Python 后端向 `backend-rs/` 的 Rust 后端迁移过程，避免继续以“多迁几个接口”为主要进度指标，而忽略迁移控制面、数据库所有权与内部边界稳定性。

 当前目标不是一次性替换 Python，而是把现有 Shared-DB + Nginx 路由分流的 Strangler Fig 迁移，推进到“可持续、可验证、可回滚”的状态。

 ---

 ## 2. Phase 0：文档与真实代码发现

 ### 2.1 已核对的资料与代码

 #### 迁移控制 / 部署

 - `deploy-strangler.bat`
 - `deploy-strangler.ps1`
 - `docker-compose.strangler.yml`
 - `deploy/nginx/mumunovel-docker.conf`
 - `deploy/nginx/mumunovel.conf`
 - `deploy/nginx/conf.d/proxy-common.conf`
 - `deploy/nginx/conf.d/sse-common.conf`
 - `Dockerfile`
 - `backend/scripts/entrypoint.sh`

 #### Rust 后端骨架

 - `backend-rs/src/main.rs`
 - `backend-rs/src/api/router.rs`
 - `backend-rs/src/config.rs`
 - `backend-rs/src/db/connection.rs`
 - `backend-rs/src/api/health.rs`
 - `backend-rs/src/models/mod.rs`
 - `backend-rs/Cargo.toml`

 #### 现有文档 / 约定

 - `docs/13-project-structure-governance.zh-CN.md`
 - `docs/14-auth-login-regression-checklist.zh-CN.md`
 - `docs/architecture/chapter-api-gateway-seams.zh-CN.md`
 - `docs/architecture/backend-refactor-milestone-summary-2026-04-21.zh-CN.md`
 - `backend/tools/run_settings_probe_smoke.py`
 - `backend/tools/run_batch_terminal_status_smoke.py`

 ### 2.2 当前真实现状摘要

 1. 当前迁移模式是 **Nginx 路径级分流 + Python/Rust 共享 PostgreSQL**。
 2. `deploy-strangler.ps1` 已具备基本部署编排，但部署成功判断仍然偏向 Rust 健康，不足以证明双栈链路整体健康。
 3. Python 仍然通过 `backend/scripts/entrypoint.sh` 在容器启动时执行 Alembic 迁移，因此 **Schema owner 仍然是 Python**。
 4. Rust 端 `backend-rs/src/main.rs` 仍带有 `create_table_if_not_exists!` 启动建表逻辑，说明 Rust 还没有进入正式 migration 驱动阶段。
 5. Rust 端 `backend-rs/src/api/router.rs` 已能承担大量路由，但 CORS / 静态托管 / Auth / API 聚合仍偏向“兼容优先”。
 6. `backend-rs/src/api/health.rs` 和 `backend-rs/src/db/connection.rs` 中仍有占位/过渡实现，例如简化的 session stats 和 `sqlite::memory:` 回退。

 ---

 ## 3. Allowed APIs / 模式清单

 下面这些模式已经在仓库中存在，后续规划和实施必须优先复用，而不是临时发明新约定。

 ### 3.1 部署编排允许复用的模式

 来源：`deploy-strangler.ps1`

 - `Wait-ContainerHealthy`：容器健康等待与诊断回收
 - `Invoke-LoggedCommand`：统一日志记录与命令执行
 - 通过 `Invoke-WebRequest` 做 gateway 健康探测
 - 部署日志输出到 `logs/ops/`

 ### 3.2 smoke / 回归脚本允许复用的模式

 来源：

 - `backend/tools/run_settings_probe_smoke.py`
 - `backend/tools/run_batch_terminal_status_smoke.py`
 - `docs/13-project-structure-governance.zh-CN.md`

 可复用模式：

 - 结果输出到 `tmp/smoke/`
 - 使用结构化 JSON 输出结果
 - 失败时抛出明确错误，避免静默失败
 - 运行结果可以作为部署后验证证据保存

 ### 3.3 章节路由继续拆分时允许复用的模式

 来源：`docs/architecture/chapter-api-gateway-seams.zh-CN.md`

 可复用 seam：

 - `route -> workflow`
 - `route -> query service`
 - `route -> compat response adapter`
 - `route -> access/request context helper`

 ---

 ## 4. 明确禁止的反模式

 1. **继续扩大 Rust 启动自动建表范围**，而不建立独立 migration 路径。
 2. **继续靠手工编辑多份 Nginx 配置维护路由所有权**，而没有单一事实来源。
 3. **新增部署验证只校验 Rust `/health`**，不校验 Python fallback 链路。
 4. **在 route handler 中继续堆 compat/legacy 序列化逻辑**，不做显式边界隔离。
 5. **继续给章节域新增行为而不先拆路由/工作流边界**。
 6. **继续新增第二套任务语义**，而不统一 status/checkpoint/恢复语义。

 ---

 ## 5. 分阶段规划

 ## Phase 1：迁移控制面硬化（本轮优先实施）

 ### 目标

 让 strangler 部署从“Rust 健康即可视为成功”升级为“网关层双栈关键路径可验证”。

 ### 要实施的内容

 1. 新增 **strangler 路由/探针清单** 作为单一事实来源的第一步。
 2. 新增 **gateway smoke 验证**，至少覆盖：
    - Rust health
    - Rust readiness
    - Python fallback reachability
 3. 将 smoke 结果写入 `tmp/smoke/`，符合目录治理约定。
 4. 在 `deploy-strangler.ps1` 中集成该 smoke，部署失败时直接中断并保留诊断信息。

 ### 参考模式

 - `deploy-strangler.ps1`
 - `docs/13-project-structure-governance.zh-CN.md`
 - `backend/tools/run_settings_probe_smoke.py`
 - `backend/tools/run_batch_terminal_status_smoke.py`

 ### 验证清单

 - manifest 能被脚本成功读取
 - smoke JSON 能输出到 `tmp/smoke/`
 - 缺失/异常状态码时部署脚本能显式失败
 - 部署日志中能看到每个 probe 的 owner/path/status

 ---

 ## Phase 2：Schema 所有权和迁移流程解耦

 ### 目标

 结束“Python 容器启动即迁移 schema + Rust 启动即自动建表”的双重过渡状态。

 ### 要实施的内容

 1. 从 Python app startup 中剥离 Alembic 迁移，做成独立 migration step。
 2. 停止 Rust 在 `main.rs` 中继续承担启动建表职责。
 3. 审计 `backend-rs/src/models/mod.rs` 中已声明实体与真实表初始化/迁移覆盖的一致性。
 4. 建立 expand / switch / contract 的 shared-DB 迁移纪律。

 ### 验证清单

 - Python 应用启动不再隐式改 schema
 - Rust 应用启动不再尝试建表
 - migration 顺序独立可观测
 - 双栈在同一 schema 上可稳定启动

 ---

 ## Phase 3：Rust 内部边界收缩

 ### 目标

 先治理复杂度最高的 Rust 域，而不是继续追求“端点数量增长”。

 ### 要实施的内容

 1. 拆分章节域：
    - `chapter_crud`
    - `chapter_generation`
    - `chapter_batch_generation`
    - `chapter_analysis`
    - `chapter_regeneration`
    - `chapter_annotation`
    - `chapter_quality`
    - `chapter_draft`
 2. 统一任务语义：
    - status
    - checkpoint
    - stream event shape
    - recover / resume semantics
 3. compat/legacy adapter 集中化，避免散落到 route 层。

 ### 参考模式

 - `docs/architecture/chapter-api-gateway-seams.zh-CN.md`
 - `docs/architecture/backend-refactor-milestone-summary-2026-04-21.zh-CN.md`

 ---

 ## Phase 4：安全与配置硬化

 ### 要实施的内容

 1. `JWT_SECRET` 在非开发环境必须为必填，禁止随机生成回退。
 2. 让 `CORS_ORIGINS` 真正参与 Rust router 的 CORS 配置。
 3. 收敛手工 cookie 拼接逻辑。
 4. 收敛 public-path 字符串白名单方式。
 5. 收紧 `sqlite::memory:` 类开发回退逻辑，避免部署误用。

 ---

 ## Phase 5：收口迁移与去 Python 化

 ### 要实施的内容

 1. 建立 Python API -> Rust API parity matrix。
 2. 给每个 route group 标记 owner / smoke / rollback / schema assumptions。
 3. 稳定切流后再移除 Python fallback。
 4. 最后做 schema contract 清理和 Python 退场。

 ---

 ## 6. 本轮执行策略

 本轮只实施 **Phase 1：迁移控制面硬化**。

 原因：

 - 它是后续继续迁移更多域之前的最低成本高收益步骤。
 - 它不需要马上重写高风险业务域。
 - 它能显著提升部署可验证性与回滚信心。

 ---

 ## 7. 本轮交付物

 计划交付以下内容：

 1. 本规划文档
 2. strangler 路由/探针 manifest
 3. `deploy-strangler.ps1` 中的 gateway smoke 集成
 4. 输出到 `tmp/smoke/` 的结构化 smoke 结果

 ---

 ## 8. 成功标准

 满足以下条件视为本轮完成：

 - 部署脚本具备结构化 gateway smoke 步骤
 - Rust 与 Python fallback 至少各有一个 through-gateway 探针
 - smoke 结果落盘到 `tmp/smoke/`
 - 探针失败时部署脚本会终止并保留诊断
 - 本次变更不扩大 Rust 业务面，只增强迁移控制面
