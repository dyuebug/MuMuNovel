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

 ### 2026-05-20 阶段补充

 当前 Phase 5 已不再停留在“只做规划”的状态，而是进入了
 “治理资产 + 窄切片 Rust 开发并行推进”的阶段：

 1. `deploy/strangler-gateway-probes.json`、P0/P1 ownership checklist、
    parity matrix、rollback runbook 已形成第一版可执行治理资产。
 2. `phase5-p0` / `phase5-p0-fallback` / `phase5-p0-asymmetric`
    已经可以独立验证，不再只依赖口头 owner 判断。
 3. 对 `chapters` 来说，当前更合理的推进方式不是立即移除 Python fallback，
    而是在治理资产足够的前提下，继续做小范围、可验证、行为保持的 Rust
    seam 收口。
 4. 因此本阶段允许进入 `backend-rs` 的低风险开发切片，但约束仍然不变：
    - 只做小步行为保持重构
    - 优先 `chapter_batch_generation` / `chapter_generation` 邻近 seam
    - 每次切片都要配 `cargo check` 与 focused tests
    - 不把 Phase 5 治理工作和新的业务扩张绑在一起

 ### 2026-05-21 阶段补充：剩余迁移面与执行顺序校准

 当前 Phase 5 需要区分两类“尚未完成”的迁移面，避免把 owner、fallback、
 治理成熟度混为一谈：

 1. **仍由 Python owner 承担的 route group / path**
 2. **Rust 已是 through-gateway owner，但仍保留 Python fallback 或缺少更强 smoke / rollback 证据的 route group**

 #### 5.1 剩余迁移面的判断规则

 后续所有 Phase 5 进度判断统一按以下规则执行：

 1. **owner 以 through-gateway 默认流量为准，不以“仓库里还有没有代码”判断。**
 2. **不能只依据 Nginx 中的遗留 `location` 注释判断 owner，必须同时核对：**
    - `deploy/nginx/mumunovel.conf`
    - Python `backend/app/bootstrap/router_registry.py`
    - Rust `backend-rs/src/api/router.rs`
    - 对应 route module 的真实路径定义
 3. **“已迁移”不等于“可移除 fallback”。**
    一个 route group 只有同时具备 owner 证据、fallback 证据、rollback 手册、
    stronger business smoke，才可评估进入 cutover 收口。
 4. **schema owner 与 API owner 分开判断。**
    即使 API 默认流量已切到 Rust，只要 Alembic / migration owner 仍是 Python，
    就不能把该组视为完全去 Python 化。

#### 5.2 当前确认的剩余迁移面

截至 2026-05-24 的最新校准，Phase 5 不再使用“仓库里还剩多少 Python
文件”作为主进度指标，而统一拆成以下三层迁移剩余量：

1. **API owner 剩余量**
2. **Python fallback 清退剩余量**
3. **schema / migration owner 切换剩余量**

这样拆分的原因是：当前大量 Python route 文件已经退化为 compatibility
shell，但这并不等价于 through-gateway 默认流量仍由 Python owner 承担；
反过来，某些 route group 即使已经由 Rust 默认承接，也不等价于 fallback
可以立刻移除，更不等价于 schema owner 已完成去 Python 化。

##### 5.2.1 API owner 剩余量：低到中

当前章节域和多个核心 route group 的 API owner 迁移已经进入后半段：

- `backend-rs/src/api/router.rs` 已承担 `auth`、`users`、`projects`、
  `chapters`、`settings`、`wizard`、`memories`、`book_import`、
  `polish` 等核心 route group 聚合入口。
- `deploy/nginx/mumunovel.conf` 中，大量 through-gateway 默认流量已明确
  指向 Rust，包括 `/api/auth/`、`/api/settings*`、`/api/chapters*`、
  `/api/book-import*`、`/api/mcp*` 等路径。
- `05-18-backend-chapter-generation-refactor-followup/design.md` 的最新
  file-level migration map 也已确认：
  `chapter_analysis_task_routes.py`、
  `chapter_annotation_routes.py`、
  `chapter_batch_generation_routes.py`、
  `chapter_draft_routes.py`、
  `chapter_expansion_plan_routes.py`、
  `chapter_generation_routes.py`、
  `chapter_partial_regeneration_routes.py`、
  `chapter_quality_routes.py`、
  `wizard_stream.py`
  均已进入 `Migrated` 状态。

当前唯一仍需谨慎按 seam cleanup 处理的章节混合边界，是
`backend/app/api/chapters.py`。但该文件应视为 compatibility shell，
而不是新的主迁移债务入口。

规划判断：

- Phase 5 在 API owner 维度上，剩余量为 **低到中**
- 后续不应再把“清理 Python route 文件数量”误当成 owner 迁移主进度
- 更合理的推进方式，是继续做 Rust 侧语义收口、route delegation polish，
  并只在已有 Rust owner 完整存在时移除遗留 Python shell

##### 5.2.2 Python fallback 清退剩余量：中到高

尽管大量核心路径已默认走 Rust，但 fallback 清退仍明显没有完成：

- `deploy/nginx/mumunovel.conf` 仍保留 `/api/` catch-all 到 Python，
  这意味着未显式声明 owner 的 API 流量仍可能落回 Python。
- `/api/wizard/` 与 `/api/wizard-stream/` 仍保留 Python fallback 前缀；
  当前是“Rust 先拦截部分明确子路径，剩余路径回 Python”的混合状态。
- `/memories/` 仍保留 Python 路径，说明该域仍存在双栈兼容面的存量。

规划判断：

- Phase 5 在 fallback 清退维度上，剩余量为 **中到高**
- 只有当 route group 同时具备 owner 证据、fallback 证据、rollback
  手册、以及更强的 business smoke 证据后，才能评估进入 cutover 收口
- 当前不应把“Rust 已经能处理一部分请求”误判为“可以立刻删除 Python
  fallback”

##### 5.2.3 schema / migration owner 切换剩余量：高

schema owner 与 API owner 必须分开核算。当前 shared-DB 迁移仍未完成真正
去 Python 化：

- Python 仍通过 `backend/scripts/entrypoint.sh` 在容器启动时执行 Alembic
  迁移，因此 schema owner 仍是 Python。
- Rust 端仍带有启动期建表/补表过渡逻辑，说明还未进入正式 migration owner
  阶段。
- 这部分仍属于 Phase 2 “Schema 所有权和迁移流程解耦”的未完成债务，只是
  当前执行节奏被 Phase 5 的 route-group owner 治理与 seam 收口并行覆盖。

规划判断：

- Phase 5 如果以“真正去 Python 化”作为目标衡量，剩余量仍然 **高**
- API 默认流量 owner 已切到 Rust，并不等价于 Python 可以整体退场
- 在 schema / migration owner 完成切换前，任何“迁移完成度”表述都必须
  显式注明这是 API owner 维度，而不是整体去 Python 化维度

##### 5.2.4 进度口径约束

从本节开始，后续所有 Phase 5 进度汇报、checklist 更新、cutover 评估，
统一按以下口径执行：

1. API owner 进度单独统计
2. fallback 清退进度单独统计
3. schema / migration owner 进度单独统计
4. 如需给出总体判断，必须明确说明属于哪一层，不得再输出模糊的单一百分比

##### 5.2.5 整个 `backend/app` 目录口径校准（2026-05-26）

为了避免继续把“Rust 已承接大量 API 默认流量”和“Python 文件还大量存在”
混为一谈，Phase 5 从本轮开始补充整个 `backend/app` 目录的现实校准口径。

截至 2026-05-26，当前仓库真实文件统计如下：

- `backend/app` 下共有 **330 个 `.py` 文件**
- `backend-rs/src` 下共有 **204 个 `.rs` 文件**

按 Python 顶层目录分布：

- `backend/app/services`: **227**
- `backend/app/api`: **37**
- `backend/app/models`: **22**
- `backend/app/schemas`: **20**
- `backend/app/bootstrap`: **5**
- `backend/app/mcp`: **4**
- `backend/app/middleware`: **3**
- `backend/app/utils`: **3**
- 顶层单文件：`config.py`、`database.py`、`logger.py`、`main.py`、
  `user_manager.py`、`user_password.py`、`init_relationship_types.py`

按 Rust 顶层目录分布：

- `backend-rs/src/services`: **113**
- `backend-rs/src/api`: **37**
- `backend-rs/src/models`: **30**
- `backend-rs/src/ai`: **7**
- `backend-rs/src/tasks`: **7**
- `backend-rs/src/db`: **2**
- `backend-rs/src/mcp`: **2**
- `backend-rs/src/middleware`: **2**
- `backend-rs/src/utils`: **2**
- 顶层单文件：`config.rs`、`main.rs`

这组数据的规划解释必须固定如下：

1. **文件存在量不等于 owner 迁移完成度。**
   Python 仍有 330 个 `.py` 文件，说明“整个 Python 后端目录”距离删除还很远；
   但这不能反推 through-gateway 默认 API owner 仍主要在 Python。
2. **当前 Rust 迁移更接近“API owner 先行、内部层逐步收口”，而不是
   “按目录整体一一替换”。**
   特别是 `backend/app/services`、`models`、`schemas` 大量文件仍保留，
   主要说明 shared-DB + compatibility 体系仍在，而不是 API owner 没迁。
3. **如果按整个 `backend/app` 目录衡量，当前迁移完成度必须拆成两种口径：**
   - 文件存在量口径：Python 仍明显占大头，迁移完成度不能表述为高
   - API owner 口径：核心 route group 已进入后半段，迁移完成度可以表述为高
4. **后续汇报“整个 Python 后端还剩多少”时，必须明确说明属于哪种口径：**
   - “仓库里还剩多少 Python 文件”
   - “默认流量 owner 还剩多少在 Python”
   - “Python fallback 还剩多少”
   - “schema / migration owner 还剩多少”

基于这组全目录校准，当前更准确的总体判断是：

- **按文件存在量**：整个 Python 后端剩余量 **高**
- **按 API owner**：剩余量 **低到中**
- **按 Python fallback 清退**：剩余量 **中到高**
- **按 schema / migration owner**：剩余量 **高**

规划含义：

1. 不应再用“整个 `backend/app` 目录下还有多少 Python 文件”来否定已经完成
   的 Rust API owner 迁移进展。
2. 也不应因为 Rust `api/` 与 `services/` 已具备规模，就误判“整个 Python
   后端已经接近可删除”。
3. Phase 5 后续仍应优先：
   - Rust 侧语义收口
   - stronger smoke / rollback 资产补强
   - fallback 收缩准备
   - schema / migration owner 切换
   而不是追求按目录清空 Python 文件数量。

##### 5.2.6 迁移提速瓶颈分析与提速原则（2026-05-28）

截至 2026-05-28，当前迁移速度偏慢的根因已基本明确。后续提速必须先对
瓶颈做口径分解，否则会继续出现“Rust 代码持续在写，但整体迁移完成度
体感提升有限”的情况。

当前主要瓶颈分为三类：

1. **进度统计口径瓶颈**
   - 当前已经明确：`API owner`、`Python fallback`、`schema / migration owner`
     是三层不同进度。
   - 但执行过程中仍容易把“Rust 接了更多接口”误判为“整体迁移接近完成”，
     从而导致优先级判断失真。
   - 结果是：开发活动集中在 Rust 内部 seam 收口，但 cutover 所需的 fallback /
     smoke / rollback / schema owner 资产推进不够快。

2. **推进单位过细的瓶颈**
   - 当前 `backend-rs` 的小步 seam 收口是正确的降风险策略，但它天然更适合
     “降低复杂度”，不适合作为唯一主推进单位来提升整体迁移速度。
   - 当每轮只去掉一个 file-local wrapper、一个单消费者 helper、一个
     optional->required shell 时，内部质量会提升，但 route-group owner、
     fallback 清退、schema owner 切换不会同步显著前进。
   - 结果是：Phase 5 在代码层看似持续推进，但 fallback 与 schema 层仍停滞。

3. **cutover 前置资产不成批的瓶颈**
   - 当前 owner / fallback / rollback / stronger smoke 资产虽然已有基础，
     但还没有形成按 route group 批量收缩 Python fallback 的执行包。
   - 这意味着即使 Rust owner 已经较稳定，也只能继续做局部 seam 收口，
     而不能开始“按组减少 Python 承接面”。
   - 同时，schema / migration owner 仍由 Python 主导，这会持续压制整体
     去 Python 化速度。

基于以上瓶颈，后续提速必须遵守以下原则：

1. **继续允许 seam 收口，但不再把 seam 收口当作唯一主进度指标。**
2. **后续每轮推进都必须明确属于哪一层：**
   - API owner 收口
   - fallback 收缩准备
   - schema / migration owner 切换
3. **优先把能触发阶段进度跳变的资产做成批次交付物，**
   而不是只累积单点 helper 优化。

截至 2026-05-21，结合 router 注册、Rust route 清单与 gateway 配置，当前应按下列口径理解：

 1. **不存在已确认的独立 Python `/api/wizard/*` route group。**
    当前 Python 侧真实注册的是 `wizard_stream.router`，
    即 `/api/wizard-stream/*`；Rust 侧实现的也是 `/api/wizard-stream/*`。
    Nginx 中的 `/api/wizard/` 更应视为遗留/兜底 location，
    不能再直接当作“已确认仍由 Python owner 承担的 API 组”。
 2. **`wizard-stream` 已是 Rust owner，但仍保留更宽的 Python fallback。**
    这说明它属于“已迁移但未完成收口”，而不是“未迁移接口”。
 3. **`projects` 已基本由 Rust owner 覆盖。**
    当前重点不是重新统计项目接口数量，而是补更强业务 smoke 与回切证据。
 4. **`chapters`、`settings`、`auth`、`users`、`memories`、`book_import`、
    `characters`、`outlines` 等核心组，当前更准确的状态是：**
    - Rust 已承担默认流量 owner，或 owner 证据已较强
    - 但 Python fallback 仍存在
    - stronger business smoke / rollback / schema assumption 仍需继续收口
 5. **真正尚未完成的“去 Python 化”剩余量，主要不再是 route 数量本身，而是：**
    - fallback 收口准备度
    - stronger smoke 覆盖度
    - rollback 可执行性
    - schema / migration owner 迁移

 #### 5.3 当前阶段进度判断

 当前 Phase 5 更适合按治理成熟度而非“接口个数”描述：

 1. route-group owner 覆盖率：**高**
    `Rust` 已承担大部分核心 route group 的默认流量。
 2. Phase 5 cutover 治理成熟度：**中低**
    owner / fallback / rollback / smoke 资产已成型，但仍不够支撑大规模移除 fallback。
 3. 去 Python fallback 准备度：**中低**
    仍需更强 business smoke 与按组回切验证。
 4. schema / migration owner 迁移成熟度：**低**
    `Python Alembic` 仍是 schema owner，Rust 尚未进入独立 migration owner 状态。

#### 5.4 Phase 5 剩余执行顺序

 后续推进顺序统一调整为：

 1. **先继续校准剩余 path inventory，但只处理“真实存在的 route group”。**
    优先核对是否还有 route-group owner 认知与真实 router / gateway 不一致的地方，
    及时修正文档，不再把遗留 location 当作已确认 API owner。
 2. **继续 `chapters` 邻域的 Rust seam 收口。**
    优先处理 `chapter_generation` / `chapter_batch_generation` 中仍跨 owner
    共享的 read-side / view-context / workflow 装配边界，保持“小步、可测、行为不变”。
 3. **补强 P0 route group 的 stronger smoke。**
    优先 `chapters`、`projects`、`wizard-stream`、`settings`、`memories`。
 4. **补强 P1 route group 的业务 smoke。**
    优先 `auth`、`users`、`characters`、`outlines`、`book_import`。
 5. **在 stronger smoke 与 rollback 资产足够后，再评估缩小 Python fallback 面。**
    不提前进入 catch-all 收缩。
6. **最后才进入 schema / migration owner 迁移与 Python 退场。**
    包括停止 Rust 启动建表、停止 Python 启动隐式迁移、建立独立 migration 流程。

#### 5.5 提速策略：从 seam 收口转向“三线并行”（2026-05-28）

从本轮开始，Phase 5 的提速策略正式调整为三条执行线并行，而不是单线持续
做 Rust seam 微收口。

##### 5.5.1 执行线 A：Rust seam 收口，目标从“减复杂度”升级为“服务 cutover”

这条线继续保留，但约束升级：

1. 只优先做直接服务于 route-group cutover 的 seam：
   - `chapter_generation`
   - `chapter_batch_generation`
   - `chapters` compatibility shell 邻域
2. 不再优先做低收益、仅改善局部美观的 JSON 搬运或对称性整理。
3. 每个 seam slice 必须明确标注它服务于哪一项 cutover 资产：
   - stronger smoke 更容易补
   - fallback 更容易收缩
   - rollback 边界更清晰
   - schema assumption 更清晰

这意味着：

- seam 收口继续做，但它现在是 **服务于 fallback 清退与 cutover 的准备动作**
- 如果一个 seam 只减少了局部 owner 数量，却不提升 cutover readiness，
  优先级应下调

##### 5.5.2 执行线 B：按 route group 打包 fallback 收缩准备

这是当前最缺、也是最能显著提升整体迁移速度的一条线。

从本轮开始，不再只维护“owner 已经在 Rust”的认知，而是要为 P0 / P1 route
group 形成可执行的 cutover 包。每个 cutover 包至少包含：

1. owner 证据
2. fallback 证据
3. stronger smoke
4. rollback 步骤
5. schema assumptions

建议优先顺序：

1. `chapters`
2. `settings`
3. `wizard-stream`
4. `projects`
5. `memories`

提速含义：

- 不再等待“整个 Rust 内部完全优雅”才讨论 fallback
- 而是当某个 route group 的治理资产足够时，就进入“可评估缩小 fallback 面”
  的状态

##### 5.5.3 执行线 C：提前启动 schema / migration owner 切换

这是当前整体迁移速度被压制最明显的一层，也是过去容易被持续推迟的一层。

后续必须把 schema / migration owner 从“远期议题”前移为并行执行线。原因是：

1. API owner 再继续推进，也不会自动提升 schema owner 进度
2. 如果这一层始终不动，整体“去 Python 化完成度”会长期卡在中段
3. fallback 收缩越多，越需要更稳定、更独立的 migration owner 作为回滚与
   部署基线

从提速角度，schema 线的近期目标不是一次性全部切完，而是：

1. 先把现状显式化：
   - 哪些 migration 仍只能由 Python Alembic 驱动
   - 哪些 Rust model 仍依赖历史补表/建表逻辑
2. 建立迁移 owner 切换 checklist：
   - migration command owner
   - startup behavior
   - expand / switch / contract 顺序
   - deploy verification
3. 让 schema owner 切换从“以后再说”变成有明确里程碑的执行项

##### 5.5.4 提速后的阶段目标重定义

从本节开始，Phase 5 的“加速推进”不再定义为“单周完成更多 seam 小切片”，
而统一改成以下三个结果指标：

1. **每轮至少推进一个高价值 seam**
2. **每轮至少补强一个 route-group cutover 资产**
3. **每两到三轮必须推进一次 schema / migration owner 显式化工作**

只有同时推进这三类结果，才算真正提高迁移速度；否则只是局部 Rust 代码
质量提升，而不是整体 Python -> Rust 迁移速度提升。

 ---

 ## 6. 执行策略

 执行顺序已从最初的 “先只做 Phase 1” 演进为：

 1. 先完成 Phase 1 / 2 / 4 的基础收口，确保 deploy、schema owner、
    startup/runtime guardrails 不再漂移。
 2. 以 Phase 5 的 route-group 治理资产作为切流判断基础。
 3. 在不破坏外部契约的前提下，持续推进 Phase 3 风格的
    `backend-rs` 窄切片重构，优先服务于 `chapters` 域的 cutover 信心。

 这意味着当前轮次不再限制为“只增强迁移控制面”，而是允许：

 - 继续补 owner / fallback / asymmetric 证据
 - 同时进入低风险 Rust seam 收口开发
 - 但仍禁止扩大业务面或提前移除 Python fallback

 ---

 ## 7. 当前交付物

 当前已形成或正在持续扩充的交付物：

 1. 本规划文档
 2. strangler 路由/探针 manifest
 3. `deploy-strangler.ps1` 中的 gateway smoke 集成
 4. 输出到 `tmp/smoke/` 的结构化 smoke 结果
 5. Phase 5 P0/P1 ownership checklist / parity matrix / rollback runbook
 6. 基于上述治理资产推进的 `backend-rs` 小步 seam 收口切片

 ---

 ## 8. 当前成功标准

 当前阶段满足以下条件，才可视为进入“可持续推进 Rust 迁移”的状态：

 - 部署脚本具备结构化 gateway smoke 步骤
 - Rust 与 Python fallback 至少各有一个 through-gateway 探针
 - smoke 结果落盘到 `tmp/smoke/`
 - 探针失败时部署脚本会终止并保留诊断
 - 关键 route group 已具备 owner / fallback / asymmetric 的最小治理资产
 - `backend-rs` 的新切片可以在不改变外部行为的前提下持续落地并验证
