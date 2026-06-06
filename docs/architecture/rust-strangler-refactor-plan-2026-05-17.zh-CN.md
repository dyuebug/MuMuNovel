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
    - 这条 2026-05-20 口径已被 2026-06-06 整模块加速策略覆盖；
      小步只允许作为模块包内部验证手段，不再作为默认规划单位
    - 优先 `chapter_batch_generation` / `chapter_generation` 邻近 seam
    - 每次切片都要配 `cargo check` 与 focused tests
    - 不把 Phase 5 治理工作和新的业务扩张绑在一起

### 2026-06-04 阶段补充：从窄切片提速到模块级迁移包

经过连续多轮 `chapter_generation` / `chapter_batch_generation` Rust owner
收口后，继续只按小块切分推进已经开始产生明显调度成本。后续 Phase 5 的
默认推进单位应从“单个 helper / 单条 owner hop”调整为“模块级迁移包”：

1. **模块级迁移包优先**
   - 一个迁移包至少要同时覆盖：
     - Rust route / service owner
     - 对应 Python fallback shell
     - task / runtime / checkpoint / SSE 或 response contract
     - smoke / rollback / schema assumption
   - 允许在包内拆小步提交验证，但规划和验收必须按模块整体判断。

2. **质量门槛不能降级**
   - 保持 HTTP payload、SSE event、task lifecycle、checkpoint、provider
     默认行为稳定。
   - Rust 代码要优先保持可维护性、健壮性和可读性；非显而易见的
     runtime、fallback、checkpoint、rollback 逻辑可以加简短注释。
   - 每个模块包至少需要 focused Rust tests + `cargo check`；涉及路由切流
     或 fallback 行为时，还要补 route-group smoke 或 manifest validation。

3. **允许适当调整框架和逻辑**
   - 如果现有结构导致迁移持续卡在 wrapper / helper 微调，可以调整服务边界、
     workflow owner、错误类型或请求/响应 adapter。
   - 调整前提是行为契约和回滚路径明确，不能用“提速”替代验证。
   - 新结构必须减少迁移阻力，例如让一个 module 能一次性回答 owner、
     fallback、schema assumption 和 rollback 四个问题。

4. **建议优先级**
   - `chapter_batch_generation`：
     batch create/resume/read/status/stream/runtime 作为一个完整 package 推进。
   - `chapter_single_generation`：
     prepare/write/stream/runtime launch 作为一个完整 package 推进。
   - `chapters` compatibility shell：
     只在 Rust owner 已完整存在时做 delegation / fallback shrink。
   - `schema / migration owner`：
     当模块包暴露表或字段所有权依赖时，同步推进 Phase 2 的 migration owner
     收口，不再把它长期留作远期债务。

这次策略调整不是放弃小步验证，而是把小步验证放回模块级目标内部。后续进度
不再主要用“本轮又收掉几条 wrapper”衡量，而应以“哪个模块 package 已具备
Rust owner、fallback shrink、smoke、rollback、schema assumption 的完整证据”
作为主要进度口径。

### 2026-06-06 阶段补充：Phase 5 加速策略调整为整文件 / 整功能 / 整模块包迁移

用户已明确要求停止默认“小块开发”推进方式。Phase 5 从本节点开始，默认不再
按“下一条 seam”排队，而是按整文件、整功能组、整模块包来规划和验收。小 seam
仍可存在，但只能作为一个模块包内部的 review / rollback checkpoint，不能再单独
代表一次迁移完成。

新的迁移单位定义如下：

1. **整文件迁移**
   - 一个 Python route / service / helper 文件已经有明确 Rust owner 时，优先按文件
     或文件内完整功能族一次性迁移。
   - 迁移结果要么让 Python 文件退化为冻结兼容壳，要么明确记录仍保留的 fallback
     分支和切流条件。
2. **整功能组迁移**
   - 对跨多个文件的行为，例如 single generation 的 prepare/write/stream/runtime，
     以功能组整体迁移，而不是每轮只挪一个 helper。
   - 功能组必须同时回答 payload、SSE、task lifecycle、checkpoint、provider 默认值、
     error shell 和 rollback 问题。
3. **整模块包迁移**
   - 对 `chapter_generation`、`chapter_single_generation`、
     `chapter_batch_generation` 这类已经有清晰 Rust owner 的模块，优先把相关
     owner chain、fallback shell、smoke、rollback、schema assumption 放在同一包
     内推进。

新的包优先级：

| 优先级 | 模块包 | 迁移目标 | 主要文件/边界 | 完成判断 |
|---|---|---|---|---|
| A | `chapter_generation` shared owners | 继续把 shared lower-level owners 从 batch-named 文件中移出 | `chapter_generation_*` shared access / snapshot / recovery / quality / runtime-context services | batch/single/resume 共享语义不再挂在伪 batch owner 上 |
| B | `chapter_single_generation` | 整体迁移单章 prepare/write/stream/runtime/snapshot/task-model/quality | `chapter_single_generation_prepare_service.rs`、`chapter_single_generation_write_workflow_service.rs`、`chapter_single_generation_stream_workflow_service.rs`、`chapter_single_generation_runtime_state_service.rs` 等 | 单章生成能作为一个 Rust-owned module 被审计，Python shell 仅剩明确 fallback |
| C | `chapter_batch_generation` | 整体迁移 batch read/write/resume/cancel/status/stream/runtime/task-view | `chapter_batch_generation_read_context_service.rs`、`chapter_batch_generation_write_workflow_service.rs`、`chapter_batch_generation_resume_task_command_service.rs`、status/stream/runtime/task-view services | batch route group 具备 owner、smoke、rollback、fallback shrink 证据 |
| D | `chapters` compatibility shell | 缩小或冻结 Python `chapters.py` 兼容壳 | `backend/app/api/chapters.py`、`backend-rs/src/api/chapters.rs`、CRUD/generation/regeneration/analysis Rust owners | 只有 Rust parity 明确的分支才收缩 Python shell |
| E | `schema / migration owner` | 将 schema assumption 从 Python startup 迁移债务前移为显式执行线 | Alembic startup、Rust model/migration readiness、route package 暴露出的表/字段假设 | 包级 owner 不再依赖隐式 Python startup schema mutation |

每个模块包开始前必须先补齐 6 项材料：

1. Python source map：涉及的 Python route、service、schema、fallback shell。
2. Rust target map：涉及的 Rust route、service owner、model、test、smoke probe。
3. 行为契约：HTTP payload、SSE event、task lifecycle、checkpoint、provider default、
   error shell、fallback 语义。
4. 实施边界：哪些文件或 function group 必须一起迁移，哪些兼容壳只冻结不删除。
5. 验证边界：focused Rust tests、`cargo check`、route-group smoke 或 manifest
   validation。
6. 回滚边界：Nginx/gateway route、fallback shell、feature/config knob、migration
   assumption、部署探针。

新的 stop-rule：

- 不再因为“发现一个可删的小 wrapper”就开启下一轮迁移。
- 不再把 helper 搬家计为主要进度，除非它直接收缩 Python fallback、明确 Rust owner、
  增强 smoke/cutover、澄清 rollback，或解除 schema assumption。
- 如果一个包还没有完成 owner / fallback / smoke / rollback / schema 任一关键证据，
  下一轮必须继续该包，除非明确记录暂停原因并切到优先级更高的包。
- 后续“进度”应按模块包剩余量汇报：哪个包已完成、哪个包只剩 fallback shell、哪个包
  仍缺 schema/migration owner，而不是统计又关了几条 seam。

### 2026-06-04 阶段补充：resume owner seam 继续向 validated-execution / restored-runtime 收口

当前 `chapter_generation` / `chapter_batch_generation` 的 Phase 5 Rust seam
收口，已继续沿 batch resume 这条高价值 write lane 往内推进，两条相邻的
resume seam 已进入更窄的 Rust owner 边界：

1. **manual-review blocker ownership 继续前移**
   - `RestoredResumeRuntimeState` 现在直接回答
     `is_manual_review_blocked(...)`
   - `prepare_batch_generation_resume(...)` 不再在 owner materialize 之后，
     重新拼接 `failed_chapters + restored quality_status_context` 来本地判断
     manual-review blocker
   - 这条 seam 的意义不是“多了一个 helper”，而是 resume command 不再保留
     一条 owner 已存在、caller 仍重放质量终态语义的隐式分支

2. **launch-persistence ownership 继续前移**
   - `BatchGenerationResumeLaunchPersistencePlan` 现在直接通过
     `prepare_from_validated_execution(...)` 消费：
     - validated execution owner
     - restored resume runtime owner
     - command state
   - caller 不再在本地先组装 dispatch plan、reset persistence plan，再把
     这些 owner-ready 结果回传给 launch-persistence owner
   - 这条 seam 的意义在于：resume validated-execution -> restored-runtime ->
     launch-persistence 现在形成了更连续的 owner 链，切流审计时更容易回答
     “到底是谁接手了 resume lifecycle 最终装配”

3. 因此，当前 batch resume lane 的 stop-rule 需要再补两条：
   - **不要**在 restored resume runtime owner 已经 materialize 之后，
     仍让 caller 本地重解 manual-review blocker 语义
   - **不要**在 validated execution owner 和 restored runtime owner 已经齐备后，
     仍保留一个 resume launch-persistence caller wrapper 来本地拼装
     dispatch/reset/response bundle

4. 这两条 seam 都属于真实 Phase 5 Rust migration，而不是纯 helper 平移：
   - 都直接减少了 `owner materialized -> caller local rebuild` 的 resume
     生命周期 hop
   - 都提高了 batch resume lane 的 cutover auditability
   - 都保持在 `chapter_generation` 邻域，不扩散到 schema / deploy /
     全仓 warning 清理

### 2026-06-05 阶段补充：batch owned task-sources / read-state 分层收口

当前 `chapter_batch_generation` 的 Phase 5 seam 收口，已经继续把
`status/stream` 与 `cancel/resume` 两组相邻生产链拆成两个明确层级，而不再让
一个 owner 同时承担 read-side recovery 和 command-side no-recovery 语义：

1. **新增 lower-level owned task-sources owner**
   - `backend-rs/src/services/chapter_batch_generation_owned_task_query_service.rs`
     现在显式提供：
     - `OwnedBatchGenerationTaskSources`
     - `load_owned_batch_generation_task_sources(...)`
   - 这个 owner 只负责：
     - owned task lookup
     - snapshot load
   - 它**不**执行 `recover_batch_generation_task_if_needed(...)`，因此可以安全复用到
     `cancel` / `resume` 这些必须保持现有 non-recovery 行为的命令链

2. **higher-level owned read-state owner 继续只服务 read/query lanes**
   - 同一模块中的
     `load_owned_batch_generation_task_read_state(...)`
     现在建立在 sources owner 之上，再叠加 recovery
   - 这保证：
     - `status payload`
     - `status stream`
     继续复用 shared owner
   - 同时避免把 recovery 误下沉到命令链，导致 `cancel/resume` 语义漂移

3. **cancel / resume 已完成对 shared sources owner 的真实接入**
   - `chapter_batch_generation_cancel_service.rs` 不再本地重放：
     `load_owned_task(...) -> load_batch_generation_snapshot(...)`
   - `chapter_batch_generation_resume_task_command_service.rs` 也不再本地重放：
     `load_owned_task(...) -> load_batch_generation_snapshot(...)`
   - 两条命令链现在都直接消费 shared `OwnedBatchGenerationTaskSources`
   - 同时保留原有错误边界：
     - task lookup failure 仍走 task error
     - snapshot load failure 仍分别保持 cancel 的 domain / resume 的 config
       语义，不因为 owner 共用而被粗暴合并

4. 这条 seam 属于真实 Phase 5 Rust migration，而不是“又抽了一个公用 helper”：
   - 它直接减少了 batch cancel/resume lane 的重复
     `owned task + snapshot` production hop
   - 它明确建立了：
     - `sources owner`
     - `read-state owner`
     两层 contract，为后续继续整块推进 `chapter_batch_generation`
     模块迁移提供更稳定的 owner 骨架
   - 它也说明了当前提速策略的边界：
     **可以整块共享 sources，但不能为了共享而把 read-side recovery 语义错误地下沉到 command lane**

### 2026-06-05 阶段补充：batch cancel 正式并入 write-workflow lane

当前 `chapter_batch_generation` 的 Phase 5 seam 收口，已经继续把 batch
command lane 往“整块模块 owner”推进，而不是只在 `cancel_service` 内继续做局部
抽取：

1. **cancel 不再是 route-local special case**
   - 之前 batch command lane 的 owner 形状并不一致：
     - `create` -> write workflow
     - `resume` -> write workflow
     - `cancel` -> route 直接调 lower-level cancel service
   - 这会让 batch command cutover 审计时，`cancel` 总是单独挂在外面，
     不利于按模块整体回答“这一组命令入口到底由谁拥有”

2. **现在 cancel 也进入 batch write-workflow public-start**
   - `backend-rs/src/services/chapter_batch_generation_write_workflow_service.rs`
     现在新增：
     - `PreparedBatchGenerationCancelWorkflowLaunch`
     - `PreparedBatchGenerationCancelWorkflowStart`
     - `cancel_owned_batch_generation_write_workflow(...)`
   - `backend-rs/src/api/chapter_batch_generation.rs`
     的 cancel route 也已经改为委托这个 write-workflow public-start owner

3. **lower-level cancel service 因此回缩到更清晰的职责**
   - `chapter_batch_generation_cancel_service.rs` 现在主要只负责：
     - owned task + snapshot source loading
     - terminal status gating
     - cancelled persistence-plan preparation
   - 也就是说，`cancel_service` 不再同时承担 route-facing public-start 壳，
     而是和 `resume command` 一样回到更窄的 command preparation owner

4. 这条 seam 的意义不是“单独修好 cancel”，而是继续把
   `chapter_batch_generation` 当成一个**模块级迁移包**来推进：
   - batch command lane 现在更接近统一 owner 形状
   - 后续再做 route/fallback shrink、smoke evidence、rollback 审计时，
     create/resume/cancel 可以按同一类 write-workflow lane 看待
   - 这比继续在 route 层保留一个 cancel 特例，更符合当前 Phase 5 的提速策略：
     **整块迁移、统一 owner、保持行为不变**

### 2026-06-05 阶段补充：batch write-workflow start 空壳层收口

在 batch cancel 正式并入 write-workflow lane 之后，本轮继续沿同一条
`chapter_generation` / `chapter_batch_generation` Phase 5 主线，把 batch
write lane 上仍残留的
`workflow start -> prepare -> persist_and_dispatch`
空壳 hop 再收掉一层。

1. `chapter_batch_generation` 的 create / resume / cancel 写入邻域已经有：
   - create workflow entry owner
   - resume workflow launch owner
   - cancel workflow launch owner
   - public write-workflow entrypoints
2. 但这三条 lane 之前仍会各自在外层保留一个额外 wrapper：
   - `PreparedBatchGenerationCreateWorkflowStart`
   - `PreparedBatchGenerationResumeWorkflowStart`
   - `PreparedBatchGenerationCancelWorkflowStart`
3. 这代表 batch write lane 在 Phase 5 上仍保留一条 Python-era 的旧 hop：
   真正的 workflow entry / launch owner 已经存在，但 public-start 邻层仍
   平行保留一条只负责转手调用的
   `prepare -> persist_and_dispatch`
   forwarding shell。
4. 本轮已把这条 shared batch write-workflow start contract 真正前移回
   邻接 owner 本身：
   - `backend-rs/src/services/chapter_batch_generation_write_workflow_service.rs`
     现在直接拥有：
     - `PreparedBatchGenerationCreateWorkflowEntry::start(...)`
     - `PreparedBatchGenerationResumeWorkflowLaunch::start(...)`
     - `PreparedBatchGenerationCancelWorkflowLaunch::start(...)`
   - 三个 public write-workflow entrypoints
     现在直接把 start handoff 交给这些 owner，不再本地 reopen
     `PreparedBatchGeneration*WorkflowStart`
5. 这条 seam 的意义不在于“少写了一个 struct 名字”。它真正回答的是：
   batch create / resume / cancel 一旦都已经交给 Rust write-workflow owner，
   public-start 邻层到底是不是还要保留一条不承担语义的空壳 workflow-start
   forwarding 支路。
   现在这条 duplicate 已被删掉，adjacent owner 本身终于成为
   batch write-lane start 的显式 Rust materialization 边界。
6. 对 Phase 5 的价值同样直接：
   - cutover 审计时更容易回答“batch create/resume/cancel 的 write-workflow
     start 到底由哪个 Rust owner 接手”
   - public start / workflow entry / workflow launch / persist-and-dispatch
     现在共享更连续的 owner 链
   - fallback shrink / rollback / stronger smoke 在 batch write 邻域上
     又少了一条藏在 public-start 邻层里的 local forwarding 支路
7. 验证：
   - `rustfmt --edition 2021 "backend-rs/src/services/chapter_batch_generation_cancel_service.rs" "backend-rs/src/services/chapter_batch_generation_write_workflow_service.rs"`
   - `cargo test chapter_batch_generation_write_workflow_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-workflow-start-owner-collapse" -- --nocapture`
   - `cargo test chapter_batch_generation_cancel_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-workflow-start-owner-collapse" -- --nocapture`
   - `cargo test chapter_batch_generation --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-workflow-start-owner-collapse" -- --nocapture`

### 2026-06-05 阶段补充：batch create workflow-entry 空壳层收口

在 batch write-workflow start 空壳层收口之后，本轮继续沿同一条
`chapter_generation` / `chapter_batch_generation` Phase 5 主线，把 batch
create lane 上仍残留的
`workflow entry -> persistence-plan owner`
空壳 hop 再收掉一层。

1. `chapter_batch_generation` 的 create 写入邻域已经有：
   - create workflow launch owner
   - create persistence-plan owner
   - public create write-workflow entry
2. 但 create lane 之前仍会在外层保留一个额外 wrapper：
   - `PreparedBatchGenerationCreateWorkflowEntry`
3. 这代表 batch create lane 在 Phase 5 上仍保留一条 Python-era 的旧 hop：
   真正的 create persistence-plan owner 已经存在，但 public-start 邻层仍
   平行保留一条只负责转手调用的
   `prepare persistence plan -> persist_and_dispatch`
   forwarding shell。
4. 本轮已把这条 batch create workflow-entry contract 真正前移回
   persistence-plan owner 本身：
   - `backend-rs/src/services/chapter_batch_generation_write_workflow_service.rs`
     现在直接拥有：
     - `BatchGenerationCreateLaunchPersistencePlan::prepare(...)`
     - `BatchGenerationCreateLaunchPersistencePlan::start(...)`
   - public create write-workflow entry
     现在直接把 start handoff 交给该 owner，不再本地 reopen
     `PreparedBatchGenerationCreateWorkflowEntry`
5. 这条 seam 的意义不在于“少一层中间 struct”。它真正回答的是：
   batch create 一旦已经把 launch materialization 收口到 Rust owner，
   public-start 邻层到底是不是还要保留一条不承担独立语义的 workflow-entry
   forwarding 支路。
   现在这条 duplicate 已被删掉，create persistence-plan owner 本身终于成为
   batch create write-lane start 的显式 Rust materialization 边界。
6. 对 Phase 5 的价值同样直接：
   - cutover 审计时更容易回答“batch create 的最终 write-lane start
     到底由哪个 Rust owner 接手”
   - public start / persistence-plan prepare / persist-and-dispatch
     现在共享更连续的 owner 链
   - fallback shrink / rollback / stronger smoke 在 batch create 邻域上
     又少了一条藏在 public-start 邻层里的 local forwarding 支路
7. 验证：
   - `rustfmt --edition 2021 "backend-rs/src/services/chapter_batch_generation_write_workflow_service.rs"`
   - `cargo test chapter_batch_generation_write_workflow_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-create-entry-collapse" -- --nocapture`
   - `cargo test chapter_batch_generation --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-create-entry-collapse" -- --nocapture`
   - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-create-entry-collapse"`

5. **execution-eligibility ownership 也继续前移**
   - `ResumeExecutionEligibilityPlan` 现在可直接通过
     `from_command_state(...)` 消费 `ResumeBatchGenerationCommandState`
   - `prepare_batch_generation_resume(...)` 不再在 caller 侧手动串接：
     - `resolve_execution_selection()`
     - 选择失败到 domain error 的映射
     - 再回传给 eligibility owner
   - 这条 seam 的意义在于：resume command-state -> execution-eligibility
     的边界现在更明确，caller 又少了一条 owner 已存在但仍重放验证前置
     语义的隐式分支

6. 因此当前 batch resume lane 的补充 stop-rule 再加一条：
   - **不要**在 `ResumeBatchGenerationCommandState` 和
     `ResumeExecutionEligibilityPlan` 都已经存在后，仍让 caller 本地重组
     execution selection + domain error mapping 再交回 eligibility owner

7. **validated-execution ownership 也继续前移**
   - `ValidatedResumeExecutionPlan` 现在可直接通过
     `from_command_state(...)` 消费 command-state owner
   - caller 不再继续本地串接：
     - eligibility owner 构造
     - access / prerequisite validation
     - 再把 validated plan 交给 launch-persistence owner
   - 这条 seam 的意义在于：resume command-state -> eligibility ->
     validated-execution 现在形成了更连续的 validation owner 链，resume
     caller 再少一条 owner 已存在但仍补完 validated handoff 的隐式分支

8. 因此当前 batch resume lane 的补充 stop-rule 再加一条：
   - **不要**在 `ResumeBatchGenerationCommandState`、
     `ResumeExecutionEligibilityPlan`、`ValidatedResumeExecutionPlan`
     都已经存在后，仍让 caller 本地串完整条 eligibility ->
     validated-execution handoff 再交回下游 owner

9. **dispatch-plan ownership 也继续前移**
   - `ResumeExecutionDispatchPlan` 现在可直接通过
     `from_validated_execution(...)` 消费：
     - validated execution owner
     - restored runtime owner
     - normalized target-word-count contract
   - `BatchGenerationResumeLaunchPersistencePlan` 不再继续本地拆
     restored runtime state、重组 dispatch-plan，再把结果塞回 dispatch owner
   - 这条 seam 的意义在于：resume validated-execution -> dispatch-plan ->
     launch-persistence 现在形成了更连续的 launch owner 链

10. 因此当前 batch resume lane 的补充 stop-rule 再加一条：
   - **不要**在 `ValidatedResumeExecutionPlan`、
     `RestoredResumeRuntimeState`、`ResumeExecutionDispatchPlan`
     都已经存在后，仍让邻近 owner 本地重组 dispatch-plan handoff 再交回
     dispatch owner

11. **dispatch runtime ownership 也继续前移**
   - `ResumeExecutionDispatchPlan` 现在不仅 materialize dispatch contract，
     也直接拥有最终 runtime dispatch：
     `dispatch(self, db, task_id)`
   - `BatchGenerationResumeLaunchPersistencePlan::persist_and_dispatch(...)`
     不再把现成的 dispatch owner 再交给一个 owner 外部 helper 去分支
     single/batch runtime dispatch
   - 这条 seam 的意义在于：resume dispatch-plan -> runtime dispatch
     现在形成了更连续的 dispatch owner 边界，launch-persistence 邻层
     不再保留最后一跳 runtime branch 语义

12. 因此当前 batch resume lane 的补充 stop-rule 再加一条：
   - **不要**在 `ResumeExecutionDispatchPlan` 已经完整 materialize 之后，
     仍让邻近 owner / helper 在 owner 外部继续重放
     single-chapter vs batch runtime dispatch 分支

13. **batch-create startup-to-runtime-seed ownership 也继续前移**
   - `BatchGenerationCreateStartupRuntimeState` 现在不仅 materialize：
     - request runtime-state
     - seeded runtime-state payload
     - startup seed source
     也直接拥有到 `BatchGenerationCreateRuntimeSeed` 的投影：
     `into_runtime_seed(self)`
   - `BatchGenerationCreateRuntimeSeed::prepare(...)` 不再把现成的 startup
     owner 再交给邻近 rebuild hop 去重组
     `request_runtime_state + runtime_state_payload -> dispatch-ready compat`
   - 这条 seam 的意义在于：batch create startup owner -> runtime seed
     现在形成了更连续的 startup owner 边界，launch 邻层不再保留这条
     owner-materialized 后的 seed rebuild 语义

14. 因此当前 batch create / startup lane 的补充 stop-rule 再加一条：
   - **不要**在 `BatchGenerationCreateStartupRuntimeState` 已经完整
     materialize 之后，仍让邻近 owner / helper 在 owner 外部继续重放
     runtime-seed 派生语义

15. **single restored-runtime-to-launch ownership 也继续前移**
   - `RestoredSingleGenerationRuntimeState` 现在不仅 materialize：
     - startup snapshot
     - restored runtime-state payload
     - seed source
     也直接拥有 restored runtime -> runtime launch 的 request-runtime
     投影边界
   - `into_startup_runtime_launch_parts(...)` 不再要求 caller 再把
     `request_runtime_state` 传回 restored owner，才能生成 launch input
   - 这条 seam 的意义在于：single restored runtime owner -> runtime launch
     现在形成了更连续的 launch owner 边界，caller 不再保留这条
     owner-materialized 之后的 request-runtime replay 语义

16. 因此当前 single startup / restored-runtime lane 的补充 stop-rule 再加一条：
   - **不要**在 `RestoredSingleGenerationRuntimeState` 已经完整
     materialize 之后，仍让邻近 caller / helper 在 owner 外部继续重放
     request-runtime -> launch-input 派生语义

17. **resume restored-runtime ownership 也继续前移**
   - `RestoredResumeRuntimeStateProjection` 现在不仅是 shared runtime owner
     给出的 restored-state projection，也直接拥有：
     - persisted-runtime-context -> restored resume projection 的构造边界
     - manual-review blocker 判定边界
   - `chapter_batch_generation_resume_task_command_service.rs` 不再保留
     本地 `RestoredResumeRuntimeState` wrapper 去二次包裹：
     - restored quality-status context
     - restored request runtime state
     - restored resume runtime-state seed
   - 这条 seam 的意义在于：batch resume persisted runtime owner ->
     restored runtime owner 现在形成了更连续的 restored-state owner
     边界，resume command 邻层不再保留这条 owner-materialized 之后的
     local restored wrapper / manual-review replay 语义

18. 因此当前 batch resume / restored-runtime lane 的补充 stop-rule 再加一条：
   - **不要**在 `BatchGenerationPersistedRuntimeContext` /
     `RestoredResumeRuntimeStateProjection` 已经完整 materialize 之后，
     仍让邻近 command owner / helper 在 owner 外部继续重放
     restored-state wrapper 或 manual-review blocker 派生语义

19. **resume reset-checkpoint ownership 也继续前移**
   - `ResumeResetSemantics` 现在不仅 materialize：
     - reset status
     - reset chapter-position semantics
     - progress-total semantics
     也直接拥有 seeded resume checkpoint 的投影边界：
     `build_resume_checkpoint_with_seed(...)`
   - `BatchGenerationResumeResetPersistencePlan::from_resume_task(...)`
     不再在 reset owner 已构造后，继续从 raw
     `ResumeBatchGenerationCommandState` 重放同一条
     resume checkpoint + seed merge 语义
   - 这条 seam 的意义在于：batch resume reset owner ->
     seeded checkpoint 现在形成了更连续的 reset owner 边界，runtime-state
     persistence 邻层不再保留这条 owner-materialized 之后的 local checkpoint
     rebuild 语义

20. 因此当前 batch resume / reset-checkpoint lane 的补充 stop-rule 再加一条：
   - **不要**在 `ResumeResetSemantics` 已经完整 materialize 之后，
     仍让邻近 runtime-state owner / helper 在 owner 外部继续重放
     resume checkpoint seed merge 或 reset-checkpoint 派生语义

21. **resume reset-persistence ownership 也继续前移**
   - `BatchGenerationResumeResetPersistencePlan` 现在不仅 materialize：
     - seeded resume checkpoint
     - task reset mutation contract
     - resume snapshot replace contract
   - `persist(...)` 不再在 owner 已构造后，重新读取 snapshot 并本地重组
     replace-runtime-state write contract
   - `prepare_from_validated_execution(...)` 现在直接把 snapshot 侧已有的
     `workflow_runtime_state` 交给 reset-persistence owner
   - 这条 seam 的意义在于：batch resume reset owner ->
     reset persistence 现在形成了更连续的 persisted-source owner 边界，
     persistence 邻层不再保留这条 owner-materialized 之后的
     local snapshot reload / replace-contract rebuild 语义

22. 因此当前 batch resume / reset-persistence lane 的补充 stop-rule 再加一条：
   - **不要**在 `BatchGenerationResumeResetPersistencePlan` 已经完整
     materialize 之后，仍让邻近 persistence owner / helper 在 owner 外部继续
     重放 existing snapshot reload 或 replace-runtime-state 派生语义

23. **owned-load prepare ownership 也继续前移**
   - `prepare_owned_batch_generation_resume(...)` 现在直接拥有：
     - owned task load
     - `ResumeBatchGenerationCommandState` projection
     - snapshot load
     - handoff into resume launch-persistence prepare
   - `resume_owned_batch_generation_write_workflow(...)` 不再在 workflow 邻层
     本地加载 task + snapshot 后，再把这些 source 回传给 resume command owner
   - 这条 seam 的意义在于：route/write-workflow -> resume command owner
     现在形成了更连续的 owned-source prepare 边界，workflow 邻层不再保留
     这条 owner 已存在之后的 local owned-load replay 语义

24. 因此当前 batch resume / owned-load-prepare lane 的补充 stop-rule 再加一条：
   - **不要**在 batch resume command owner 已经完整接手
     task / snapshot source 之后，仍让邻近 workflow owner / helper 在 owner
     外部继续重放 owned task load、snapshot load、command-state projection
     再回传给同一条 resume owner 链

25. **cancel owned-response persistence ownership 也继续前移**
   - `BatchGenerationCancelledPersistencePlan` 现在直接拥有：
     - cancelled task projection
     - merged cancelled runtime checkpoint
     - response-ready status payload
   - `BatchGenerationReadContext::from_task_and_snapshot_projection(...)`
     现在允许 read/status/stream 语义直接从 owner-projected task +
     runtime-state 视图构造，而不必只依赖 DB reload
   - `cancel_owned_batch_generation_task(...)` 不再在 cancel 邻层先落库，
     再通过 `load_owned_batch_generation_status_payload(...)` 重走一遍
     owned read-context/status payload 链路来构造当前这次 cancel 的立即响应
   - 这条 seam 的意义在于：cancel write lane -> read/status payload owner
     现在形成了更连续的 response-ready owner 边界，cancel 邻层不再保留
     这条 owner 已存在之后的 local persist-then-reload-status-payload 语义

26. 因此当前 batch cancel / owned-response-persistence lane 的补充 stop-rule 再加一条：
   - **不要**在 cancel owner 已经完整 materialize：
     - task terminal projection
     - merged cancelled checkpoint
     - response-ready status payload
     之后，仍让邻近 workflow owner / helper 在 owner 外部继续先 persist，
     再 reload 同一条 owned read-context/status payload 链来返回当前这次
     cancel 的即时响应

   - 如果当前变更已经同时拥有：
     - task source
     - snapshot source
     - runtime checkpoint merge contract
     - shared status payload owner
     那么优先把即时响应直接留在同一条 owner 链上，而不是回退到
     “持久化完成后再查一次”的 Python 时代回读路径

27. **batch cancel final response envelope 也要继续留在同一条 owner 链**
   - 如果 cancel runtime/write owner 已经拥有：
     - merged cancelled runtime state
     - response-ready cancelled status payload
     - task progress source
   - **不要**再让邻近 `cancel workflow` / helper 在 owner 外部：
     - 本地重建 `BatchGenerationCommandProgressSummary`
     - 再补一层 `Batch generation cancelled` summary fields
   - 这条 seam 的判断标准不是“字段少不少”，而是：
     - final cancel response envelope 是否仍在 workflow 邻层本地重组
     - 还是已经由同一条 runtime/write owner 直接投影
   - 一旦 owner 已经同时拥有 cancelled status payload 和 progress source，
     就优先把最终 response envelope 也留在 owner 内，而不是保留一条
     Python 时代 `owner-ready payload -> local summary extend` 的旧 hop

### 2026-06-03 阶段补充：`settings` probe failure contract 已进入真实 Rust owner 收口

当前 Phase 5 的 `settings` lane 不再只覆盖 success-path transport
语义，失败路径的 probe debugging contract 也已开始由 Rust 真实接管：

1. `settings/test` 与 `check-function-calling` 在 Rust owner 路径下，
   probe 失败时也会返回 `details.transport_diagnostics`，不再把
   transport attempt / failover evidence 只保留在 success shell。
2. 这轮收口不是单纯 response shell 美化，而是把 Python 已有的
   “失败时也能用 transport diagnostics 判断 proxy / gateway /
   base-url 漂移”的合同迁入 Rust。
3. 同轮还收掉了一条更细的 candidate drift：
   当配置的 base URL 已经以 `/v1` 结尾时，Rust 不再继续尝试 root
   candidate；这里要以 Python 当前
   `_build_chat_completions_base_url_candidates(...)` 为准，而不是以
   早期 Rust 测试假设为准。
4. 因此后续对 `settings` lane 的判断规则需要更新：
   - 不再把 `transport_diagnostics` 视为 success-only metadata
   - failure shell parity 也属于真实 owner seam
   - 旧测试如果仍假设 `/v1 -> root` fallback，应视为合同漂移而不是
     迁移目标

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

##### 5.2.8 2026-06-04 阶段补充：`chapter_generation` batch stream read-context owner 继续收口

截至 2026-06-04，Phase 5 在 `chapter_generation` 邻域继续沿
`batch read-context -> status-stream` 这一条 owner seam 链推进，而不是回到
route shell 或无关 lane。

本轮新增的真实 Rust seam 收口点是：

1. `backend-rs/src/services/chapter_batch_generation_read_context_service.rs`
   新增 `BatchGenerationReadContext::into_stream_state()`。
2. `backend-rs/src/services/chapter_batch_generation_status_stream_service.rs`
   的 `load_owned_batch_generation_status_stream(...)` 及其 polling reload
   路径，改为直接消费上述 owner projection。

这轮为什么算真实迁移，而不是 helper shuffle：

1. 在这之前，batch stream lane 已经有共享的
   `BatchGenerationReadContext` owner，且它已经持有：
   - `task`
   - `workflow_runtime_state`
   - `quality_status_context`
2. 但 status-stream caller 仍然在 owner materialize 之后，本地再次执行：
   - `BatchGenerationStreamState::from_task_state_with_quality_context(...)`
   - 重新穿透 `workflow_runtime_state.as_ref()`
   - 重新穿透 `Some(&quality_status_context)`
3. 且这套 caller-side projection 同时存在于：
   - 初次 stream load
   - polling reload

因此，这不是单纯“把一段 match/if 挪位置”，而是继续消除
**owner 已存在，但 caller 仍重组 persisted read-side / stream semantic**
的旧边界。

本轮收口后的规划含义：

1. Phase 5 的 `chapter_generation` Rust 开发，仍然应该优先围绕
   `chapter_batch_generation` / `chapter_single_generation` / shared
   read-context owner 这条链继续推进。
2. 当前更高价值的下一步，仍然不是新增 route group，也不是泛化全仓清理，
   而是继续找：
   - owner 已经 materialize
   - caller 仍本地重组 compat payload / stream state / lifecycle state
   的真实 seam。
3. 这类 seam 虽然切片小，但它们直接提升：
   - cutover 审计性
   - read-side / stream semantic 一致性
   - Python-era caller-side payload rebuild 的收敛度

##### 5.2.9 2026-06-04 阶段补充：single existing-background compat shell 已完成最终 owner 收口

同日继续沿 `chapter_generation` 的 single existing-background lane 推进后，
这条 read-side seam 又完成了一次更细但真实的 Rust owner 收口。

本轮新增的真实收口点是：

1. `estimated_task_minutes(...)` 不再挂在
   `chapter_batch_generation_write_workflow_service.rs` 的写入链内部，
   而是提升到共享的 task payload/base semantic 层。
2. `BatchGenerationReadContext::into_single_generation_existing_background_task_payload()`
   改为直接产出最终 existing-background compat payload，不再要求 caller
   先从同一个 task 再本地计算 `estimated_time_minutes` 后回填。
3. `chapter_single_generation_write_workflow_service.rs` 中的
   `single_generation_existing_background_task_payload(...)` 现在只做 owner
   delegation，不再本地完成最后一层 compat shell。

这轮为什么算真实迁移，而不是“把一个函数挪地方”：

1. 在前一轮 `single-existing-background-read-context-owner` 完成后，
   shared read-context owner 已经持有该 payload 所需的核心状态：
   - `task`
   - `workflow_runtime_state`
   - `quality_status_context`
2. 但 caller 仍然在 owner materialize 之后，再从同一个 `task` 读取：
   - `target_word_count`
   - `enable_analysis`
   并本地重算 `estimated_time_minutes`
3. 这意味着 single existing-background lane 仍残留一个
   **owner 已存在，但 caller 继续完成 compat payload 最后一层语义**
   的旧边界

本轮收口后的规划含义：

1. `chapter_generation` Phase 5 的有效推进，仍然主要发生在
   `shared read-context owner` 邻域，而不是 route shell 数量变化上。
2. 当前 Rust 迁移的高价值切片，已经从“把大块逻辑搬进服务”继续下沉到：
   - 消除 owner 已 materialize 后的 caller-side ETA / payload rebuild
   - 让 read-side / stream-side 的 compat contract 更完整地归属 Rust owner
3. 因此下一步仍应优先检查：
   - `task_view_query`
   - `read_context`
   - `single existing-background`
   这条链上是否还残留 owner 已存在后的本地 payload/state completion。

##### 5.2.10 2026-06-04 阶段补充：resume response compat shell 已回收到 reset persistence owner

同一轮继续推进后，`chapter_generation` 的 resume lane 也完成了一条新的真实
Rust seam 收口，而且这条 seam 比简单的 read-side list/query 壳更有价值。

本轮新增的真实收口点是：

1. `BatchGenerationResumeResetPersistencePlan` 不再只拥有 reset checkpoint
   与持久化语义，还直接拥有最终 resume response payload projection。
2. `chapter_batch_generation_resume_task_command_service.rs` 不再通过
   `command_state + reset_persistence_plan` 在 caller 侧手工补齐最终 compat
   response shell。

这轮为什么算真实迁移，而不是 helper 搬运：

1. 在这之前，resume lane 已经有明确的 reset owner：
   `BatchGenerationResumeResetPersistencePlan` 已持有：
   - reset status / chapter pointer
   - persisted resume checkpoint
   - restored quality/runtime context projection入口
2. 但 caller 仍在 owner materialize 之后，本地再次做了：
   - outer runtime payload shell 组装
   - single/batch quality payload 分支补齐
   - active story repair payload / quality history context 插回
   - resume stage fields / `resumed_from_batch_id` patch
3. 这意味着 resume lane 仍残留一个
   **owner 已存在，但 caller 继续完成 compat response 最后一层语义**
   的旧边界

本轮收口后的规划含义：

1. Phase 5 的高价值 seam 已经不仅限于 `read_context` 邻域，也明确扩展到
   `resume persisted-runtime -> reset persistence -> compat response`
   这一条生命周期链。
2. 后续继续加速 Rust 迁移时，优先级应继续偏向：
   - owner 已存在
   - caller 仍补最终 response/payload/state shell
   的链路，而不是去做低信号 JSON 壳搬运。
3. 这条 resume seam 收掉后，下一步更值得继续检查：
   - `resume` lane 是否还残留 execution-plan / persisted-runtime caller-side
     completion
   - `read_context` / `stream` / `resume` 三条链之间是否还有共享 compat
     payload 语义可继续归并为 owner contract

##### 5.2.7 2026-06-02 阶段校准：为什么持续要求加速，体感仍然慢

截至 2026-06-02，关于“为什么已经持续推进、也多次要求加速，但整体体感
仍然慢”的原因已经进一步收敛。这里需要把**工程上真实发生的推进**与
**用户主观能感知到的阶段跳变**明确拆开：

1. **当前大量工作是真实迁移，但多数仍属于线 A 的 owner / seam 收口。**
   - 这些切片在 Rust 侧持续减少 Python 语义依赖，尤其是 `chapters`、
     `outline`、`quality-trend` 等高价值邻域。
   - 但它们更多改善的是“Rust 内部 owner 一致性”和“compat drift 风险”，
     不会自动转化成“某个 route group 已可以缩小 fallback 面”。
   - 结果是：代码持续在前进，但 route-group cutover 的阶段跳变较少。

2. **当前完成定义与“迁移速度感”的完成定义并不一致。**
   - 工程上的完成，通常是：
     - 新增一个真实 Rust owner
     - focused tests 通过
     - `cargo check` 通过
     - 外部行为不变
   - 但更直观的“速度感”往往来自：
     - 某个 route group 可进入 shrink 决策
     - 某块 Python fallback 明显缩小
     - 某层 schema owner 开始脱离 Python
   - 当两套完成定义长期不一致时，就会出现“持续在做事，但整体看起来不快”。

3. **验证成本高，且当前切片模型会重复支付这笔成本。**
   - 当前大多数切片都要求：
     - focused test
     - `cargo check`
     - 行为不变复核
   - 这对 shared DB、SSE、任务状态、checkpoint 恢复等高兼容风险链路是必要的，
     但也意味着“每一刀都很小、每一刀都要重新验证”。
   - 结果是：质量稳定，但吞吐量天然低于“粗放删 Python”。

4. **真正决定速度感的 B/C 线资产还没有与线 A 同步形成批次交付。**
   - 当前最强的 B 线资产已经集中在 `settings` / `projects`，但过去多轮推进
     之后，它们仍主要停留在：
     - owner / fallback probe
     - rollback runbook
     - parity / checklist 文档
   - 如果没有把这些资产继续收束成“可直接决策”的 shrink-ready 包，
     就会持续出现“证据很多，但还不能快速 cutover”的状态。
   - 同时，schema / migration owner 仍由 Python Alembic 显式承担，这进一步
     压制了整体去 Python 化速度感。

因此，2026-06-02 之后的提速目标不应再表述为“写更多 Rust seam”，而应改成：

1. **保持真实 seam 收口，但要求每轮都尽量服务某个 cutover 资产。**
2. **把 `settings` / `projects` 从“已有大量 probe”推进成“可直接读的 shrink-ready 批次包”。**
3. **把 schema / migration owner 继续从文档债务前移为执行线，而不是只保留在分析层。**

补充校准：

- 2026-06-03 在 `settings` Phase 5 lane 上，已经不再只停留在
  owner/fallback probe 或 response shell 对齐。
- 本轮新增的是一条真实 transport owner 收口：
  `POST /api/settings/test` 与
  `POST /api/settings/check-function-calling`
  现在由 Rust owner 直接承担最小 probe request-options 语义，
  包括：
  - openai-compatible root base URL 优先切到 normalized `/v1` candidate
  - 稳定 `details.transport_diagnostics.events/attempts/summary` 外壳
  - focused Rust tests 证明 custom root base URL 能命中 `/v1/chat/completions`
- 这类 slice 仍然属于“真实 seam 收口”，但它开始更直接服务
  `settings` route-group 的 shrink readiness，而不是只增加外围治理资产。

规划含义：

- 后续再汇报“推进很快/很慢”时，必须说明究竟是：
  - Rust seam 收口速度
  - fallback shrink readiness 形成速度
  - schema owner 切换速度
- 不再把“又完成了一个真实 Rust seam”直接等价为“整体去 Python 化快了很多”。

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

##### 5.5.5 2026-05-31 schema owner 显式化进展

本轮已完成一条小但真实的 schema / migration owner 收口：

1. `backend-rs/src/config.rs` 不再把
   `ENABLE_STARTUP_SCHEMA_SYNC=true` 视为可忽略的普通配置。
2. 在 **non-development** 模式下，Rust 启动现在会显式拒绝该配置并启动失败。
3. 在 **development** 模式下，该标志会被告警并归一化回 `false`，
   保持本地容错但不再暗示 Rust 拥有实际 schema mutation 能力。
4. `backend-rs/src/main.rs` 里原先“仅 warning 后继续启动”的软约束
   已被移除，当前 owner 边界改为由配置校验直接执行。

这条收口的意义不是“Rust 已接管 migration”，而是：

- 把“shared DB strangler 下 schema owner 仍是 Python Alembic”
  从文档约定提升为启动期可执行约束
- 防止后续部署把无效的 Rust schema-sync 标志继续带入生产路径
- 为后续 Phase 5 的 migration checklist / deploy verification
  提供更清晰的 owner 基线

##### 5.5.6 Phase 5 加速执行矩阵（2026-05-31）

从本节开始，Phase 5 的推进不再只记录“某个 Rust seam 又收窄了一点”，
而要把每个 route group 放进可执行矩阵里判断是否能进入 fallback 收缩准备。

判断口径固定为五项：

1. Rust owner 是否已通过 `backend-rs/src/api/router.rs` 和 gateway path 生效。
2. Python fallback 是否仍通过 `backend/app/bootstrap/router_registry.py` 或
   Nginx catch-all 可达。
3. `deploy/strangler-gateway-probes.json` 是否同时具备 Rust owner 与
   Python fallback / asymmetric 探针。
4. rollback 是否能通过当前 Nginx route 规则恢复到 Python。
5. schema / migration assumption 是否清楚，且不依赖 Rust 隐式建表。

| route group | Rust owner 证据 | Python fallback 证据 | smoke/probe 状态 | rollback / schema 假设 | 加速通道 | 下一步动作 |
| --- | --- | --- | --- | --- | --- | --- |
| `settings` | Rust router 已 merge，Nginx 明确路由 `/api/settings*` 到 Rust。 | Python 仍注册 `settings` router，`/api/` catch-all 仍可回 Python。 | 已有 `settings` Rust owner 与 Python fallback probes；preset 读取与 create/update/delete/from-current/activate/test 低前提边界已补齐；并已固化 `phase5-settings-owner` / `phase5-settings-fallback` / `phase5-settings-asymmetric` 三个专用 profile。2026-06-02 又补入首条登录态 business smoke：`GET /api/settings` 的 Rust/Python 双侧 `settings-get-business-*` probes，并为公共 smoke runner 增加本地登录会话/cookie 复用能力。 | schema owner 仍按 Python Alembic；rollback 可通过恢复 Nginx settings path 到 Python；preset 仍写入 `preferences` JSON，不新增 schema owner；`settings/models` 是非对称 public/network-error 边界。当前真实 business smoke 实跑还依赖 `LOCAL_AUTH_USERNAME / LOCAL_AUTH_PASSWORD` 环境前提。 | 第一批 fallback shrink readiness。 | 继续从登录态只读 smoke 扩展到 preset CRUD / activate / provider-test 成功态，并把 shrink 决策从“有 probe”推进到“有真实登录态 owner/fallback 证据”。 |
| `projects` | Rust router 已 merge，Nginx 明确路由主要 `/api/projects*` 到 Rust。 | Python 仍注册 `projects` router，fallback 探针仍存在。 | 已有 projects Rust owner / fallback probes，现覆盖基础 CRUD、列表/详情、public import validation、multipart import、TXT/JSON 两类导出和维护修复入口；并已固化 `phase5-projects-owner` / `phase5-projects-fallback` 两个专用 shrink-readiness profile；业务 smoke 仍需登录态加强。 | 项目表 schema 仍不由 Rust mutation；rollback 走 Nginx path 级回退。 | 第一批 fallback shrink readiness。 | 进入 projects 登录态 business smoke 与 shrink 决策，不再继续只扩未登录边界。 |
| `wizard-stream` | Rust router 已 merge，Nginx 已把多个具体 `/api/wizard-stream/*` SSE path 指到 Rust。 | `/api/wizard/` 与宽泛 `/api/wizard-stream/` 仍指向 Python；`world-building` 基础入口与 `regenerate` 已有同路径 fallback 证据。 | 已有 Rust 与 Python fallback probes，现覆盖 `outline`、`world-building`、`world-building/{project_id}/regenerate`、`career-system`、`characters`；`cleanup` 暂为 Rust-only owner 证据。 | SSE rollback 必须保留 `sse-common` 配置；schema 依赖既有 project/world-building 数据；当前 probes 仍以未登录边界为主。 | 边界澄清优先。 | 继续枚举宽泛 Python fallback 下仍未被 Rust 精确覆盖的 path；下一步补登录态 SSE stronger smoke，而不是把未登录边界误判为业务等价。 |
| `memories` | Rust router 已 merge，Nginx 明确 `/api/memories/` 到 Rust。 | Python 仍注册 `/memories/` 非 `/api` path，Nginx 明确保留。 | 已有 memories Rust owner / fallback probes。 | schema 仍沿用 Python 历史表；rollback 需区分 `/api/memories/` 与 `/memories/`。 | path 边界整理。 | 先确认 `/memories/` 是否仍是前端/兼容入口；若不是，转入 deprecation 或 redirect 方案。 |
| `chapters` | Rust router 已 merge，Nginx 将章节 CRUD / generation / batch-generation path 指到 Rust。 | Python 仍注册多个 `chapter_*` routers，fallback probes 覆盖部分路径。 | owner/fallback probes 已有，但行为风险最高。 | schema 与任务状态表仍按 Python Alembic；rollback 必须保留 batch/SSE 兼容路径。 | 高价值 seam + stronger smoke。 | 不作为第一批移除 fallback；继续补 batch-generation status/SSE/read-side parity 和 stronger smoke。 |
| `auth` / `users` | Rust router 已 merge，Nginx 明确 `/api/auth/`、`/api/users/` 到 Rust。 | Python 仍注册 auth/users，fallback probes 充足。 | Rust owner / fallback probes 充足。 | 安全敏感；rollback 需验证 cookie/header/JWT 行为一致。 | P1 cutover 包。 | 等 settings/projects 先跑通 shrink 流程后，再推进 auth/users。 |
| `characters` / `outlines` / `book_import` / `relationships` / `foreshadows` / `writing_styles` / `organizations` / `careers` / `inspiration` / `mcp_plugins` / `prompt_templates` / `background_tasks` / `prompt_workshop` / `polish` / `changelog` | Rust router 已 merge，Nginx 明确主要 path 到 Rust。 | Python routers 与 fallback probes 仍存在；`changelog` 是 public GitHub proxy fallback，真实 smoke 受外部网络影响。 | characters/outlines/book_import 已有 owner/fallback probes；relationships 已补第一组 project-list / graph probe；foreshadows 已补第一组 project-list / stats probe；writing_styles 已补第一组 user / project probe；organizations 已补第一组 project-list / generate-stream probe；careers 已补第一组 list / generate-system probe；inspiration 已补第一组 generate-options / quick-generate probe；mcp_plugins 已补第一组 list / simple-create probe；prompt_templates 已补第一组 list / system-defaults probe；background_tasks 已补第一组 list / create probe；prompt_workshop 已补第一组 submit / like probe；polish 已补第一组 text / batch probe；changelog 已补 list / refresh public probe；业务覆盖仍分散。 | schema 仍归 Python Alembic；rollback 可 path 级恢复；relationships graph 还依赖组织成员与角色节点 join 语义；foreshadows 依赖章节上下文和 pending/overdue 查询语义；writing_styles 依赖 preset 同步与 project default style 语义；organizations 依赖 `organization_members`、组织角色映射与 generation_history 语义；careers 依赖 `careers` 与 `character_careers` 表、SSE 生成和角色职业绑定语义；inspiration 依赖 prompt template、AI provider 与可选 web research 语义；mcp_plugins 依赖 MCP client session 与后台注册/断开语义；prompt_templates 依赖 managed template sync 与 prompt formatting 语义；background_tasks 依赖 task registry / stream hub 语义；prompt_workshop 混合公开与登录接口且依赖云端代理模式；polish 依赖 provider 配置与 generation_history 写入；changelog 依赖 GitHub API 可用性与缓存语义。 | P1 批量包。 | 在 P0 路径形成模板后，批量补 checklist 与 stronger smoke；changelog 若进入真实 smoke 需隔离网络波动。 |
| `ai_test / ai` | Rust router 已 merge，Nginx 明确 `/api/ai-test` 与 `/api/ai/` 到 Rust。 | 当前仓库未发现对应 Python router，不能按普通同路径 fallback 统计。 | 已补 `POST /api/ai-test` 与 `POST /api/ai/test` 两条 Rust auth-boundary asymmetric probe。 | provider 配置、超时策略、SSE 行为仍需后续 stronger smoke；若产品决定保留 fallback，需要先补出明确 Python owner，否则按 Rust-only 或禁用策略处理。 | P1 asymmetric。 | 先确认是否仍需 Python fallback；若不需要，后续补 stream auth-boundary 和登录态不可达 provider failure smoke。 |

本矩阵给出的提速结论：

1. **第一批不应从 `chapters` 移除 fallback 开始。**
   `chapters` 的 Rust owner 已强，但行为面最复杂，应继续作为 seam parity 与
   stronger smoke 的主战场。
2. **第一批 fallback shrink readiness 应从 `settings` / `projects` 开始。**
   这两组 Rust owner、fallback probes、rollback 路径都更清晰，适合作为模板。
3. **`wizard-stream` 与 `memories` 先做边界收窄。**
   它们不是简单“已迁移/未迁移”，而是精确 Rust path 与宽泛 Python path 并存。
4. **schema owner 仍是所有 fallback 收缩的共同前置假设。**
   当前已禁止 Rust 启动期隐式 schema sync，但还没有完成 migration owner 切换；
   因此任何 fallback 收缩包都必须显式写明“不改变 schema owner”。
5. **后续每轮开发至少选择一个矩阵单元推进到下一状态。**
   例如：`settings` checklist、`projects` stronger smoke、`chapters` parity seam、
   `wizard-stream` fallback path 枚举，不能继续只做孤立 helper 移动。

- 2026-06-03 在 `chapters` / `chapter_generation` 的 Phase 5 seam lane 上，
  又完成了一条直接服务 cutover 的真实 Rust owner 收口：
  - `backend-rs/src/services/chapter_single_generation_prepare_service.rs`
    现在新增
    `load_single_chapter_generation_target(...)`，
    统一拥有：
    - chapter access 校验
    - generation prerequisite gate
    - `SingleChapterGenerationTarget` 投影
  - `backend-rs/src/services/chapter_single_generation_write_workflow_service.rs`
    的
    `start_owned_single_generation_background_write_workflow(...)`
    不再本地重复 chapter load / prerequisite / target projection，
    而是直接消费 prepare owner 给出的 target
  - 这条 seam 不是单纯“抽出一个 helper”，而是把单章 background write
    lane 上最后一段重复的 owner 决策从 write workflow 收回到了 prepare owner，
    让 write lane 更接近纯 startup persistence + dispatch 边界
  - 对 Phase 5 的意义是：
    - `chapters` lane 的 Rust seam 收口继续服务于 cutover readiness，
      而不是停留在局部代码美化
    - 单章 background lane 的 access/prerequisite owner 现在更容易和
      stream / resume / batch 相邻路径做一致性审计
    - 后续若继续推进 `chapter_generation` / `chapter_batch_generation`
      的 startup-to-runtime 生命周期统一，这条 seam 已经清掉了一块真实阻力
  - 该 slice 已通过：
    - `cargo test chapter_single_generation_prepare_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`
    - `cargo test chapter_single_generation_write_workflow_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`
    - `cargo check --manifest-path "backend-rs/Cargo.toml"`

补充判断：

- `chapters` lane 仍然不适合作为第一批 fallback shrink 目标，
  但它继续保持“高价值 seam + stronger smoke”主战场的定位是正确的。
- 本轮收口后，单章 background write lane 的下一步应优先考虑：
  1. 与 batch write lane 继续对齐 startup-to-runtime 生命周期 owner；
  2. 或继续压缩仍留在 write-workflow-local 的非 cutover 关键 launch 组装。
- 只有当这些 seam 与 stronger smoke / rollback / schema assumption 的证据
  继续同步补强时，`chapters` 才会从“owner 覆盖高但行为风险高”
  逐步推进到真正可评估 shrink readiness 的状态。

2026-06-01 进展补记：

- `settings` 已补入 `GET /api/settings/presets` 的 Rust owner 与 Python
  fallback 双侧 probe，并同步 ownership checklist / rollback runbook。
- `settings` 进一步补入 preset 管理写侧低前提边界：
  `POST /api/settings/presets`、`PUT /api/settings/presets/{preset_id}`、
  `DELETE /api/settings/presets/{preset_id}`、
  `POST /api/settings/presets/from-current`、
  `POST /api/settings/presets/{preset_id}/activate` 与
  `POST /api/settings/presets/{preset_id}/test`。该组现在已从 preset 读取扩展到
  主要 preset 管理入口的 owner/fallback 可切换证据，但仍不代表登录态
  `preferences` JSON 写入、激活应用主字段或 provider 测试结果完整等价。
- `settings` 已进一步固化为三个一键执行 profile：
  `phase5-settings-owner`、`phase5-settings-fallback` 与
  `phase5-settings-asymmetric`。其中普通 owner 包 13 条，fallback 包 12 条，
  models 非对称包 2 条；这让 `settings` 的第一批 fallback shrink readiness
  从“需要组合 `phase5-p0 + --route-group settings` 参数”推进到 manifest 内
  显式 profile。下一步应补登录态 preset CRUD / activate / provider-test
  business smoke，而不是继续只增加未登录边界 probe。
- 2026-06-02 进一步推进后，`backend/tools/run_strangler_gateway_smoke.py`
  已补齐公共本地登录 bootstrap 能力：支持从 `.env` / 环境变量 / CLI 读取
  `LOCAL_AUTH_USERNAME` 与 `LOCAL_AUTH_PASSWORD`，并在 probe 声明
  `requires_login=true` 时自动建立 cookie jar、复用登录态请求、校验
  `user_id/session_expire_at` 以及 Rust owner 所需 `token` cookie。
  基于这层公共能力，`settings` 首次补入真实登录态读侧 business smoke：
  `settings-get-business-rust` 与 `settings-get-business-python-fallback`，
  共同断言 `GET /api/settings` 返回 `id/user_id/api_provider/has_api_key/llm_model`
  这些稳定外壳字段。当前真实实跑已验证命令链路可执行，但若本地 `.env`
  未提供 `LOCAL_AUTH_USERNAME / LOCAL_AUTH_PASSWORD`，则仍会停在环境前提缺口，
  不应误判为代码未完成。
- 2026-06-02 同日继续推进后，`settings` 又补入第二组登录态 business smoke：
  `settings-presets-get-business-rust` 与
  `settings-presets-get-business-python-fallback`。它们共同断言
  `GET /api/settings/presets` 返回 `presets/total/active_preset_id`，把
  `settings` 的真实登录态证据从根设置读取扩展到 `preferences` JSON 内的
  preset 读取入口。与之配套，Rust `settings` preset owner 也进一步收口：
  preset 读取 / 创建 / 更新 / 删除 / 激活 / from-current 现在会在缺少
  `settings` 记录时自动创建默认设置，不再暴露 `settings not found`；
  同时对齐了“激活中预设不可删除”的业务保护、activate 返回体以及
  preset create/create-from-current 的 `200` 契约。这让 `settings`
  shrink-readiness 从“有第一条登录态读侧 smoke”推进到“preset lane 也已有
  真实业务 handler 证据与一处真实 Python 业务语义迁移”。
- 2026-06-02 同日继续推进后，`settings` 主设置写入链路也完成了第二轮真实
  Python -> Rust 契约迁移。Rust 不再把 `POST /api/settings` 与
  `PUT /api/settings` 压成同一个宽松 upsert 路径，而是显式拆成：
  - `POST /api/settings` 保持 Python 风格 upsert
  - `PUT /api/settings` 改为 existing-only，并在缺失设置时返回
    `404 {"detail":"设置不存在，请先创建设置"}`
  - `DELETE /api/settings` 在缺失设置时返回
    `404 {"detail":"设置不存在"}`
  同时，Rust `POST /api/settings` 现在也对齐 Python 的一条关键业务联动：
  当用户手动修改当前 provider/key/base-url/model/temperature/max_tokens
  使其偏离激活中的 preset config 时，会自动取消该 preset 的激活状态。
  这说明 `settings` route-group 的真实迁移已经从 preset lane 延伸到
  base save/update/delete contract，不再只停留在 probe 或只读证据层。
- 2026-06-02 同日继续推进后，`settings` preset action lane 也完成了下一轮
  真实 Python -> Rust 契约收口。Rust 现在不再把 preset config 里所有扩展
  provider 字段都回灌回主设置，也不再把当前 `settings` 行的扩展 provider
  状态原样快照进 `from-current` preset：
  - `POST /api/settings/presets/{preset_id}/activate` 现在只回写 Python 真正
    owner 的主设置字段：`api_provider`、`api_key`、`api_base_url`、
    `llm_model`、`temperature`、`max_tokens`、`system_prompt`
  - Rust 不再借由 preset activate 隐式改写 `api_backup_urls`、
    `provider_type`、`fallback_strategy`、`azure_api_version`
  - `POST /api/settings/presets/from-current` 现在也改为生成 Python-shaped
    snapshot config，而不是把当前 Rust settings 扩展字段原样写入 preset：
    `api_backup_urls=null`、`provider_type="openai"`、
    `fallback_strategy="auto"`、`azure_api_version=null`
  - 同时保留 Python 当前动作语义允许的空值行为：preset activate 可把
    空字符串 `api_key` / `llm_model` 与空 `system_prompt` 应回主设置
  这说明 `settings` 的真实迁移不再只停留在 preset CRUD shell 或 base
  settings write contract，而是已经开始收口 preset action owner 本身。
- 2026-06-03 继续推进后，`settings/models` 也完成了一轮更接近 Python 的
  provider-specific success-path 契约迁移。Rust 现在不再把这条路由主要当成
  “generic openai `/models` + curated Anthropic fallback”的自有实现，而是开始
  对齐 Python 已经稳定的 provider owner：
  - openai-compatible providers 现在按 Python 风格尝试 candidate URL：
    base `/models`，以及 root 或 `/v1/models` 的后续候选；早期候选的
    `404`/连接失败不再直接终止
  - Azure 现在在 `/settings/models` 使用 `api-key` header，并在
    `404/403` 或空结果时返回 `200 + [] + 友好 message`
  - Anthropic 不再返回 Rust 侧静态 curated 列表，而是改为真实调用
    `{base}/v1/models`，并携带 `x-api-key` 与 `anthropic-version`
  - Gemini 现在只保留 `supportedGenerationMethods` 含 `generateContent`
    的模型，避免把非生成模型暴露给前端选择
  这说明 `settings` 的真实迁移已经从 preset / write 契约继续推进到模型发现
  success path 本身；后续最高信号缺口更聚焦于 `settings/test`、
  `check-function-calling` 与更强的登录态 success smoke，而不是 `models`
  基础 success contract 仍大面积漂移。
- 2026-06-03 同日继续推进后，`settings/check-function-calling` 也完成了一轮
  核心 Python 契约收口。Rust 现在不再把这条路由当成
  “本地占位工具 + tool_calls 才算 success”的简化探测，而是开始对齐
  Python 已稳定依赖的 probe owner：
  - AI 抽象现在新增 `ToolChoice` 能力，并由
    `backend-rs/src/ai/service.rs` 向 OpenAI / Anthropic client 透传；
    同时保留已有 tools 调用在未显式覆盖时的 `auto` 语义
  - `POST /api/settings/check-function-calling` 现在改用 Python 对齐的
    `get_weather` 工具，并强制 `tool_choice = required`
  - 模型如果成功返回但仅输出纯文本，Rust 现在也与 Python 一样保持：
    - `success = true`
    - `supported = false`
    而不再把这类结果误判为整次 probe 失败
  - 成功路径 `details` 现在对齐到 Python 最小稳定外壳：
    `endpoint_diagnostics / finish_reason / has_tool_calls /
    tool_call_count / test_tool / response_type`
  - 失败路径现在也附带 `details.endpoint_diagnostics`，与 Python 的
    timeout/runtime failure 壳层保持一致
  - 已补 focused route tests 与 client-level tool-choice tests，确认
    tool-call 成功态、plain-text unsupported 态与 `required` 序列化都已被
    Rust owner 覆盖
  这说明 `settings` 的真实迁移又从 model-list success path 继续推进到
  function-calling probe 本身；后续最高信号缺口进一步集中到
  `settings/test` 的 transport / diagnostics parity，以及更强的
  登录态 success smoke，而不是 `check-function-calling` 仍停留在核心
  success/error 壳层漂移。
- 2026-06-03 同日继续推进后，`settings/test` 也完成了一轮真实 Python
  probe contract 收口。Rust 现在不再只返回一个本地化的
  “API connection succeeded / failed + 顶层 probe_max_tokens” 壳层，而是开始
  对齐 Python 已稳定依赖的 API-connection probe owner：
  - `POST /api/settings/test` 现在接受 widened probe request body：
    - `api_backup_urls`
    - `fallback_strategy`
  - probe 成功路径现在回到 Python 风格的 `details` 壳层：
    - `api_available`
    - `model_accessible`
    - `response_valid`
    - `temperature`
    - `max_tokens`
    - `probe_max_tokens`
    - `endpoint_diagnostics`
  - `endpoint_diagnostics` 现在开始对齐 Python 的归一化 owner 语义：
    - `backup_endpoints`
    - `configured_endpoint_count`
    - `fallback_strategy`
    - `auto_failover_enabled`
    不再固定为 Rust 本地的 `[] + auto(false)` 占位形态
  - 失败路径现在也附带 Python 风格的 `details.endpoint_diagnostics`
  - `settings/presets/{preset_id}/test` 复用链路现在也会透传
    `api_backup_urls` / `fallback_strategy`，避免 preset probe 继续丢失
    transport 相关字段
  - `check-function-calling` 现在复用同一套 widened probe diagnostics
    helper，因此两条 `settings` probe 子路由不再对 backup/fallback metadata
    产生本地漂移
  这说明 `settings` 的真实迁移又从 function-calling probe 继续推进到
  API-connection probe 本身；后续最高信号缺口进一步收缩到
  `settings/test` 更深层的 transport parity
  （`request_options` / provider-specific probe pathing / transport_diagnostics）
  与更强的登录态 success smoke，而不是这条路由仍停留在核心 response shell
  漂移。
- 2026-06-03 同日继续推进后，`settings/test` /
  `check-function-calling` 的 backup/fallback execution semantics 也开始由
  Rust 真正执行，不再只停留在 widened `endpoint_diagnostics` metadata：
  - Rust `AIConfig` / `AIService` / `OpenAIClient` 现在已接通
    `backup_urls`
  - `fallback_strategy == "auto"` 现在是 Rust probe transport 中真正启用
    backup endpoint failover 的 gate；`manual` 只继续保留在 diagnostics
    shell，不再误触发实际 failover
  - Rust transport 现在已显式拆开三层责任：
    - base URL candidate probing
    - endpoint failover (`primary + backup_urls`)
    - retry budget per endpoint
  - 这也把上一轮为了覆盖 root + normalized `/v1` candidate 而临时把
    `transport_max_retries` 提到 `Some(2)` 的 parity debt 收回到
    Python 对齐的 `transport_max_retries = 1`
  - 已新增 focused tests，锁住：
    - auto fallback 时 primary -> backup 的真实执行顺序
    - manual fallback 时 backup 不应被执行
    - `transport_diagnostics.summary.backup_endpoint_used`
    - `transport_diagnostics.summary.failover_count`
  这说明 `settings` 的真实迁移又从 widened probe shell 继续推进到了
  backup/fallback 的 transport execution owner；后续更高信号缺口已进一步
  收缩到更深层 diagnostics/provider parity 与登录态 success smoke，而不是
  继续停留在 backup/fallback 是否真正执行这一层。
- 2026-06-03 同日再向前收口后，`settings/test` /
  `check-function-calling` 的 openai-compatible probe transport 已继续修掉
  一条更细的 Python owner drift：Rust 不再把 base URL candidate
  continuation rule 和 backup endpoint failover rule 混用。
  - 当前 Rust 已保持两层独立语义：
    - candidate continuation 仍允许处理 `404/405/415/422`、non-JSON、
      base-URL-shape drift、timeout / connection failures
    - backup endpoint failover 只允许处理网络错误、`5xx`、`429`
  - 这意味着 Rust 不会再因为 `404/405/415/422` 或 parse/non-JSON drift
    错误切到 `backup_urls`，而是会像 Python 一样继续尝试下一个 base URL
    candidate
  - 同时 primary `500` 等真实 failover 场景仍保持 Rust owner 能力，不会因
    这次收口而退回到“只会 candidate fallback、不会 backup failover”
  - 已新增 focused tests，锁住：
    - `/v1` candidate `404` 时应继续 root candidate，而不是 fail over 到
      backup endpoint
    - primary `500` 时仍应保持 auto fallback 到 backup endpoint 的行为
  这说明 `settings` probe lane 的 Phase 5 Rust owner 已从“具备 backup/fallback
  执行能力”进一步收敛到“backup failover 触发边界也与 Python 对齐”，后续
  再推进应优先选择 provider-specific probe pathing、登录态 success smoke、
  或确有 shrink 价值的 `transport_diagnostics` 深层 parity，而不是再回到这条
 已完成的 failover 规则边界。
- 2026-06-03 同日继续推进后，`settings/test` /
  `check-function-calling` 又收掉了一条更真实的
  provider-specific probe pathing drift：对于 `sub2api` /
  `openai_responses`，Rust 不再把 root base URL 继续回退到 root
  `/chat/completions` candidate，而是与 Python 当前 owner 一样，只保留
  normalized `/v1` probe candidate。
  - 这条收口不是 generic openai-compatible candidate 偏好的重复实现，
    而是把 Python
    `_build_chat_completions_base_url_candidates(...)` 中已稳定存在的
    compat-profile 语义真正迁进 Rust owner path
  - 之前 Rust 的 drift 在于：虽然 generic openai-compatible providers
    已支持 normalized `/v1` candidate 偏好，但 `sub2api` /
    `openai_responses` 仍会复用 generic fallback 规则，因此在 Python
    应该失败的 probe 上，Rust 仍可能借由 root candidate 产生假阳性 success
  - 本轮之后，这两个 provider profile 的 probe path 都开始遵守 Python 的
    `/v1-only` contract；已新增 focused tests，锁住 normalized `/v1 404`
    时不能再偷偷成功于 root candidate
  - 这说明 `settings` Phase 5 lane 的剩余高信号 seam 已继续收窄：
    后续更值得做的是本地 gateway / docker-host / http fallback candidate
    parity、登录态 provider-success smoke，或确有 shrink 价值的更深层
    diagnostics contract，而不是再回头重复 generic openai-compatible
    candidate 规则
- 2026-06-03 同日继续推进后，`settings/test` /
  `check-function-calling` 的本地 gateway probe candidate contract 也开始由
  Rust 真实接手，不再只停留在 provider-specific `/v1` pathing 对齐：
  - Rust `OpenAIClient` 现在会像 Python 一样，为本地 loopback /
    local gateway probe 构造候选变体：
    - 运行在 Docker 时，`127.0.0.1` / `localhost` 可扩展为
      `host.docker.internal`
    - 本地 `https://127.0.0.1` / `https://localhost` probe 可继续降级尝试
      `http://...`
  - 同时，candidate continuation 现在也开始把 transport-level
    network/TLS failure 视为可继续尝试下一个 candidate 的信号；不再只局限于
    `404/405/415/422`、non-JSON、base-URL drift 这些更偏 shape/status 的错误
  - 这轮 focused tests 已锁住：
    - 本地 HTTPS gateway probe 可通过后续 HTTP candidate 成功
    - local-gateway candidate expansion 顺序
    - network error 仍可继续 candidate fallback
  - 这说明 `settings` Phase 5 lane 又从“provider-specific candidate 规则”
    继续推进到了“本地 gateway transport candidate 规则”；后续更高信号缺口
    已进一步收窄到登录态 provider-success smoke、route-group cutover
    证据，以及仅在真实合同漂移存在时才继续扩展 non-openai-compatible
    detailed error/status owner
- 2026-06-03 同日继续推进后，`settings` 的登录态 business smoke 也补到了
  probe 子路由本身，不再只停留在读侧 `GET`：
  - 新增 `settings-test-business-rust` /
    `settings-test-business-python-fallback`
  - 新增 `settings-check-function-calling-business-rust` /
    `settings-check-function-calling-business-python-fallback`
  - 四条 probe 都复用已落地的本地登录 bootstrap 与 cookie 复用能力，
    并继续挂在 `phase5-settings-business-owner` /
    `phase5-settings-business-fallback` 两个专用 profile 下
  - 这组 smoke 刻意只锁稳定的 `200 + failure shell` 契约，而不把
    route-group cutover readiness 误绑到真实上游 provider 成功态或网络稳定性：
    - `settings/test` 断言 `success=false`、`message="API 测试失败"`，
      同时要求 `provider/model/error/error_type/suggestions/details` 外壳存在
    - `check-function-calling` 断言
      `success=false`、`supported=null`，并要求
      `error/error_type/suggestions/details` 外壳存在；不要再把 Rust owner
      smoke 绑死到单一 generic `message`，因为 Python contract 会按
      `5xx/429/401/404/timeout` 输出不同失败文案
  - 这让 `settings` route-group 的真实登录态 cutover 证据，从：
    - `GET /api/settings`
    - `GET /api/settings/presets`
    继续扩展到：
    - `POST /api/settings/test`
    - `POST /api/settings/check-function-calling`
  - 至此，`settings` 的下一条高信号缺口已经进一步收窄到：
    - `settings/test` 更深层的 transport parity
    - preset create/update/delete/activate/test 的登录态 success lanes
    - 真正 provider-success smoke，而不是继续补更多未登录或壳层级 probe
- 2026-06-03 同日继续推进后，`settings` 的 preset business lane 也不再受
  shared smoke runner 无状态限制，可以在 strangler control-plane 下执行真实的
  登录态 preset 链路：
  - `backend/tools/run_strangler_gateway_smoke.py` 现在新增最小 stateful
    manifest contract：
    - `extract_json`
    - `{{placeholder}}` 模板替换
  - 该能力只服务于当前高信号 route-group business lane，不引入泛化 DSL：
    - 从成功 JSON body 提取字段
    - 在后续 probe 的 `path` / `headers` / `body` / `json_body` /
      `multipart_form` / `expected_*` 中复用已提取值
  - `deploy/strangler-gateway-probes.json` 中，
    `phase5-settings-business-owner` /
    `phase5-settings-business-fallback` 已从：
    - `GET /api/settings`
    - `GET /api/settings/presets`
    - `POST /api/settings/test`
    - `POST /api/settings/check-function-calling`
    扩展为真实 preset business chain：
    - `POST /api/settings/presets` -> 提取 `preset_id`
    - `PUT /api/settings/presets/{preset_id}`
    - `POST /api/settings/presets/{preset_id}/test`
    - `POST /api/settings/presets/{preset_id}/activate`
    - `GET /api/settings` 确认 active preset 已应用
    - `POST /api/settings` 手动保存触发 preset 自动取消激活
    - `GET /api/settings/presets` 确认 `active_preset_id = null`
    - `DELETE /api/settings/presets/{preset_id}`
    - `POST /api/settings/presets/from-current` -> 提取新 preset id
    - `DELETE /api/settings/presets/{current_preset_id}`
  - 这说明 `settings` Phase 5 lane 又从“已有 preset owner 语义”推进到了
    “可执行的 preset business cutover evidence”
  - 当前真实环境阻塞已经收敛到部署环境本身，而不是 runner/manifest：
    - 使用本地默认登录约定 `admin / admin123` 尝试真实 smoke 时，
      `http://127.0.0.1:8005/api/auth/local/login` 返回
      `WinError 10061`
    - 这表示 `8005` 当前未监听，阻塞是 live strangler 环境未在线，而不是
      preset business smoke 资产缺失
  - 因此 `settings` 的后续高信号动作也进一步明确：
    - 优先在在线 `8005` 环境拿到 owner/fallback 双侧 preset business
      evidence
    - 再评估 preset lane 的 fallback shrink readiness
    - 不再回头补低信号的只读或未登录 probe
- 2026-06-03 同日继续推进后，`settings/check-function-calling` 的 failure
  shell 也进一步收口到 Python owner：
  - Rust owner 路径不再把所有失败统一退回
    `generic_suggestions("function_calling")`
  - `check-function-calling` 现在复用 `settings/test` 已迁入 Rust 的
    provider/gateway/base-url-aware failure guidance 生成器，因此
    `5xx` gateway、`429` 限流、`401` 认证失败、`404` 地址/模型漂移、
    `timeout` 等失败开始共享同一组 Python-aligned 提示语义
  - failure `details` 现在也开始带上 `http_status_code`，与
    `settings/test` 的 failure shell 收敛到同一 owner 语义
  - 同时顶层 `message` 也已按状态码收口到 Python 语义：
    - `5xx` -> `上游服务暂时不可用（HTTP NNN）`
    - `429` -> `请求过于频繁，暂时无法确认模型能力`
    - `401` -> `认证失败，暂时无法确认模型能力`
    - `404` -> `接口地址或模型不可用，暂时无法确认模型能力`
    - timeout -> `检测超时`
  - 这轮 focused tests 继续锁住了 gateway failure 的 owner path，避免
    `check-function-calling` 再退回 generic failure shell
- 2026-06-03 同日继续推进后，`settings` probe lane 又完成了下一条更靠近
  transport owner 的收口：`http_status_code` 在 Rust 主 owner 路径上不再只靠
  message parsing。
  - `backend-rs/src/ai/types.rs` 的 `AIRequestError` 现在新增了结构化
    `status_code`
  - `backend-rs/src/ai/clients/openai.rs` 的 non-stream detailed failure
    现在会在 finalize 时把 HTTP 状态码直接带回 route owner
  - `backend-rs/src/ai/service.rs` 的 fallback-model detailed error 合并路径
    也会保留 `status_code`
  - `settings/test` 与 `check-function-calling` 现在都优先读取
    `error.status_code`，只在旧/非结构化错误来源上才退回文本解析
  - 新增 focused helper test，明确锁住：
    即使错误 message 里不含 `502` 这类数字，只要 carrier 已给出
    `status_code=502`，Rust 也必须继续生成 Python-aligned gateway guidance
  - 因此这里的 debt 已从“主路径仍靠 message parsing”收缩为：
    “个别旧来源或非 OpenAI-compatible detailed error 仍可能走兼容退路”
- 2026-06-03 同日继续推进后，`settings/test` 的 non-openai-compatible
  detailed error/status owner 也继续缩口到了 Anthropic 主路径：
  - `backend-rs/src/ai/clients/anthropic.rs` 现在新增真正的
    `chat_completion_detailed(...)` owner path，而不再只返回 `String`
  - Anthropic non-stream detailed probe 失败现在会像 OpenAI-compatible
    owner 一样，把 HTTP 状态码直接落进 `AIRequestError.status_code`
  - `backend-rs/src/ai/service.rs` 的 Anthropic detailed branch 也不再
    用 `AIRequestError::new(...)` 重新包一层纯 message，从而避免在
    service owner 边界把结构化状态码再次丢掉
  - `backend-rs/src/api/settings.rs` 已新增 focused route test，证明
    Anthropic gateway `502` 会经由真实 Rust detailed error path 落到
    `details.http_status_code=502`，而不是靠 message parsing 偶然成功
  - 同时 client owner 侧也新增 focused test，直接锁住
    `chat_completion_detailed_keeps_http_status_code`
  - 这说明 `settings` Phase 5 lane 的 debt 又从
    “非 OpenAI-compatible detailed error 仍可能是 message-only owner”
    收缩为：
    - 少量旧来源仍可能走兼容解析
    - 但 Anthropic 这条真实 probe owner path 已开始遵守与
      OpenAI-compatible 相同的 structured status contract
- 2026-06-03 同日继续推进后，`settings` probe lane 也把 Gemini 从
  “误走 OpenAI-compatible 通道”的 owner drift 收口到了独立 Rust provider path：
  - `backend-rs/src/ai/clients/gemini.rs` 新增真正的 `GeminiClient`，
    开始接手 Python `gemini_client.py` 已稳定拥有的
    `/models/{model}:generateContent?key=...` 合同
  - `backend-rs/src/ai/service.rs` 不再让 `provider="gemini"` 落到
    `OpenAIClient`，而是把普通 probe、detailed probe、stream probe
    全部切到 Gemini owner path
  - 这条 seam 不是简单“让请求能通”，而是把：
    - Gemini native request shaping
    - OpenAI-style tool schema -> Gemini `functionDeclarations` 转换
    - Gemini text/tool-call parts 解析
    - Gemini detailed HTTP failure `status_code`
    真正迁进 Rust provider owner
  - `backend-rs/src/api/settings.rs` 已新增两条 focused route tests：
    - `check_function_calling_uses_gemini_owner_path_for_tool_calls`
    - `test_api_connection_uses_gemini_owner_path_for_success_shell`
    它们直接证明 `check-function-calling` 与 `settings/test` 已经不再依赖
    OpenAI-compatible `/chat/completions` 假通道
  - 这说明 `settings` Phase 5 lane 的 debt 又从
    “Gemini 仍是假 owner 路径”收缩为：
    - Gemini 基础 endpoint ownership 已迁入 Rust
    - 后续更该做的是登录态 business success 证据和 preset success lanes，
      而不是继续容忍 provider owner path 仍挂在 generic OpenAI shim 上
- `wizard-stream` 已补入 `POST /api/wizard-stream/world-building` 基础入口的
  Rust owner 与 Python fallback 双侧 probe。该入口在 Rust
  `backend-rs/src/api/wizard.rs` 与 Python `backend/app/api/wizard_stream.py`
  都存在，Nginx 也有 Rust 显式 location，因此本轮把该组从
  `outline + regenerate + career-system + characters` 扩展到初始世界观生成
  SSE 入口。当前证据仍是未登录边界，不代表登录态 SSE event shape、模型调用、
  world-building 落库或事务回滚语义完整等价。
- `projects` 已补入 `GET /api/projects/{project_id}` 的 Rust owner 与
  Python fallback 双侧 probe，并同步 ownership checklist / rollback runbook。
- `projects` 已补入基础 CRUD 与 TXT 导出低前提边界：
  `POST /api/projects`、`PUT /api/projects/{project_id}`、
  `DELETE /api/projects/{project_id}` 与 `GET /api/projects/{project_id}/export`
  均进入 Rust owner / Python fallback 双侧 probe。至此，`projects` 的
  第一批 fallback shrink readiness 证据已覆盖基础项目生命周期、列表/详情、
  public import validation、multipart import、TXT/JSON 两类导出，以及维护修复
  入口。当前新增项仍只证明未登录 owner/fallback 边界，不代表登录态创建、
  更新、删除级联清理、TXT 文件内容或响应头完整等价。
- `projects` 已进一步固化为两个一键执行 profile：
  `phase5-projects-owner` 与 `phase5-projects-fallback`。它们各包含 12 条
  `projects` 同路径 probe，使第一批 fallback shrink readiness 从“需要组合
  `phase5-p0 + --route-group projects` 参数”推进到 manifest 内显式 profile。
  下一步应优先补登录态 business smoke / shrink 决策清单，而不是继续增加
  未登录边界 probe。
- `projects` 进一步补入 `POST /api/projects/{project_id}/check-consistency`
  的 Rust owner 与 Python fallback 双侧 probe，把第一批 readiness 模板扩到
  数据维护类显式 Rust location。
- `projects` 继续补入 `POST /api/projects/{project_id}/fix-organizations`
  与 `POST /api/projects/{project_id}/fix-member-counts` 的 Rust owner /
  Python fallback 双侧 probe。至此，当前 Nginx 中 `projects` 维护修复类显式
  Rust location 已具备完整的低前提 owner/fallback 证据。
- `memories` 已从原先 `stats + search` 两条低前提 probe 扩展到六条：
  `stats`、`memories` 列表、`analysis/{chapter_id}`、`foreshadows`、
  `search`、`chapters/{chapter_id}/memories` 删除。该组现在覆盖
  `/api/memories/projects/{project_id}` 下读、查、删三类 API 边界。
- `memories` 的边界结论同步收窄：`/api/memories/*` 是当前 Rust API owner
  与 Python API fallback 的 cutover 评估面；`/memories/` 仍按页面或非 API
  fallback 单独处理，不能和 `/api/memories/*` 混为同一组 fallback 清退对象。
- `relationships` 已补入 `GET /api/relationships/project/{project_id}` 与
  `GET /api/relationships/graph/{project_id}` 的 Rust owner / Python fallback
  双侧 probe。该组从 checklist 里的“可补 smoke”推进到 P1 starter evidence；
  当前仍只证明同路径未登录 owner/fallback 边界，不证明登录态关系图谱聚合
  与组织成员边的完整业务等价。
- `foreshadows` 已补入 `GET /api/foreshadows/projects/{project_id}` 与
  `GET /api/foreshadows/projects/{project_id}/stats` 的 Rust owner /
  Python fallback 双侧 probe。该组从 checklist 里的“可补 smoke”推进到
  P1 starter evidence；当前仍只证明同路径未登录 owner/fallback 边界，
  不证明登录态伏笔列表、统计、章节上下文或 plant/resolve/abandon 写侧语义等价。
- `writing_styles` 已补入 `GET /api/writing-styles/user` 与
  `GET /api/writing-styles/project/{project_id}` 的 Rust owner /
  Python fallback 双侧 probe。该组从 checklist 里的“可补 smoke”推进到
  P1 starter evidence；当前仍只证明同路径未登录 owner/fallback 边界，
  不证明登录态 preset 同步、自定义风格 CRUD、默认风格写入或
  `project_default_styles` 侧效应等价。
- `organizations` 已补入 `GET /api/organizations/project/{project_id}` 与
  `POST /api/organizations/generate-stream` 的 Rust owner / Python fallback
  双侧 probe。该组从 checklist 里的“有 schema 风险注记”推进到
  P1 starter evidence；当前仍只证明同路径未登录 owner/fallback 边界，
  不证明登录态组织 CRUD、成员增删改、AI 生成落库、成员计数或
  `organization_members` 字段一致性等价。
- `careers` 已补入 `GET /api/careers?project_id={project_id}` 与
  `GET /api/careers/generate-system?project_id={project_id}` 的 Rust owner /
  Python fallback 双侧 probe。该组从 checklist 里的“可补 smoke”推进到
  P1 starter evidence；当前仍只证明同路径未登录 owner/fallback 边界，
  不证明登录态职业 CRUD、角色职业绑定、AI 生成落库、职业阶段进度或
  `character_careers` 关联语义完整等价。
- `inspiration` 已补入 `POST /api/inspiration/generate-options` 与
  `POST /api/inspiration/quick-generate` 的 Rust owner / Python fallback
  双侧 probe。该组从 checklist 里的“可补 smoke”推进到 P1 starter evidence；
  当前仍只证明同路径未登录 owner/fallback 边界，不证明登录态 prompt
  template、AI provider、web research、retry/validation 或完整灵感工作流语义
  等价。
- `mcp_plugins` 已补入 `GET /api/mcp/plugins` 与
  `POST /api/mcp/plugins/simple` 的 Rust owner / Python fallback 双侧 probe。
  该组从 checklist 里的“可补 smoke”推进到 P1 starter evidence；当前仍只
  证明同路径未登录 owner/fallback 边界，不证明登录态插件创建/更新、toggle、
  status/tools/test/call、MCP session 注册/断开或后台任务语义完整等价。
- `prompt_templates` 已补入 `GET /api/prompt-templates` 与
  `GET /api/prompt-templates/system-defaults` 的 Rust owner / Python fallback
  双侧 probe。该组从 checklist 里的“可补 smoke”推进到 P1 starter evidence；
  当前仍只证明同路径未登录 owner/fallback 边界，不证明登录态 categories、
  sync-status、保存/删除/导入/预览、managed template sync 或 prompt formatting
  语义完整等价。
- `background_tasks` 已补入 `GET /api/background-tasks` 与
  `POST /api/background-tasks` 的 Rust owner / Python fallback 双侧 probe。
  该组从 checklist 里的“可补 smoke”推进到 P1 starter evidence；当前仍只
  证明同路径未登录 owner/fallback 边界，不证明登录态 task registry 生命周期、
  SSE stream、cancel、workflow-state 或任务缺失 payload 语义完整等价。
- `prompt_workshop` 已补入 `POST /api/prompt-workshop/submit` 与
  `POST /api/prompt-workshop/items/{item_id}/like` 的 Rust owner / Python
  fallback 双侧 probe。该组从 checklist 里的“可补 smoke”推进到
  P1 starter evidence；当前仍只证明两个登录态入口的 owner/fallback 边界，
  不证明公开 items/status、import/download、my-submissions、admin 审核或
  云端代理模式语义完整等价。
- `polish` 已补入 `POST /api/polish` 与 `POST /api/polish/batch` 的 Rust
  owner / Python fallback 双侧 probe。该组从 checklist 里的“低风险但需补
  smoke”推进到 P1 starter evidence；当前仍只证明同路径未登录 owner/fallback
  边界，不证明登录态 provider 调用、PromptService 模板、generation_history
  写入或批量结果 payload 语义完整等价。
- `changelog` 已补入 `GET /api/changelog` 与
  `POST /api/changelog/refresh` 的 Rust owner / Python fallback 双侧 public
  probe。该组从“低风险但需补 smoke”推进到 P1 starter evidence；当前证明
  双侧同路径 public contract 形态存在，但真实执行依赖 GitHub API 网络可用性、
  限流与缓存行为，不能按本地纯 auth-boundary probe 的稳定度评估。
- `ai_test / ai` 已补入 `POST /api/ai-test` 与别名
  `POST /api/ai/test` 的 Rust auth-boundary probe，并归入
  `phase5-p1-asymmetric`。当前仓库未发现对应 Python fallback router，因此
  该组不纳入 `phase5-p1-fallback` 统计；后续需要先确认产品策略是 Rust-only、
  临时禁用，还是重新补 Python fallback。
- 因此第一批 fallback shrink readiness 模板已经从“规划建议”推进为
  `settings + projects` 两组可复用资产；`memories` 则从 path 边界澄清推进到
  API readiness 增强，但登录态 vector memory / analysis / delete side effects
  仍需后续 business smoke 才能支持更激进的 fallback 收缩。
- 2026-06-02 进一步校准后，`settings + projects` 的下一阶段目标不再只是
  “继续累积 owner/fallback probes”，而是要把现有 probe、business 样本、
  rollback 线索和 dedicated profile 组织成**可直接读的 shrink-ready 摘要**。
  这类摘要的作用是减少人工逐条比对 manifest / rollback / checklist 的成本，
  让 route-group cutover 评估更接近批次化执行，而不是继续停留在“证据很多，
  但每次都要重新手工拼装”的状态。
- 2026-06-03 继续推进 `chapter_generation` 的真实 Rust seam 收口后，
  `chapter_batch_generation_resume_task_command_service.rs` 的 single-chapter
  resume lane 已不再本地拥有一套独立的：
  - chapter access load
  - prerequisite gate
  - target projection
  - provider payload / execution-config prepare
  当前它已改为复用
  `chapter_single_generation_prepare_service.rs` 的 shared owner：
  - `load_single_chapter_generation_target(...)`
  - `prepare_single_chapter_generation_execution_config_from_runtime_state(...)`
  同时把 single resume 的 prerequisite / target 校验前移到了
  `ResumeExecutionEligibilityPlan::validate_access_and_prerequisites(...)`
  阶段，并通过 `validated_single_chapter_target` 显式传给 dispatch-plan
  input。这个变化的意义不是“少写几行代码”，而是让：
  - 单章 start
  - 单章 background write workflow
  - 单章 resume
  三条相邻生命周期开始共享同一个 Rust owner boundary，进一步缩小
  Python-era owner split。

  这条 seam 已经通过：
  - `chapter_batch_generation_resume_task_command_service`: 50 passed
  - `chapter_single_generation_prepare_service`: 22 passed
  - `cargo check`: passed

  因而 Phase 5 当前在 `chapter_generation` 组的重心可以继续从：
  - “先证明 route-group owner/fallback/strangler 治理能力”
  逐步推进到：
  - “持续收口 start / resume / runtime / write workflow 内部的真实 Rust owner seam”
  但仍应保持同样的约束：
  - 小步可验证
  - 不扩大业务面
  - 不提前移除 Python fallback
- 2026-06-03 再向前推进一条相邻 `chapter_generation` seam 后，
  batch create 与 batch resume 两条 lane 也不再各自本地手工组装
  `BatchGenerationExecutionInput`。当前
  `chapter_batch_generation_runtime_state_service.rs`
  已新增 shared owner
  `build_batch_generation_execution_input(...)`，
  并由：
  - `chapter_batch_generation_write_workflow_service.rs`
  - `chapter_batch_generation_resume_task_command_service.rs`
  共同复用。

  这条 seam 的意义不在于“抽出一个公共函数”，而在于把 batch
  startup-to-runtime handoff 的真实契约再次收回到单一 Rust owner：
  - `chapter_ids`
  - `target_word_count`
  - `compat_options`
  - `PreparedGenerationExecutionConfig -> AIConfig`
  不再分别由 create / resume 各自维护一套局部装配逻辑。

  同时，batch create lane 的
  `PreparedBatchGenerationCreateRuntimeLaunch`
  也不再额外持有第二份本地 `chapter_ids` owner，而是直接从 shared
  runtime input 派生 task-launch 所需 chapter id payload。这样
  `chapter_generation` 组内部又少了一层 Python-era lifecycle seam，
  让后续 batch write-lane cutover 与 fallback shrink 的审计边界更清晰。

  这条 seam 已通过：
  - `chapter_batch_generation_runtime_state_service`: 72 passed
  - `chapter_batch_generation_resume_task_command_service`: 50 passed
  - focused batch create launch / persistence tests: passed
  - `cargo check`: passed

  目前 `chapter_generation` Phase 5 lane 的下一步仍应保持相同节奏：
  - 继续优先收口 batch write / runtime / prepare 的相邻 owner seam
  - 不把无关顺序断言或广域清理混入当前迁移 patch
  - 保持小步可验证，再逐步逼近真正的 Python fallback shrink
- 2026-06-03 沿着同一条 `chapter_generation` Phase 5 lane 再收一层后，
  batch create write workflow 已继续删除一组重复 `chapter_ids` owner。
  当前：
  - `PreparedBatchGenerationCreateTaskLaunch`
  - `BatchGenerationCreateLaunchPersistencePlan`
  都不再各自持有本地 `chapter_ids`，
  而是统一围绕 shared `runtime_input.chapter_ids` 派生 create-task
  persistence 所需的 chapter-id payload。

  这条 seam 的意义不是“字段少了一个”，而是把 batch create lane 内部的
  startup-to-runtime handoff 再收窄一层：shared runtime handoff 已建立后，
  task-launch 和 persistence-plan 不再继续并排维护第二份、第三份同值
  contract。这样后续如果继续推进 batch create / batch resume 的 write-lane
  cutover，审计边界会更清楚，因为 persisted task payload 已显式依赖当前
  runtime owner，而不是依赖额外的局部向量副本。

  这条 seam 已通过：
  - `should_build_batch_generation_create_launch_persistence_plan_from_create_parts`
  - `should_build_batch_generation_create_launch_task_from_create_parts`
  - `should_build_batch_generation_create_task_launch_contract`
  - `should_build_batch_generation_create_runtime_launch_from_runtime_seed`
  - `should_build_batch_generation_create_persistence_dispatch_contract`
  - `cargo check`

  因而当前 `chapter_generation` lane 的下一步仍应保持同样策略：
  - 继续只收相邻、可验证、会减少 lifecycle contract duplication 的 seam
  - 不把无关排序断言或广域 suite 噪音混进当前 Rust 迁移 patch
  - 让每一条 seam 都更直接服务于后续 fallback shrink / cutover 审计
- 2026-06-03 同一条 batch create lane 继续向前推进后，
  `chapter_batch_generation_write_workflow_service.rs` 又删除了一层 shared
  runtime input 已经拥有的重复字段 owner。当前：
  - `PreparedBatchGenerationCreateTaskLaunch` 不再本地持有
    `target_word_count`
  - `BatchGenerationCreateLaunchPersistencePlan` 不再本地持有
    `user_id` 与 `target_word_count`
  - create response / persisted task payload 需要的这两类字段，统一改为从
    `runtime_input.user_id` 和 `runtime_input.target_word_count` 派生

  这条 seam 的意义同样不在于“结构体又少了两个字段”，而在于 batch create
  startup-to-runtime handoff 的真实 owner 再次收窄：当 shared
  `BatchGenerationExecutionInput` 已成为显式 runtime contract 后，下游
  task-launch / persistence-plan 不再继续并排维护第二套相同语义的字段副本。
  这样后续继续推进 batch create / batch resume write-lane cutover 时，
  lifecycle contract 的审计链会更短，也更接近真正的 Rust single-owner。

  这条 seam 已通过：
  - `should_build_batch_generation_create_launch_persistence_plan_from_create_parts`
  - `should_build_batch_generation_create_task_launch_contract`
  - `should_build_batch_generation_create_persistence_dispatch_contract`
  - `should_build_batch_generation_create_workflow_launch_into_persistence_plan`
  - `cargo check`

- 2026-06-03 沿着同一条 batch create Phase 5 lane 再向前收一层后，
  queued startup snapshot 的 owner 也已经从“下游局部现算”改成显式 Rust
  contract。当前：
  - `PreparedBatchGenerationCreateRuntimeLaunch` 不再持有裸
    `runtime_state_payload: Value`
  - create lane 改为显式持有并转发
    `BatchGenerationQueuedSnapshotPlan`
  - `PreparedBatchGenerationCreateTaskLaunch`、
    `BatchGenerationCreateLaunchPersistencePlan`、
    create response / persistence dispatch
    现在都围绕同一份 `startup_snapshot_plan` 工作

  这条 seam 的意义不在于“字段换了个名字”，而在于 batch create
  startup-to-runtime handoff 的 queued snapshot contract 终于不再由多个下游
  分支各自二次拼装。shared runtime handoff 已经建立后，response、persist、
  dispatch 现在消费的是上游已经准备好的同一份 startup snapshot owner，
  而不是在每个边界再次局部计算 queued snapshot shape。

  对 Phase 5 的价值是直接的：这让 batch create startup 边界更接近 single
  lane 的 startup snapshot owner 形态，也让后续 batch create / batch resume
  write-lane cutover 与 fallback shrink 的审计链进一步缩短。

  这条 seam 已通过：
  - `should_build_batch_generation_create_launch_persistence_plan_from_create_parts`
  - `should_build_batch_generation_create_launch_task_from_create_parts`
  - `should_build_batch_generation_create_task_launch_contract`
  - `should_build_batch_generation_create_runtime_launch_from_runtime_seed`
  - `should_build_batch_generation_create_persistence_dispatch_contract`
  - `should_build_batch_generation_create_workflow_launch_into_persistence_plan`
  - `should_keep_prepared_batch_generation_create_launch_owner_contract_explicit`
  - `cargo check`

- 2026-06-03 同一条 batch create Phase 5 lane 继续推进后，create-lane
  config contract 也已经进一步收回到单一 Rust owner。当前：
  - `PreparedBatchGenerationCreateTaskLaunch` 不再把
    `start_chapter_number / style_id / enable_analysis / max_retries`
    拆成独立字段继续向下游转发
  - `BatchGenerationCreateLaunchPersistencePlan` 也不再并排持有这些字段
  - 两层 create-lane owner 现在统一围绕
    `BatchGenerationCreateTaskSpec` 工作
  - 同时 `total_chapters` 也不再作为 persistence-plan 的额外存储字段，
    而是直接从 `chapters_to_generate.len()` 派生

  这条 seam 的意义不是“把几个字段包回结构体里”，而是在 batch create
  write-lane 上继续减少平行 contract。既然上游已经有显式
  `BatchGenerationCreateTaskSpec`，下游 task-launch 与 persistence-plan
  就不再需要继续把同一组配置字段拆开重持；而章节总数也不再需要额外存一份
  plan-local owner。这样 create task config、chapter total、persisted task
  assembly 的审计链更短，也更接近真正的 Rust single-owner。

  对 Phase 5 的价值同样直接：这让 batch create / batch resume write-lane
  的 owner 边界继续收紧，为后续 fallback shrink、cutover smoke 和 rollback
  审计提供更干净的证据链。

  这条 seam 已通过：
  - `should_build_batch_generation_create_launch_persistence_plan_from_create_parts`
  - `should_build_batch_generation_create_task_launch_contract`
  - `should_build_batch_generation_create_workflow_launch_into_persistence_plan`
  - `should_keep_prepared_batch_generation_create_launch_owner_contract_explicit`
  - `should_build_batch_generation_create_launch_task_from_create_parts`
  - `should_build_batch_generation_create_persistence_dispatch_contract`
  - `cargo check`

- 2026-06-03 同一条 batch create Phase 5 lane 再继续向前推进后，
  `chapter_batch_generation_write_workflow_service.rs` 又删掉了一层
  create workflow 和 persistence owner 之间不再需要的中间 lifecycle
  contract。当前：
  - `PreparedBatchGenerationCreateTaskLaunch` 已完全删除
  - `PreparedBatchGenerationCreateWorkflowLaunch` 现在直接通过
    `BatchGenerationCreateLaunchPersistencePlan::from_workflow_launch(...)`
    进入 persistence owner
  - create lane 从：
    `WorkflowLaunch -> TaskLaunch -> PersistencePlan`
    收成：
    `WorkflowLaunch -> PersistencePlan`

  这条 seam 的意义不是“结构体又少了一个”，而是在 batch create
  write-lane 上继续删掉不对应真实 transport / persistence / runtime
  边界的假 owner 停靠点。既然 workflow launch 已经拥有 create task spec、
  prepared execution 和 runtime launch，下游就不再需要本地 task-launch
  wrapper 再转一次。这样 create startup-to-persistence handoff 的审计链更短，
  也更接近真正的 Rust single-owner。

  这条 seam 已通过：
  - `should_build_batch_generation_create_launch_persistence_plan_from_create_parts`
  - `should_build_batch_generation_create_launch_task_from_create_parts`
  - `should_keep_prepared_batch_generation_create_launch_owner_contract_explicit`
  - `should_build_batch_generation_create_workflow_launch_into_persistence_plan`
  - `should_build_batch_generation_create_persistence_dispatch_contract`
  - `cargo check`

- 2026-06-03 同一条 batch create Phase 5 lane 又继续向 persistence / dispatch
  邻域收一层后，create write-lane 的两层本地 wrapper owner 也已经被删除。
  当前：
  - `PreparedBatchGenerationCreateLaunch` 已完全删除
  - `BatchGenerationCreatePersistenceDispatchContract` 也已完全删除
  - `start_owned_batch_generation_write_workflow(...)` 现在直接：
    `PreparedBatchGenerationCreateWorkflowLaunch::prepare(...)`
    -> `into_persistence_plan(...)`
    -> `persist_and_dispatch(...)`
  - persistence owner 会直接完成：
    - active task model 组装
    - create response payload 组装
    - queued snapshot persist
    - runtime dispatch

  这条 seam 的意义也不是“函数少了一层”，而是在 batch create write-lane 上
  继续删除不承载真实生命周期语义的本地 owner 壳层。此前 create lane 在
  prepared workflow owner 和真正 runtime boundary 之间，还保留着：
  - 一个只负责包 `now + persistence_plan` 的 launch wrapper
  - 一个只负责临时打包 task / snapshot / response / runtime_input 的
    dispatch wrapper
  现在这两层也被收掉之后，create write-lane 的 owner 图会更短、更干净，
  对后续 batch create / batch resume 的 cutover smoke、fallback shrink、
  rollback 审计都更友好。

  这条 seam 已通过：
  - `should_build_batch_generation_create_workflow_launch_into_persistence_plan`
  - `should_keep_batch_generation_create_persistence_plan_owner_contract_explicit`
  - `should_build_batch_generation_create_persistence_plan_task_and_response_payload`

- 2026-06-03 同一条 batch resume Phase 5 lane 再继续向 load / prepare 邻域
  收一层后，resume write-lane 的本地 workflow-context owner 也已经被删除。
  当前：
  - `ResumeBatchGenerationWorkflowContext` 已完全删除
  - resume lane 现在直接：
    `load_owned_batch_generation_resume_dependencies(...)`
    -> `prepare_batch_generation_resume(...)`
    -> `dispatch(...)`
  - load-time owner 只保留：
    - `ResumeBatchGenerationCommandState`
    - optional persisted snapshot
    - parsed `BatchGenerationRequestRuntimeState`
    然后直接喂给 prepared resume launch owner

  这条 seam 的意义不是“少了一个 struct”，而是在 batch resume write-lane 上
  继续删除不承载真实 transport / persistence / runtime 边界的本地生命周期壳层。
  此前 resume 在 owned-task / snapshot 已经加载完之后，还要先停到一个本地
  workflow-context wrapper，再转给真正的 prepared launch boundary；现在这层也被
  收掉之后，resume 的 load -> prepare owner 图会更短，也更接近当前 create lane
  已经形成的直接 prepared-owner handoff 形态。

  对 Phase 5 的价值同样直接：这会让 batch create / batch resume 在
  write-lane 上的 prepared-owner 审计更接近同一套路径，对后续 fallback shrink、
  cutover smoke、rollback 审计都更友好。

  这条 seam 已通过：
  - `should_keep_resume_batch_generation_dependency_loader_contract_explicit`
  - `should_build_dispatch_plan_from_prepared_resume_launch_owner`
  - `cargo check`

- 2026-06-03 同一条 `chapter_generation` Phase 5 lane 继续向 batch resume
  邻域推进后，resume write-lane 的 execution-config owner 也已经前移到
  prepared launch。当前：
  - `PreparedBatchGenerationResumeLaunch` 显式持有
    `PreparedGenerationExecutionConfig`
  - resume 不再在 `dispatch(...)` 阶段临时查库、迟到装配 execution config
  - execution config 现在和 batch create lane 一样，在 prepare 阶段就被
    显式准备好，再向下游 runtime dispatch 转发

  这条 seam 的意义不在于“少了一次函数调用”，而在于 batch create 和 batch
  resume 在 write-lane 上的 owner 形态进一步拉平。此前 create 已经把
  execution config 前移到了 prepared owner，但 resume 仍保留一层迟到装配；
  现在这层差异被收掉之后，两条 lane 更接近同一种 Rust contract：
  prepare 拥有 config，dispatch 只负责 runtime handoff。

  对 Phase 5 的价值也很直接：这进一步缩短了 create/resume 的生命周期分叉，
  让后续 fallback shrink、cutover smoke、rollback 审计时更容易说明
  “Rust 在哪一层真正拥有 execution-config 边界”。

  这条 seam 已通过：
  - `should_build_prepared_resume_launch_contract_with_shared_reset_and_dispatch_owner`
  - `should_keep_resume_execution_and_payload_contract_explicit`
  - `should_build_dispatch_plan_from_prepared_resume_launch_owner`
  - `cargo check`

- 2026-06-03 同一条 batch resume Phase 5 lane 再继续向里收一层后，
  eligibility 阶段的 `chapter_ids` contract 也已经从双份 owner 收成单份。
  当前：
  - `ResumeExecutionEligibilityPlan::Batch` 不再同时保留
    `execution_selection` 和 `validated_chapter_ids`
  - batch resume eligibility 现在只保留一个
    `chapter_ids: Vec<String>` owner
  - 需要 `ResumeExecutionSelection::Batch` 的地方再从这一个 owner 构造，
    而不是在 eligibility 阶段持续并排维护两份相同语义的 batch chapter
    selection contract

  这条 seam 的意义也不是“枚举字段少了一个”，而是 batch resume write-lane
  上又去掉了一层平行 contract。此前 eligibility 阶段在真正进入 dispatch
  plan 之前，就已经把同一组 resumed chapter ids 复制成两份本地 owner；
  现在这层复制被收掉之后，resume batch chapter selection 的审计链更短，
  也更符合 Phase 5 持续推进的 single-owner 方向。

  对后续迁移的价值是连续的：这让 create / resume 两条 lane 在
  chapter-id handoff 上都更接近“一个显式 owner + 下游按需派生”的形态，
  对 fallback shrink、cutover smoke、rollback 审计都会更友好。

  这条 seam 已通过：
  - `should_build_resume_execution_eligibility_plan_for_single_and_batch_selection`
  - `should_build_prepared_resume_launch_contract_with_shared_reset_and_dispatch_owner`
  - `should_build_dispatch_plan_from_prepared_resume_launch_owner`
  - `cargo check`

- 2026-06-03 同一条 batch resume Phase 5 lane 再往 prepared launch 的最后一跳
  收口后，resume write-lane 的 dispatch-plan owner 也已经前移到
  `PreparedBatchGenerationResumeLaunch`。当前：
  - `PreparedBatchGenerationResumeLaunch` 不再并排保留
    `dispatch_plan_input + execution_config`
  - 它现在直接显式持有
    `dispatch_plan: ResumeExecutionDispatchPlan`
  - `persist_reset_and_build_prepared_batch_generation_resume_launch(...)`
    会在 prepare 阶段就把最终 runtime dispatch plan 组好
  - `into_dispatch_contract()` 不再需要额外的 `user_id`
  - `resume_owned_batch_generation_write_workflow(...)` 现在直接把 prepared
    owner 交给 `dispatch(...)`，不再在最后一跳补一次用户维度 handoff

  这条 seam 的价值也不是“少传了一个参数”，而是 batch resume write-lane
  上最后一个“prepared owner 还没真正持有 runtime dispatch contract”的假边界
  被收掉了。此前 resume prepare 虽然已经拥有 reset 结果、response payload、
  execution config 和 selection 语义，但真正的 single/batch runtime dispatch
  plan 仍然在最后 handoff 时才临时组出来；现在这一跳也被前移之后，
  prepared launch 本身就成为了更接近 cutover 的单一 owner。

  对 Phase 5 的价值同样直接：这让 create / resume 两条 write-lane 在
  prepare -> dispatch 的 owner 形态继续拉平，也让 fallback shrink、
  stronger smoke、rollback 审计时更容易回答“Rust 到底在哪一层真正拥有
  runtime dispatch contract”。

  这条 seam 已通过：
  - `should_build_prepared_resume_launch_contract_with_shared_reset_and_dispatch_owner`
  - `should_keep_resume_execution_and_payload_contract_explicit`
  - `should_build_dispatch_plan_from_prepared_resume_launch_owner`
  - `should_build_resume_execution_eligibility_plan_for_single_and_batch_selection`
  - `cargo check`

- 2026-06-03 同一条 batch resume Phase 5 lane 继续沿 eligibility -> prepared
  launch owner 链向里收后，`ResumeExecutionDispatchPlanInput` 这层中间 contract
  也已经被删除。当前：
  - batch resume eligibility 不再先转成
    `ResumeExecutionDispatchPlanInput`
  - `ResumeExecutionEligibilityPlan` 现在直接拥有
    `prepare_dispatch_plan(...)`
  - validated single/batch selection 会在这个 owner 内直接：
    - 准备 execution config
    - 组装最终 `ResumeExecutionDispatchPlan`
  - `persist_reset_and_build_prepared_batch_generation_resume_launch(...)`
    现在直接消费最终 dispatch plan，而不是先构造一个临时
    dispatch-plan-input 再转一次

  这条 seam 的意义不在于“少了一个 struct”，而在于 batch resume write-lane
  上又删掉了一层假的生命周期中转。此前 eligibility 已经完成 selection
  和 access/prerequisite validation，但在真正生成 prepared dispatch contract
  之前，还要经过一次本地 `DispatchPlanInput` 搬运；现在这一跳也被收掉之后，
  resume prepare 链更接近“validated owner 直接产出 runtime dispatch owner”。

  对 Phase 5 的价值同样直接：这让 batch resume 在 prepare 阶段的 owner 图
  更短、更单一，也让 fallback shrink、cutover smoke、rollback 审计时更容易
 说明 Rust 在 runtime launch 前到底还剩哪些真实 owner 边界。

  这条 seam 已通过：
  - `should_build_prepared_resume_launch_contract_with_shared_reset_and_dispatch_owner`
  - `should_build_dispatch_plan_from_prepared_resume_launch_owner`
  - `should_build_resume_execution_eligibility_plan_for_single_and_batch_selection`
  - `should_keep_resume_execution_and_payload_contract_explicit`
  - `cargo check`

  - 2026-06-03 同一条 `chapter_generation` Phase 5 lane 继续沿 batch create
    write-lane startup/runtime 边界收口后，本地
    `PreparedBatchGenerationCreateRuntimeLaunch` owner 也已经被删除。当前：
    - `PreparedBatchGenerationCreateWorkflowLaunch` 不再保留
      `runtime_launch`
    - 它现在直接显式持有：
      - `startup_snapshot_plan: BatchGenerationQueuedSnapshotPlan`
      - `runtime_input: BatchGenerationExecutionInput`
    - `PreparedBatchGenerationCreateWorkflowLaunch::prepare(...)`
      现在会直接消费 `BatchGenerationCreateRuntimeSeed::into_parts()`，原地完成：
      - startup snapshot 组装
      - runtime execution input 组装
    - `BatchGenerationCreateLaunchPersistencePlan::from_workflow_launch(...)`
      现在直接消费 workflow-launch owner，不再多经过一次
      runtime-launch unpack

  这条 seam 的意义不在于“又少了一个 struct”，而是在 batch create write-lane
  上又删除了一层不承载真实 transport / persistence / runtime 分叉的本地
  生命周期中转。此前 create lane 的 startup state 和 runtime dispatch contract
  虽然都已经是 Rust owner，但它们之间仍然停留在一个单纯搬运用的
  `PreparedBatchGenerationCreateRuntimeLaunch` wrapper；现在这一跳也被收掉后，
  create lane 的 owner 图会更短，也更接近当前 resume lane 持续收口后的
  single-owner 方向。

  对 Phase 5 的价值同样直接：这让 batch create / batch resume 两条 write-lane
  更容易对齐到“prepare owner 直接持有 startup snapshot + runtime dispatch
  contract”的同一种 Rust 形态，对后续 fallback shrink、cutover smoke、
  rollback 审计都会更友好。

  这条 seam 已通过：
  - `should_build_batch_generation_create_workflow_launch_into_persistence_plan`
  - `should_build_batch_generation_create_workflow_runtime_parts_from_runtime_seed`
  - `should_keep_batch_generation_create_workflow_launch_owner_contract_explicit`
  - `should_build_batch_generation_create_persistence_plan_task_and_response_payload`
  - `cargo test batch_generation_create --manifest-path "backend-rs/Cargo.toml" -- --nocapture`
  - `cargo check`

  - 2026-06-03 同一条 batch create Phase 5 lane 再继续向 request-prepare ->
    workflow-launch 边界收口后，本地
    `PreparedBatchGenerationCreateExecution` owner 也已经被删除。当前：
    - `BatchGenerationCreateWorkflowRequest::prepare(...)` 不再返回
      `PreparedBatchGenerationCreateExecution`
    - 它现在直接返回最终验证后的：
      - `normalized_target_word_count`
      - `Vec<BatchGenerationCreateChapterTarget>`
    - `PreparedBatchGenerationCreateWorkflowLaunch` 不再保留
      `prepared_execution`
    - workflow-launch owner 现在直接显式持有
      `chapters_to_generate`
    - persistence-plan 构造也不再多经过一次
      `prepared_execution.into_parts()` 本地解包

  这条 seam 的意义不是“又少了一个中间 struct”，而是在 batch create write-lane
  上继续删除一层不承载真实 transport / persistence / runtime 分叉的 prepare-time
  owner hop。此前 request prepare 虽然已经完成了范围选择、前置校验和
  target-word-count 归一化，但这些结果还要先停留在一个单纯搬运用的
  `PreparedBatchGenerationCreateExecution` wrapper，再投递给 workflow-launch
  owner；现在这一跳也被收掉后，create lane 的 validated selection 会更直接地
  进入 startup/runtime workflow owner。

  对 Phase 5 的价值同样直接：这让 batch create prepare -> workflow-launch 的
  owner 图再缩短一层，也让 create / resume 两条 write-lane 后续对齐时更容易
  审计“Rust 在 prepare 阶段真正保留了哪些 owner，哪些已经不再需要本地壳层”。

  这条 seam 已通过：
  - `should_keep_batch_generation_create_workflow_launch_owner_contract_explicit`
  - `should_build_batch_generation_create_launch_task_from_create_parts`
  - `should_build_batch_generation_create_persistence_plan_task_and_response_payload`
  - `cargo test batch_generation_create --manifest-path "backend-rs/Cargo.toml" -- --nocapture`
  - `cargo check`

  - 2026-06-03 同一条 batch resume Phase 5 lane 继续向 validated eligibility ->
    final dispatch owner 收口后，本地
    `ResumeExecutionDispatchPlan::from_execution_selection(...)` 也已经被删除。
    当前：
    - `ResumeExecutionEligibilityPlan::SingleChapter` 不再保留完整
      `ResumeExecutionSelection`
    - 它现在只保留真正还需要的：
      - `chapter_id`
      - `validated_single_chapter_target`
    - `ResumeExecutionEligibilityPlan::prepare_dispatch_plan(...)`
      不再把 single/batch selection 重新包回
      `ResumeExecutionSelection`
    - 它现在直接在 eligibility owner 内组装最终
      `ResumeExecutionDispatchPlan::{SingleChapter,Batch}`

  这条 seam 的意义不是“少了一个 helper 调用”，而是在 batch resume write-lane
  上继续删除一层已经完成 validation 之后仍然重复搬运同一语义 contract 的本地
  owner hop。此前 eligibility owner 虽然已经完成 single/batch 分支确认、访问
  校验与前置条件校验，但仍要把这些结果重新投回
  `ResumeExecutionSelection`，再交给
  `ResumeExecutionDispatchPlan::from_execution_selection(...)`
  去构造真正的 runtime dispatch contract；现在这一跳也被收掉后，validated
  eligibility 会更直接地流入最终 runtime dispatch owner。

  对 Phase 5 的价值同样直接：这让 batch resume prepare owner 图再缩短一层，
  也让 create / resume 两条 write-lane 后续对齐时更容易审计“Rust 在
  prepare 阶段真实保留了哪些 owner，哪些只是本地过渡壳层”。同时，这也让
  reset persistence 前后的 prepared launch contract 更接近单一路径 owner 形态。

  这条 seam 已通过：
  - `cargo test chapter_batch_generation_resume_task_command_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`
  - `cargo check`

  - 2026-06-03 同一条 batch create Phase 5 lane 继续沿 startup runtime-seed
    owner 收口后，create write-lane 的 `BatchGenerationCreateRuntimeSeed`
    也不再跨阶段保留 raw `BatchGenerationRequestRuntimeState`。当前：
    - `BatchGenerationCreateRuntimeSeed` 只保留真正跨边界需要的两项：
      - `runtime_state_payload`
      - `resolved_compat_options`
    - `into_parts()` 也只返回这两项，不再把 raw request owner 一起传给
      create workflow launch / persistence assembly
    - `PreparedBatchGenerationCreateWorkflowLaunch::prepare(...)` 与对应测试
      helper 现在只消费 startup-seed 的 dispatch-ready 产物

  这条 seam 的意义不是“少一个字段”，而是在 batch create write-lane 上删掉
  一层已经失去真实消费者的 raw request owner。此前 create lane 虽然已经把
  runtime payload 和 compat restore 都放进 runtime-seed，但这个 seed 仍然把
  原始 `BatchGenerationRequestRuntimeState` 一起跨阶段带到 workflow launch 邻域。
  现在这一层被收掉后，batch create 更接近 Phase 5 真正需要的
  startup-ready / dispatch-ready owner 形态，而不是“原始输入 + 派生状态”混装
  的 Python-era shell。

  对 Phase 5 的价值同样直接：这让 batch create / batch resume 两条 write-lane
  在 startup-seed / persistence / dispatch 结构上进一步对齐，也让后续
  fallback shrink、stronger smoke、rollback 审计更容易回答“Rust 到底在哪
  一层还持有 raw request owner，哪一层开始只剩 runtime-ready owner”。

  这条 seam 已通过：
  - `cargo test should_keep_batch_generation_create_runtime_seed_contract --manifest-path "backend-rs/Cargo.toml" -- --nocapture`
  - `cargo test should_build_batch_generation_create_workflow_runtime_parts_from_runtime_seed --manifest-path "backend-rs/Cargo.toml" -- --nocapture`
  - `cargo test should_build_batch_generation_create_workflow_launch_into_persistence_plan --manifest-path "backend-rs/Cargo.toml" -- --nocapture`
  - `cargo check`

  - 2026-06-03 同一条 batch create Phase 5 lane 继续沿 create prepare 的
    request-runtime owner 收口后，execution-config 也不再从 raw request 平行取
    `model_override`，而是和 startup runtime-seed 一样由同一份
    `BatchGenerationRequestRuntimeState` 驱动。当前：
    - `PreparedBatchGenerationCreateWorkflowLaunch::prepare(...)` 先构造一份
      `request_runtime_state`
    - `BatchGenerationCreateRuntimeSeed::prepare(...)` 改为借用
      `&BatchGenerationRequestRuntimeState`
    - `prepare_generation_execution_config(...)` 现在也直接读取
      `request_runtime_state.model_override`
    - create prepare 路径不再对同一个 execution-config 边界同时依赖：
      - raw workflow request owner
      - derived request-runtime owner

  这条 seam 的意义不是“把 move 改成 borrow”，而是在 batch create write-lane
  上继续删掉一个真正的 mixed owner hop。此前 create lane 虽然已经把
  startup-seed 和 compat restore 收回到了 runtime-state / payload owner，但
  execution-config 的 model selection 仍然要回头从 raw request 读
  `model_override`。现在这一步也收回后，create prepare 更接近 cutover 需要的
  单一 owner 形态：同一份 `BatchGenerationRequestRuntimeState` 同时驱动
  runtime seed 和 downstream execution-config 选择。

  对 Phase 5 的价值同样直接：这让 batch create 在 prepare 阶段又少了一层
  Python-era 平行输入依赖，也让后续关于 create persistence/response assembly、
  runtime-dispatch-ready owner 的审计更容易回答“Rust 到底从哪一步开始只剩
  request-runtime owner，而不再回头依赖 raw request”。

  这条 seam 已通过：
  - `cargo test should_keep_batch_generation_create_runtime_seed_contract --manifest-path "backend-rs/Cargo.toml" -- --nocapture`
  - `cargo test should_build_batch_generation_create_workflow_runtime_parts_from_runtime_seed --manifest-path "backend-rs/Cargo.toml" -- --nocapture`
  - `cargo check`

  - 2026-06-03 同一条 batch create Phase 5 lane 再继续沿 payload-owned
    runtime-seed restore 收口后，create startup seed 的 compat owner 也开始
    直接从 runtime payload 自恢复，而不再依赖并行传入的 raw request state。
    当前：
    - `BatchGenerationCreateRuntimeSeed::from_runtime_state_payload(...)`
      只接收 `runtime_state_payload`
    - 它会通过
      `parse_batch_generation_request_runtime_state(Some(&runtime_state_payload))`
      自行恢复 `BatchGenerationRequestRuntimeState`
    - `from_startup_runtime_state(...)` 也只再向前传递 payload owner
    - create runtime-seed 测试与 workflow-launch 契约测试都改为只断言
      payload-owned restore 路径

  这条 seam 的意义不是“构造函数又少一个参数”，而是在 batch create write-lane
  上继续删掉一层 persisted payload 之外的假 owner hop。此前即使 runtime-seed
  已经不保存 raw request state，create lane 仍然需要通过
  `from_runtime_state_payload(request_runtime_state, runtime_state_payload)` 这种双输入
  方式来恢复 compat owner；但同一份 payload 里早已内嵌
  `batch_request_runtime_state`。现在 compat restore 也改成 payload-owned 之后，
  create lane 的 startup restore 边界更像相邻 batch resume lane 的
  persisted-source owner 形态，也更接近 cutover 时需要的单点恢复路径。

  对 Phase 5 的价值同样直接：这让 batch create 的 runtime-seed 恢复边界
  不再保留第二份 parallel raw request owner，也让后续关于 startup payload
  restoration、create persistence assembly、runtime dispatch ownership 的审计
  更容易回答“Rust 到底在哪一步从 persisted payload 完成 startup 恢复”。

  这条 seam 已通过：
  - `cargo test should_keep_batch_generation_create_runtime_seed_contract --manifest-path "backend-rs/Cargo.toml" -- --nocapture`
  - `cargo test should_build_batch_generation_create_workflow_runtime_parts_from_runtime_seed --manifest-path "backend-rs/Cargo.toml" -- --nocapture`
  - `cargo test should_build_batch_generation_create_workflow_launch_into_persistence_plan --manifest-path "backend-rs/Cargo.toml" -- --nocapture`
  - `cargo check`

  - 2026-06-03 同一条 batch resume Phase 5 lane 继续向 prepare ->
    persistence 边界收口后，resume write-lane 的 reset persistence 也已经从
    prepare-time 隐式副作用收成显式 owner。当前：
    - `prepare_batch_generation_resume(...)` 不再在内部直接执行
      reset persistence
    - 它现在只返回显式的
      `BatchGenerationResumeLaunchPersistencePlan`
    - 新的 resume persistence owner 现在显式持有：
      - `ResumeBatchGenerationCommandState`
      - `ResumeExecutionDispatchPlan`
      - `resume_runtime_state_seed`
    - `resume_owned_batch_generation_write_workflow(...)` 现在更接近 batch
      create 的形态：
      `load -> prepare -> persist_and_dispatch`

  这条 seam 的意义不是“多了一层 plan struct”，而是在 batch resume write-lane
  上继续拆开 prepare 与 persistence 的混合所有权。此前 resume prepare
  已经完成 restored runtime-state 恢复、selection validation 和 runtime
  dispatch contract 准备，但还在同一步里隐式执行 reset persistence；现在这一跳
  被拉成显式 persistence owner 后，resume write-lane 的生命周期边界会更清楚，
  也更接近当前 batch create 已经形成的 owner 图。

  对 Phase 5 的价值同样直接：这让 create / resume 两条 write-lane 在
  `prepare -> persistence -> dispatch` 结构上进一步对齐，也让 fallback shrink、
  stronger smoke、rollback 审计更容易回答“Rust 到底在哪一层改变持久化状态，
  在哪一层进入 runtime dispatch”。这比继续做局部字段整理更接近真正 cutover
  readiness 所需的 owner clarity。

  这条 seam 已通过：
  - `cargo test chapter_batch_generation_resume_task_command_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`
  - `cargo check`

  因而当前 `chapter_generation` Phase 5 lane 的高信号节奏仍然成立：
  - 继续优先推进与 shared runtime handoff 相邻的 seam
  - 只在能减少真实 lifecycle duplication 时才继续收口
  - 不把无关广域失败混入当前迁移 patch

  - 2026-06-03 同一条 batch resume Phase 5 lane 继续沿 persisted runtime
    source 恢复边界收口后，resume write-lane 上的
    `request_runtime_state` 恢复所有权也已经从 write workflow 收回
    resume command owner。当前：
    - `prepare_batch_generation_resume(...)` 会直接从 `snapshot` 内部恢复
      `workflow_runtime_state`
    - 它会在同一 owner 内解析
      `BatchGenerationRequestRuntimeState`
    - `load_owned_batch_generation_resume_dependencies(...)` 不再返回本地
      解析后的 request runtime state，而只保留 write workflow 仍真实拥有的
      依赖：
      - `ResumeBatchGenerationCommandState`
      - optional snapshot
    - `resume_owned_batch_generation_write_workflow(...)` 现在只负责 load，
      然后把 snapshot 直接交给 resume command owner 继续 prepare

  这条 seam 的意义不是“函数参数少了一个”，而是在 batch resume write-lane
  上继续收回同一份 persisted source 的恢复所有权。此前 write workflow 还要
  先本地解析 `workflow_runtime_state -> BatchGenerationRequestRuntimeState`，
  但同一条 persisted-source 恢复链上的其余语义，包括质量上下文恢复、
  active story-repair payload 恢复、runtime seed 重建，都已经在
  `chapter_batch_generation_resume_task_command_service.rs`。现在这段恢复也一起
  收回后，`load -> prepare -> persist_and_dispatch` 的 owner 边界更一致，
  也更接近真正 cutover 所需的 Rust 单点恢复路径。

  对 Phase 5 的价值同样直接：这让 batch resume 的 write-lane 不再在 workflow
  和 command 两层之间拆分同一个 persisted runtime source 的恢复职责，
  进一步缩短了 resume owner 图，也让后续 fallback shrink、cutover smoke、
  rollback 审计更容易回答“究竟是谁在恢复 persisted resume runtime state”。

  这条 seam 已通过：
  - `cargo test should_block_resume_when_runtime_active_story_repair_payload_requires_manual_review --manifest-path "backend-rs/Cargo.toml" -- --nocapture`
  - `cargo test should_block_resume_when_quality_summary_requires_manual_review_even_without_failed_chapter_label --manifest-path "backend-rs/Cargo.toml" -- --nocapture`
  - `cargo test should_keep_resume_batch_generation_dependency_loader_contract_explicit --manifest-path "backend-rs/Cargo.toml" -- --nocapture`
  - `cargo test chapter_batch_generation_resume_task_command_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`
  - `cargo check`

  - 2026-06-03 同一条 batch resume Phase 5 lane 继续沿 validated execution
    boundary 收口后，single-chapter resume 也不再用一个 `Option`-态 enum
    横跨“验证前 selection”和“验证后 dispatch-ready target”两个生命周期阶段。
    当前：
    - `ResumeExecutionEligibilityPlan` 只保留 validation-before owner：
      - `SingleChapter { chapter_id }`
      - `Batch { chapter_ids }`
    - 新增 `ValidatedResumeExecutionPlan` 作为 validation-after owner：
      - `SingleChapter { validated_single_chapter_target }`
      - `Batch { chapter_ids }`
    - `validate_access_and_prerequisites(...)` 现在直接返回
      `ValidatedResumeExecutionPlan`
    - `prepare_dispatch_plan(...)` 只挂在 validated owner 上，single resume
      dispatch 直接从 `SingleChapterGenerationTarget` 取 `chapter_id`

  这条 seam 的意义不是“又多一个 enum”，而是在 batch resume write-lane 上
  继续删掉一个会跨两个生命周期阶段的假 owner。此前
  `ResumeExecutionEligibilityPlan::SingleChapter` 既表示“只选中了 chapter_id”，
  又试图在同一个类型上通过
  `validated_single_chapter_target: Option<SingleChapterGenerationTarget>`
  过渡到“已经验证完、可以直接组装 dispatch”的状态。这会让 Rust 在类型层
  继续保留一个 impossible state：dispatch-ready 路径仍然允许“validated target
  不存在”。现在 validation-before 和 validation-after 被拆成两个显式 owner
  后，batch resume 的生命周期边界更清楚，也更接近 cutover 时需要的单阶段
  明确 owner 图。

  对 Phase 5 的价值同样直接：这让 batch resume 的 single-chapter path 又少了
  一层 fake state transition，也让后续关于 validated execution ownership、
  dispatch assembly、fallback shrink 的审计更容易回答“Rust 到底在哪一步完成
  了 single resume 的真实验证，并从哪一步开始只剩 dispatch-ready owner”。

  这条 seam 已通过：
  - `cargo test chapter_batch_generation_resume_task_command_service --manifest-path "backend-rs/Cargo.toml" -- --nocapture`
  - `cargo check`

  - 2026-06-03 同一条 batch create Phase 5 lane 又继续沿 queued response
    projection 收口后，create write-lane 的 queued response 也不再从同一组
    queued owner 字段重复重建，而是被提前收成显式 persistence owner 的一部分。
    当前：
    - `BatchGenerationCreateLaunchPersistencePlan` 现在直接持有
      `response_payload`
    - `from_workflow_launch(...)` 会在 persistence owner 构造阶段就生成这份
      queued create response payload
    - `response_payload()` 与 `persist_and_dispatch(...)` 都改为复用同一份
      `response_payload`
    - create lane 不再在 queued response projection 路径里重复从 task/checkpoint/
      summary owner 字段做一次平行 rebuild

  这条 seam 的意义不是“少调一次 builder”。它真正删掉的是 batch create
  write-lane 上一段仍残留的 Python-era 生命周期重复：此前 queued create
  persistence assembly 与 queued response assembly 虽然都依赖同一组 owner，
  但 Rust 仍把它们保留成两条平行重组路径。现在 queued response 也进入显式
  persistence owner 后，create lane 更接近 cutover 所需的单一
  `launch -> persistence-ready owner -> dispatch/response projection` 形态。

  对 Phase 5 的价值同样直接：这让 create lane 的 queued-lifecycle owner 图继续
  变窄，也让后续关于 create persistence/response assembly、shared runtime
  handoff、fallback shrink 审计更容易回答“Rust 到底在哪一步就已经拥有最终的
  queued response projection，而不再回头 rebuild”。

  这条 seam 已通过：
  - `cargo test should_keep_batch_generation_create_runtime_seed_contract --manifest-path "backend-rs/Cargo.toml" -- --nocapture`
  - `cargo test should_build_batch_generation_create_workflow_runtime_parts_from_runtime_seed --manifest-path "backend-rs/Cargo.toml" -- --nocapture`
  - `cargo test should_build_batch_generation_create_workflow_launch_into_persistence_plan --manifest-path "backend-rs/Cargo.toml" -- --nocapture`
  - `cargo test should_build_batch_generation_create_persistence_plan_task_and_response_payload --manifest-path "backend-rs/Cargo.toml" -- --nocapture`
  - `cargo check`

  - 2026-06-03 同一条 batch resume Phase 5 lane 在显式 persistence owner
    之后又继续向 final reset-ready owner 收口后，resume write-lane 的最后一段
    reset persistence / response projection 隐式 rebuild 也已经被移除。当前：
    - `BatchGenerationResumeLaunchPersistencePlan` 不再保存
      `resume_runtime_state_seed` 以待后续重建
    - 它现在直接持有：
      - `BatchGenerationResumeResetPersistencePlan`
      - `response_payload`
    - `BatchGenerationResumeLaunchPersistencePlan::new(...)` 会在构造阶段一次性完成：
      - final reset-ready persistence owner 组装
      - queued resume response payload 组装
    - `response_payload()` 与 `persist_and_dispatch(...)` 现在都复用同一份
      reset-ready owner，而不再走晚期 rebuild

  这条 seam 的意义不是“把 seed 换成 plan 字段”。它真正删掉的是 batch resume
  write-lane 上最后一段 prepare 之后仍会分叉的 fake lifecycle hop：此前 Rust
  虽然已经把 resume prepare 与 persistence 拆开，但最终 reset-ready owner 仍然
  要在 response/persist path 里从 `resume_runtime_state_seed` 再造一次。现在这一跳
  也被收回后，resume lane 更清楚地进入了与 batch create 对齐的
  `prepare -> persistence-ready owner -> dispatch` 结构。

  对 Phase 5 的价值同样直接：这让 create / resume 两条 write-lane 不只是
  “都有 persistence owner”，而是开始同时拥有更完整的
  response-ready / reset-ready final owner；后续关于 runtime handoff、
  rollback、fallback shrink 与 stronger smoke 的审计，也更容易回答
  “Rust 到底在哪一步已经拥有最终 reset-ready checkpoint，而不再依赖隐藏重建”。

  这条 seam 已通过：
  - `cargo test should_prepare_resume_launch_with_restored_request_runtime_state_owner --manifest-path "backend-rs/Cargo.toml" -- --nocapture`
  - `cargo test should_build_resume_persistence_plan_with_shared_reset_and_dispatch_owner --manifest-path "backend-rs/Cargo.toml" -- --nocapture`
  - `cargo test should_keep_resume_execution_and_payload_contract_explicit --manifest-path "backend-rs/Cargo.toml" -- --nocapture`
  - `cargo test should_build_dispatch_plan_from_resume_persistence_plan_owner --manifest-path "backend-rs/Cargo.toml" -- --nocapture`
  - `cargo check`

  - 2026-06-03 同一条 `chapter_generation` Phase 5 lane 继续沿 single startup
    launch assembly 收口后，single prepare owner 也开始直接持有
    request-runtime owner，并贯通到 restored runtime launch。当前：
    - `PreparedSingleChapterGenerationExecution` 现在直接持有
      `BatchGenerationRequestRuntimeState`
    - `prepare_validated_single_chapter_generation_request_from_target(...)`
      在 prepare 阶段已经完成 request-runtime owner 的显式构造
    - `PreparedSingleChapterGenerationRestoredRuntimeLaunch::prepare_from_prepared_execution(...)`
      现在直接消费这份 owner，而不再从：
      - `execution_input.compat_options`
      - `request.model`
      再平行重建一份 request-runtime owner

  这条 seam 的意义不是“prepare struct 多带了一个字段”，而是在 single background
  write-lane 上继续删掉一条真实的 Python-era owner 重建路径。此前 single lane
  虽然已经把 access / prerequisite / target / execution-config 往 prepare owner
  收拢，但 restored runtime launch 仍会重新拼回一份 request-runtime state；现在
  这条 hop 也被移除后，single-lane startup restore 更接近 batch create /
  batch resume 已经在推进的单一 owner 形态。

  对 Phase 5 的价值同样直接：这让 `chapter_generation` 主线不再只在 batch lane
  缩 owner，也把 single-lane 的 startup-to-runtime handoff 继续向 cutover-ready
  形态推进。后续关于 startup snapshot、runtime restore、fallback shrink 与
  stronger smoke 的审计，也更容易回答“Rust 到底从哪一步开始只剩 prepare-time
  request-runtime owner，而不再回头重建”。

  这条 seam 已通过：
  - `cargo test should_project_prepared_single_chapter_generation_execution_owner --manifest-path "backend-rs/Cargo.toml" -- --nocapture`
  - `cargo test should_build_single_generation_background_launch_persistence_plan_from_prepared_owner --manifest-path "backend-rs/Cargo.toml" -- --nocapture`
  - `cargo test should_convert_restored_single_generation_launch_into_runtime_input --manifest-path "backend-rs/Cargo.toml" -- --nocapture`
  - `cargo test should_build_single_generation_runtime_launch_input_from_restored_seed --manifest-path "backend-rs/Cargo.toml" -- --nocapture`
  - `cargo check`

  - 2026-06-03 同一条 batch create Phase 5 lane 再继续向 startup runtime-state
    owner 收口后，create startup owner 也开始直接持有 request-runtime owner，
    而 runtime-seed owner 直接消费这份 owner 但自身保持 dispatch-ready
    形态。当前：
    - `BatchGenerationCreateStartupRuntimeState` 现在直接持有
      `BatchGenerationRequestRuntimeState`
    - `BatchGenerationCreateStartupRuntimeState::prepare(...)` /
      `from_recent_history_summary(...)` 在 startup 阶段就完成了
      request-runtime owner 的显式构造与携带
    - `BatchGenerationCreateRuntimeSeed::from_startup_runtime_state(...)`
      现在直接消费这份 owner，而不再从 `runtime_state_payload`
      再平行反解析一份 request-runtime owner
    - create workflow prepare 侧对 `model_override` 的读取现在也来自
      同一份 startup-stage request-runtime owner，而不是在 seed 邻域再保留
      一份平行 request owner

  这条 seam 的意义不是“startup struct 多带了一个字段”，而是在 batch create
  write-lane 上继续删掉一条真实的 Python-era owner 重建路径。此前 batch create
  虽然已经把 request-runtime owner / runtime seed payload / queued response
  往 prepare owner 收拢，但 startup runtime-state 到 runtime-seed 之间仍会再从
  payload parse 回一份 request-runtime state；现在这条 hop 被移除后，batch create
  startup boundary 更接近 single background / batch resume 已在推进的单一 owner
  形态。

  对 Phase 5 的价值同样直接：这让 `chapter_generation` 主线在 batch create lane
  上对 startup-to-runtime handoff 的 owner 链又少了一次“先序列化再反解析”的
  历史路径，同时 runtime-seed 继续保持 dispatch-ready owner 的窄边界。后续
  关于 startup snapshot、fallback shrink、rollback 边界和
  stronger smoke 的审计，也更容易回答“Rust 从哪一步开始只剩显式
  request-runtime owner，而不再依赖 payload parse 回填”。

  这条 seam 已通过：
  - `cargo test should_build_batch_generation_create_runtime_seed_from_startup_owner --manifest-path "backend-rs/Cargo.toml" -- --nocapture`
  - `cargo test should_build_batch_generation_create_workflow_runtime_parts_from_runtime_seed --manifest-path "backend-rs/Cargo.toml" -- --nocapture`
  - `cargo test should_build_batch_generation_create_startup_runtime_state_from_request_only --manifest-path "backend-rs/Cargo.toml" -- --nocapture`
  - `cargo test should_build_batch_generation_create_startup_runtime_state_from_recent_history_summary --manifest-path "backend-rs/Cargo.toml" -- --nocapture`
  - `cargo test should_build_batch_generation_create_persistence_plan_task_and_response_payload --manifest-path "backend-rs/Cargo.toml" -- --nocapture`
  - `cargo check`

  - 2026-06-03 同一条 `chapter_generation` Phase 5 lane 继续沿 single background
    persistence assembly 收口后，single background persistence plan 也开始只保留
    `runtime_input` 作为派生字段 owner。当前：
    - `SingleGenerationBackgroundLaunchPersistencePlan` 现在只保留：
      - `task_id`
      - `chapter_target`
      - `startup_snapshot_plan`
      - `response_payload`
      - `runtime_input`
    - 持久化计划不再平行持有：
      - `user_id`
      - `target_word_count`
    - `background_task_active_model(...)` 现在直接从：
      - `runtime_input.user_id`
      - `runtime_input.execution_input.target_word_count`
      派生 background task 所需字段

  这条 seam 的意义不是“删了两个字段”，而是在 single background
  write-lane 上继续删掉一层真实的 Python-era owner 重复。此前 single lane
  虽然已经把 prepare-time request-runtime owner 收进 restored runtime launch，
  但 persistence assembly 仍会再平行携带一份 `user_id` / `target_word_count`
  副本；现在这条重复 owner hop 被移除后，single background persistence
  boundary 更接近 batch create / batch resume 已在推进的单一 owner 形态。

  对 Phase 5 的价值同样直接：这让 `chapter_generation` 主线不只是在 startup
  restore 阶段收 owner，也把 single background write-lane 的 task projection
  继续向 cutover-ready 形态推进。后续关于 existing-background payload reuse、
  startup snapshot、fallback shrink 与 stronger smoke 的审计，也更容易回答
  “Rust 到底从哪一步开始只剩 runtime_input owner，而不再在 persistence
  plan 再平行复制派生字段”。

  这条 seam 已通过：
  - `cargo test should_build_single_generation_background_launch_persistence_plan_from_prepared_owner --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-single-write-owner-contract" -- --nocapture`
  - `cargo test should_keep_single_generation_background_persistence_plan_runtime_input_owner_contract --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-single-write-owner-contract" -- --nocapture`
  - `cargo test should_keep_single_generation_background_active_model_defaults --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-single-write-owner-contract" -- --nocapture`
  - `cargo test should_build_single_generation_background_response_payload_from_runtime_seed --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-single-write-owner-contract" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-single-write-owner-contract"`

  - 2026-06-03 同一条 `chapter_generation` Phase 5 lane 继续沿 single existing
    background payload reuse 收口后，single 既有后台任务复用路径也开始直接消费
    Rust read-context owner，而不再把它拆成三个并行字段再传一跳。当前：
    - `single_generation_existing_background_task_payload(...)` 现在直接接收
      `BatchGenerationReadContext`
    - 该 helper 在内部再显式解构：
      - `task`
      - `workflow_runtime_state`
      - `quality_status_context`
    - `load_existing_single_generation_background_task_payload(...)` 现在把
      已加载的 read-context owner 直接传给 payload helper，而不再先拆开
      再重组一遍

  这条 seam 的意义不是“函数参数从三个改成一个”，而是在 single background
  write-lane 的 existing-task reuse 路径上继续删掉一条真实的 Python-era owner
  hop。此前 Rust 已经通过共享 batch read-lane owner 构造出了完整
  `BatchGenerationReadContext`，但 single-generation 既有后台任务 payload
  仍会把它拆成 `task/runtime_state/quality_context` 三段再传入 helper；现在这条
  重复 hop 被移除后，single existing-background reuse boundary 更接近
  batch read-side owner 已在推进的单一 owner 形态。

  对 Phase 5 的价值同样直接：这让 `chapter_generation` 主线不只是在
  startup restore / persistence assembly 阶段缩 owner，也把
  existing-background reuse 这条真实兼容路径继续向 cutover-ready 形态推进。
  后续关于 existing-task reuse、fallback shrink 与 stronger smoke 的审计，
  也更容易回答“Rust 到底从哪一步开始直接复用 read-context owner，而不再在
  single lane 里平行拆包重传”。

  这条 seam 已通过：
  - `cargo test should_preserve_richer_quality_runtime_contract_on_existing_single_generation_background_payload --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-single-write-owner-contract" -- --nocapture`
  - `cargo test should_keep_single_generation_existing_background_payload_read_context_owner_contract --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-single-write-owner-contract" -- --nocapture`
  - `cargo test should_build_single_generation_background_response_payload_from_runtime_seed --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-single-write-owner-contract" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-single-write-owner-contract"`

  - 2026-06-03 同一条 `chapter_generation` Phase 5 lane 再继续沿 single
    background restored-launch -> persistence 收口后，single background
    persistence-plan 也开始直接消费 restored-launch owner，而不再先拆成局部
    字段再重投影同一边界。当前：
    - `PreparedSingleGenerationBackgroundLaunch::prepare_from_target(...)`
      现在把完整
      `PreparedSingleChapterGenerationRestoredRuntimeLaunch`
      直接传给 persistence-plan owner
    - `SingleGenerationBackgroundLaunchPersistencePlan::from_restored_launch(...)`
      现在直接接收这份 restored-launch owner，并在内部解构：
      - `chapter_target`
      - `runtime_state_payload`
      - `runtime_input`
    - test-only 的 `from_prepared_request(...)` 也改为先构造同一份
      restored-launch owner，再走同一条 persistence-plan owner 边界，而不再
      绕开它本地平行投影

  这条 seam 的意义不是“把 `into_parts()` 挪了个位置”，而是在 single
  background write-lane 的 startup-to-persistence handoff 上继续删掉一条真实的
  Python-era owner hop。此前 Rust 已经有明确的
  `PreparedSingleChapterGenerationRestoredRuntimeLaunch` owner，但 write
  workflow 仍会立即把它拆成三段，再交给 persistence-plan 重建同一 owner；
  现在这条重复 hop 被移除后，single background persistence boundary 更接近
  batch create / batch resume 已在推进的单一 owner 形态。

  对 Phase 5 的价值同样直接：这让 `chapter_generation` 主线不只是在
  request-runtime owner、runtime-input owner、existing-background reuse owner
  上持续收口，也把 restored-launch 到 persistence-plan 这一跳继续向
  cutover-ready 形态推进。后续关于 startup restore、fallback shrink、
  rollback 边界与 stronger smoke 的审计，也更容易回答“Rust 到底从哪一步
  开始只剩 restored-launch owner，而不再在 write lane 里平行拆包重投影”。

  这条 seam 已通过：
  - `cargo test should_keep_single_generation_background_persistence_plan_restored_launch_owner_contract --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-single-write-owner-contract" -- --nocapture`
  - `cargo test should_keep_single_generation_background_persistence_plan_runtime_input_owner_contract --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-single-write-owner-contract" -- --nocapture`
  - `cargo test should_build_single_generation_background_launch_persistence_plan_from_prepared_owner --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-single-write-owner-contract" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-single-write-owner-contract"`

  - 2026-06-03 同一条 batch resume Phase 5 lane 再继续沿 restored
    runtime-state -> persistence 边界收口后，resume persistence-plan owner
    也开始直接消费 restored runtime-state owner，而不再先拆成
    `request_runtime_state` / `runtime_state_seed` 两个平行字段再重投影同一边界。
    当前：
    - `ValidatedResumeExecutionPlan::prepare_dispatch_plan(...)` 现在只借用
      `BatchGenerationRequestRuntimeState`
    - `prepare_batch_generation_resume_persistence_plan(...)` 现在直接接收
      `RestoredResumeRuntimeState`
    - `prepare_batch_generation_resume(...)` 不再本地拆出
      `request_runtime_state` 与 `runtime_state_seed`
    - focused owner test
      `should_keep_resume_persistence_plan_restored_runtime_state_owner_contract`
      锁住了这条新的 persistence owner 边界

  这条 seam 的意义不是“参数少了两个”，而是在 batch resume write-lane 的
  restored-state-to-persistence handoff 上继续删掉一条真实的 Python-era owner
  hop。此前 Rust 已经有明确的 `RestoredResumeRuntimeState` owner，里面已经同时
  持有恢复后的 request runtime state 与 resume runtime-state seed，但
  `prepare_batch_generation_resume(...)` 仍会立刻把它拆成两段，再交给
  persistence-plan 重建同一条边界；现在这条重复 hop 被移除后，batch resume
  persistence boundary 更接近 single background / batch create 已在推进的单一
  owner 形态。

  对 Phase 5 的价值同样直接：这让 `chapter_generation` 主线不只是在
  reset-ready persistence、validated execution、single background restored
  launch 等边界上持续收口，也把 batch resume 的 restored runtime-state ->
  persistence 这一跳继续向 cutover-ready 形态推进。后续关于 fallback shrink、
  rollback 边界与 stronger smoke 的审计，也更容易回答“Rust 到底从哪一步开始
  只剩 restored runtime-state owner，而不再在 resume write lane 里平行拆包
  重投影”。

  这条 seam 已通过：
  - `cargo test should_keep_resume_persistence_plan_restored_runtime_state_owner_contract --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-resume-owner-contract" -- --nocapture`
  - `cargo test should_build_resume_persistence_plan_with_shared_reset_and_dispatch_owner --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-resume-owner-contract" -- --nocapture`
  - `cargo test should_build_dispatch_plan_from_resume_persistence_plan_owner --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-resume-owner-contract" -- --nocapture`
  - `cargo test should_build_resume_runtime_state_seed_owner_with_restored_contracts --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-resume-owner-contract" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-resume-owner-contract"`

  - 2026-06-03 同一条 batch resume Phase 5 lane 再继续沿 reset-ready
    persistence -> response 边界收口后，resume response owner 也开始直接消费
    `BatchGenerationResumeResetPersistencePlan`，而不再回头从
    `ResumeBatchGenerationCommandState` 重新 `resolve_reset_semantics()` 再投影同一批
    reset 字段。当前：
    - `BatchGenerationResumeResetPersistencePlan` 现在显式暴露：
      - `total_chapters()`
      - `completed_chapters()`
      - `status()`
      - `current_chapter_id()`
    - `BatchGenerationResumeLaunchPersistencePlan::new(...)` 现在把完整
      reset persistence owner 直接交给
      `build_batch_generation_resume_response_payload(...)`
    - response builder 现在直接从 reset owner 取：
      - reset status
      - reset current chapter pointer
      - reset completed chapters
      - reset total chapters
      - reset checkpoint
    - focused owner test
      `should_keep_resume_response_payload_reset_persistence_owner_contract`
      锁住了这条新的 response owner 边界

  这条 seam 的意义不是“多了几个 getter”，而是在 batch resume write-lane 的
  reset-ready-to-response handoff 上继续删掉一条真实的 Python-era owner hop。
  此前 Rust 已经有明确的 `BatchGenerationResumeResetPersistencePlan` owner，
  里面已经同时持有 reset checkpoint、reset status、reset current chapter 与
  reset completed/total contract，但 response payload 仍会回头从 command state
  再解一遍 reset 语义；现在这条重复 hop 被移除后，batch resume response
  boundary 更接近 single-owner 的 cutover-ready 形态。

  对 Phase 5 的价值同样直接：这让 `chapter_generation` 主线不只是在 restored
  runtime-state、validated execution、persistence-plan 等边界上持续收口，也把
  batch resume 的 reset-ready persistence -> response 这一跳继续向 cutover-ready
  形态推进。后续关于 fallback shrink、rollback 边界与 stronger smoke 的审计，
  也更容易回答“Rust 到底从哪一步开始只剩 reset persistence owner，而不再在
  response 组装时平行重投影同一套 reset 字段”。

  这条 seam 已通过：
  - `cargo test should_keep_resume_response_payload_reset_persistence_owner_contract --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-resume-response-owner-contract" -- --nocapture`
  - `cargo test should_build_resume_response_payload_from_owner --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-resume-response-owner-contract" -- --nocapture`
  - `cargo test should_prepare_resume_launch_with_restored_request_runtime_state_owner --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-resume-response-owner-contract" -- --nocapture`
  - `cargo test should_build_resume_persistence_plan_with_shared_reset_and_dispatch_owner --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-resume-response-owner-contract" -- --nocapture`
  - `cargo test should_keep_resume_persistence_plan_restored_runtime_state_owner_contract --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-resume-response-owner-contract" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-resume-response-owner-contract"`

  - 2026-06-03 同一条 batch resume Phase 5 lane 再继续沿 reset-ready
    persistence owner -> response projection 的更深一层收口后，resume response
    不只是不再从 command state 重新解 reset 语义，也不再在本地直接从 raw
    `resume_checkpoint` 平行恢复同一批 quality / story-repair / history 语义。当前：
    - `BatchGenerationResumeResetPersistencePlan` 现在进一步显式暴露：
      - single-task `GenerationQualityRuntimeContext`
      - batch-task `BatchGenerationQualityRuntimeContext`
      - `latest_quality_metrics`
      - `quality_metrics_history`
      - `quality_metrics_summary_state`
      - `quality_metrics_summary`
      - `active_story_repair_payload`
      - task-kind-aware `quality_history_context`
    - `build_batch_generation_resume_response_payload(...)` 现在直接从
      reset persistence owner 读取这些 response-side owner 语义，而不再：
      - `resume_checkpoint.get(...)` 本地读 quality 字段
      - 本地重新 resolve single/batch quality runtime context
      - 本地从 raw checkpoint 回补 `active_story_repair_payload`
      - 本地再拼一条 `quality_history_context`
    - focused owner tests
      `should_build_resume_response_payload_from_owner` 与
      `should_keep_resume_response_payload_reset_persistence_owner_contract`
      继续锁住这条更深一层的 response owner 边界

  这条 seam 的意义不是“给 reset owner 再多挂几个 accessor”，而是在
  batch resume write-lane 的 reset persistence -> response projection 这一跳，
  继续删掉一条更深的 Python-era owner seam：owner 明明已经存在，response
  helper 却仍退回 raw checkpoint 再本地恢复同一层语义。现在这一跳被收掉后，
  Rust 可以更明确地回答：
  “从 reset persistence owner 开始，resume response 读取的是显式 owner
  contract，而不是隐式 checkpoint 解析副本。”

  对 Phase 5 的价值同样直接：这让 `chapter_generation` 主线不只是在
  reset field projection 上接近 cutover-ready，还把 quality / story-repair /
  history 这些高信号 response-side 语义也继续并到同一个 reset persistence
  owner 下。后续做 fallback shrink、rollback 边界和 stronger smoke 审计时，
  也更容易判断 Rust 是否已经在 resume response 这一跳真正拥有单一 owner。

  这条 seam 已通过：
  - `cargo test should_build_resume_response_payload_from_owner --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-resume-response-reset-owner" -- --nocapture`
  - `cargo test should_keep_resume_response_payload_reset_persistence_owner_contract --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-resume-response-reset-owner" -- --nocapture`
  - `cargo test should_keep_resume_execution_and_payload_contract_explicit --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-resume-response-reset-owner" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-resume-response-reset-owner"`

  - 2026-06-03 同一条 batch resume Phase 5 lane 再继续沿 persisted runtime
    owner -> restored runtime-state assembly 收口后，resume 在恢复
    quality/compat/story-repair/status 语义时也开始直接消费
    `BatchGenerationPersistedRuntimeContext`，而不再在
    `chapter_batch_generation_resume_task_command_service.rs` 内部重放一套
    本地 persisted-source rebuild。当前：
    - `BatchGenerationPersistedRuntimeContext` 现在进一步显式暴露：
      - `restored_quality_runtime_context(task_kind)`
      - `restored_resume_compat_options(...)`
      - `resolved_resume_active_story_repair_payload(...)`
      - `resume_quality_status_context(...)`
    - `RestoredResumeRuntimeState::from_persisted_runtime_context(...)`
      现在直接消费这些 owner helper，而不再本地继续做：
      - persisted quality runtime-context rebuild
      - resume compat restore
      - active story-repair payload resolve
      - quality-status-context projection
    - 旧的本地 helper 只保留为 `#[cfg(test)]` 包装，生产逻辑不再重复
      持有这条 persisted-source 恢复链

  这条 seam 的意义不是“把几个函数搬进 owner 上”，而是在 batch resume
  write-lane 的 persisted-source 恢复边界继续删掉一条更深的 Python-era owner
  seam：shared persisted runtime owner 明明已经存在，resume service 却仍要
  本地 replay 同一套恢复语义。现在这条重复 hop 被收掉后，Rust 可以更清楚地
  回答：
  “从 persisted runtime owner 开始，resume restored runtime-state
  assembly 读取的是显式 owner contract，而不是 resume service 私有的恢复副本。”

  对 Phase 5 的价值同样直接：这让 `chapter_generation` 主线不只是在
  persisted runtime refresh、reset persistence、response projection 这些边界
  上收口，也把 resume 的 restored-runtime-state assembly 继续并回同一条
  persisted owner 链。后续做 fallback shrink、rollback 边界和 stronger smoke
  审计时，也更容易判断 Rust 是否已经在 resume persisted-source 恢复这一跳
  真正拥有单一 owner。

  这条 seam 已通过：
  - `cargo test should_restore_resume_runtime_state_from_shared_persisted_runtime_context_owner --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-resume-persisted-owner" -- --nocapture`
  - `cargo test should_restore_single_resume_compat_options_from_restored_history_only_quality_context --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-resume-persisted-owner" -- --nocapture`
  - `cargo test should_restore_single_resume_active_story_repair_payload_from_restored_history_only_quality_context --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-resume-persisted-owner" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-resume-persisted-owner"`

  - 2026-06-03 同一条 batch runtime Phase 5 lane 又继续沿 persisted owner ->
    runtime payload rebuild 的相邻消费点收口后，runtime 侧的 compat restore /
    story-repair refresh / current-quality snapshot 也开始直接消费
    `BatchGenerationPersistedRuntimeContext` 的 owner-projected payload，而不再在
    三个调用点各自把相同 persisted 字段重新拆包再传给下游 builder。当前：
    - `BatchGenerationPersistedRuntimeContext` 现在进一步显式暴露：
      - `restored_batch_runtime_compat_options(...)`
      - `build_refreshed_runtime_state_preserving_quality(...)`
      - `build_current_chapter_quality_runtime_snapshot(...)`
    - `restore_batch_generation_runtime_compat_options_from_persisted_runtime_context(...)`
      现在直接委托给 persisted owner，而不再本地恢复 batch quality +
      story-repair 语义
    - `refresh_batch_generation_runtime_story_repair_state(...)` 现在直接消费
      owner 返回的 rebuilt runtime-state payload，而不再把
      `request_runtime_state` / `explicit_story_repair_payload` /
      `quality_metrics_*` 字段串成一长串参数
    - `build_batch_generation_current_chapter_quality_runtime_snapshot(...)`
      现在也直接消费 owner 投影结果，而不再重放同一套 persisted-source
      参数列表

  这条 seam 的意义不是“把 3 个 helper 挂进 struct 上”，而是在 batch runtime
  生产路径继续删掉一条真实的 Python-era owner seam：persisted runtime owner
  明明已经存在，但相邻调用点仍在各自 restitch 同一套 quality /
  story-repair / payload rebuild 输入。现在这一跳被收掉后，Rust 可以更明确地
  回答：
  “从 persisted runtime owner 开始，这几条 runtime payload rebuild 读取的是
  显式 owner contract，而不是各自维护的一份字段列表副本。”

  对 Phase 5 的价值同样直接：这让 `chapter_generation` 主线不只是在 resume
  restore 和 response projection 上收口，也把 batch runtime 紧邻的三个生产态
  persisted-source 消费点继续并回同一条 owner 链。后续做 fallback shrink、
  rollback 边界和 stronger smoke 审计时，也更容易判断 Rust 是否已经在
  runtime payload rebuild 这一层真正进入单一 owner 形态。

  这条 seam 已通过：
  - `cargo test should_restore_batch_runtime_compat_options_from_history_only_quality_runtime_context --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-runtime-owner-followup" -- --nocapture`
  - `cargo test should_build_refreshed_batch_runtime_state_with_existing_active_payload_and_recent_history --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-runtime-owner-followup" -- --nocapture`
  - `cargo test should_build_batch_runtime_state_payload_with_fresh_latest_quality_metrics --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-runtime-owner-followup" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-runtime-owner-followup"`

  - 2026-06-03 同一条 batch create Phase 5 lane 再继续沿 runtime-seed ->
    workflow-launch 边界收口后，create workflow-launch owner 也开始直接消费
    runtime-seed owner，而不再先拆成局部字段再重投影同一边界。当前：
    - `PreparedBatchGenerationCreateWorkflowLaunch::from_runtime_seed(...)`
      现在成为显式 workflow-launch owner 边界，直接接收：
      - `task_spec`
      - normalized target word count
      - chapter targets
      - `user_id`
      - `BatchGenerationCreateRuntimeSeed`
      - prepared execution config
    - `PreparedBatchGenerationCreateWorkflowLaunch::prepare(...)` 现在直接委托给
      这条 runtime-seed owner 边界，而不再本地先 `into_parts()`
    - test helper `build_test_batch_generation_create_workflow_launch(...)`
      也改为复用同一条 runtime-seed owner 边界，不再绕开它做测试专用字段投影

  这条 seam 的意义不是“多加了一个构造函数”，而是在 batch create
  write-lane 的 startup-to-workflow handoff 上继续删掉一条真实的 Python-era
  owner hop。此前 Rust 已经有明确的 `BatchGenerationCreateRuntimeSeed` owner，
  但 workflow-launch 仍会立刻把它拆成 `runtime_state_payload` /
  `resolved_compat_options` 两段，再用这两段重建同一 workflow-launch owner；
  现在这条重复 hop 被移除后，batch create workflow-launch boundary 更接近
  single background / batch resume 已在推进的单一 owner 形态。

  对 Phase 5 的价值同样直接：这让 `chapter_generation` 主线不只是在
  startup runtime-state -> runtime-seed 收口，也把 create runtime-seed ->
  workflow-launch 这一跳继续向 cutover-ready 形态推进。后续关于 startup
  snapshot、fallback shrink、rollback 边界与 stronger smoke 的审计，也更容易
  回答“Rust 到底从哪一步开始只剩 runtime-seed owner，而不再在 create
  workflow-launch 里平行拆包重投影”。

  这条 seam 已通过：
  - `cargo test should_keep_batch_generation_create_workflow_launch_runtime_seed_owner_contract --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-create-owner-contract" -- --nocapture`
  - `cargo test should_keep_batch_generation_create_workflow_launch_owner_contract_explicit --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-create-owner-contract" -- --nocapture`
  - `cargo test should_build_batch_generation_create_workflow_launch_into_persistence_plan --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-create-owner-contract" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-create-owner-contract"`

  - 2026-06-03 同一条 `chapter_generation` Phase 5 runtime lane 再继续沿
    persisted snapshot/runtime-source 边界收口后，batch runtime 的 story-repair
    refresh 与 current-quality snapshot 路径也开始直接消费同一份 persisted
    runtime-context owner，而不再各自重复解析相同的 snapshot/runtime 字段。当前：
    - `backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs`
      新增 `BatchGenerationPersistedRuntimeContext`，显式拥有：
      - `workflow_runtime_state`
      - parsed `BatchGenerationRequestRuntimeState`
      - active story-repair payload
      - persisted `quality_metrics_history`
      - persisted `quality_metrics_summary_state`
      - persisted `quality_metrics_summary`
      - persisted `latest_quality_metrics`
    - `resolve_runtime_compat_options_for_batch_generation_step(...)` 现在先加载
      shared persisted runtime-context owner，再消费它的
      `workflow_runtime_state()` boundary
    - `refresh_batch_generation_runtime_story_repair_state(...)` 不再本地从
      snapshot + runtime JSON 平行恢复：
      - `request_runtime_state`
      - `active_story_repair_payload`
      - quality history/summary-state/summary/latest metrics
    - `build_batch_generation_current_chapter_quality_runtime_snapshot(...)` 也不再
      重复做另一份 persisted runtime-source recovery，而是消费同一条 owner 边界
    - focused owner test
      `should_keep_persisted_batch_runtime_context_owner_contract`
      锁住了这条新的 persisted-source owner contract

  这条 seam 的意义不是“把几段解析逻辑抽成一个 struct”，而是在 batch runtime
  lane 上继续删掉一条真实的 Python-era persisted-source owner hop。此前 Rust
  虽然已经把 snapshot 持久化边界收在 `batch_generation_snapshot::Model` /
  `workflow_runtime_state`，但 refresh 与 current-quality 两条相邻 runtime 路径仍然
  会各自回头把同一批 persisted 字段再解析一遍；现在这条重复 hop 被移除后，
  batch runtime 的 persisted-source recovery boundary 更接近 single-owner 的
  cutover-ready 形态。

  对 Phase 5 的价值同样直接：这让 `chapter_generation` 主线不只是在
  create/resume/write-lane 的 prepare/persistence/response 边界收口，也把 runtime
  lane 的 persisted-source 恢复职责继续向单点 Rust owner 推进。后续关于
  fallback shrink、rollback 边界与 stronger smoke 的审计，也更容易回答
  “Rust 到底由哪一个 owner 负责恢复 batch runtime 的 persisted context，而不再让
  多条 runtime 路径各自重建同一份语义来源”。

  这条 seam 已通过：
  - `cargo test should_keep_persisted_batch_runtime_context_owner_contract --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-runtime-context-owner" -- --nocapture`
  - `cargo test should_build_refreshed_batch_runtime_state_with_existing_active_payload_and_recent_history --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-runtime-context-owner" -- --nocapture`
  - `cargo test should_build_batch_runtime_state_payload_with_fresh_latest_quality_metrics --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-runtime-context-owner" -- --nocapture`
  - `cargo test should_restore_batch_runtime_compat_options_from_history_only_quality_runtime_context --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-runtime-context-owner" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-runtime-context-owner"`

- 2026-06-03 同一条 batch resume Phase 5 lane 再继续沿 persisted runtime-source
  restore boundary 收口后，resume 的 restored runtime-state 恢复路径也开始直接
  消费同一份 shared persisted runtime-context owner，而不再在 resume owner
  内部重复解析 snapshot/runtime quality 字段。当前：
    - `backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs`
      中的 `BatchGenerationPersistedRuntimeContext` 已提升为 crate-internal
      shared owner，并补齐 snapshot-priority:
      - `quality_metrics_history`
      - `quality_metrics_summary`
      - `latest_quality_metrics`
      - runtime `quality_metrics_summary_state`
      - parsed `BatchGenerationRequestRuntimeState`
      - active story-repair payload
    - `backend-rs/src/services/chapter_batch_generation_resume_task_command_service.rs`
      的 `prepare_batch_generation_resume(...)` 现在先 materialize
      `BatchGenerationPersistedRuntimeContext::from_snapshot(snapshot.cloned())`
      再交给 `RestoredResumeRuntimeState::from_persisted_runtime_context(...)`
    - `restored_resume_quality_runtime_context(...)` 的 persisted-source 恢复职责
      也不再手工平行读取：
      - snapshot `latest_quality_metrics`
      - snapshot `quality_metrics_history`
      - snapshot `quality_metrics_summary`
      - runtime `quality_metrics_summary_state`
      - runtime `active_story_repair_payload`
      - runtime `batch_request_runtime_state`
      而是统一通过 shared persisted runtime owner 恢复
    - focused contract test
      `should_restore_resume_runtime_state_from_shared_persisted_runtime_context_owner`
      进一步锁住了“resume 确实消费 shared owner”的新边界

  这条 seam 的意义不是“让 resume 也用同一个 helper”。它真正删掉的是 batch
  resume write-lane 上一条真实的 Python-era persisted-source owner hop。此前
  runtime lane 已经开始把 snapshot/runtime persisted context 收在同一个 Rust
  owner 里，但 resume lane 仍然保留着一套平行恢复逻辑；现在这条重复 hop 被移除
  后，batch resume 的 restored runtime-state boundary 更接近与 batch runtime
  lane 共用同一份 persisted-source owner 的 cutover-ready 形态。

  对 Phase 5 的价值同样直接：这让 `chapter_generation` 主线不只是在
  runtime lane 上回答“谁恢复 persisted runtime context”，也把 batch resume
  prepare lane 的答案收敛到同一个 shared Rust owner。后续关于 fallback
  shrink、rollback 边界与 stronger smoke 的审计，会更容易回答
  “resume 与 runtime 是否已经共享同一份 persisted-source 恢复边界，而不是各自
  再重建一遍语义来源”。

  这条 seam 已通过：
  - `cargo test should_prefer_snapshot_quality_fields_in_persisted_batch_runtime_context_owner --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-resume-persisted-owner" -- --nocapture`
  - `cargo test should_restore_single_resume_seed_from_summary_only_snapshot_quality_context --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-resume-persisted-owner" -- --nocapture`
  - `cargo test should_restore_quality_summary_state_and_history_into_resume_seed --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-resume-persisted-owner" -- --nocapture`
  - `cargo test should_prefer_runtime_active_story_repair_payload_over_quality_context_for_resume_seed --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-resume-persisted-owner" -- --nocapture`
  - `cargo test should_restore_resume_runtime_state_from_shared_persisted_runtime_context_owner --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-resume-persisted-owner" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-resume-persisted-owner"`

- 2026-06-03 同一条 batch runtime Phase 5 lane 又继续收掉了一条 shared owner
  materialize 之后仍退回 raw runtime JSON 的 compat restore seam。当前：
  - `backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs`
    中的 `resolve_runtime_compat_options_for_batch_generation_step(...)` 早已先
    materialize `BatchGenerationPersistedRuntimeContext`
  - 但旧实现仍把恢复动作降级回
    `restore_batch_generation_runtime_compat_options_from_runtime_state(...)`
    并重新从 `workflow_runtime_state` 平行读取：
    - `latest_quality_metrics`
    - `quality_metrics_history`
    - `quality_metrics_summary_state`
    - `quality_metrics_summary`
    - `active_story_repair_payload`
  - 这代表 runtime lane 虽然已经有 shared persisted owner，但 compat restore
    仍保留着一条 Python-era 的 “owner materialize -> drop back to raw JSON ->
    local re-parse” 旧 hop
  - 本轮已改为
    `restore_batch_generation_runtime_compat_options_from_persisted_runtime_context(...)`
    直接消费 shared owner：
    - snapshot-priority `quality_metrics_summary`
    - snapshot-priority `latest_quality_metrics`
    - persisted `quality_metrics_history`
    - runtime `quality_metrics_summary_state`
    - persisted active story-repair payload
  - `workflow_runtime_state()` accessor 也随之删除，说明这条 lane 不再需要从
    shared owner 逃逸回 raw runtime payload 才能完成 compat restore

  这条 seam 的意义不只是 “helper 参数类型更干净”。它真正回答的是：
  batch runtime lane 在 compat restore 这一步是否已经与 shared persisted
  owner 保持同一条恢复边界，而不是 owner 先 materialize、下游再各自把同一份
  persisted 语义从 `workflow_runtime_state` 手工重建一遍。现在这条重复 hop 已经被
  删掉，且 snapshot-priority 也终于完整传导到 runtime compat restore。

  对 Phase 5 的价值同样直接：
  - cutover 审计时更容易回答“哪一个 Rust owner 负责 batch runtime compat
    restore”
  - fallback shrink / rollback / stronger smoke 在 runtime lane 上少了一条
    隐式 raw-JSON 恢复支路
  - shared persisted runtime owner 的 source-priority 语义现在不只影响 refresh /
    resume，也影响 batch runtime compat restore 本身

  这条 seam 已通过：
  - `cargo test should_restore_batch_runtime_compat_options_from_snapshot_payload --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-runtime-compat-owner" -- --nocapture`
  - `cargo test should_fallback_to_base_compat_options_when_runtime_snapshot_missing --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-runtime-compat-owner" -- --nocapture`
  - `cargo test should_restore_batch_runtime_compat_options_from_history_only_quality_runtime_context --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-runtime-compat-owner" -- --nocapture`
  - `cargo test should_restore_batch_runtime_compat_options_from_latest_quality_metrics_when_summary_missing --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-runtime-compat-owner" -- --nocapture`
  - `cargo test should_prefer_snapshot_quality_fields_when_restoring_batch_runtime_compat_options_from_persisted_owner --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-runtime-compat-owner" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-runtime-compat-owner"`

- 2026-06-03 同一条 batch create Phase 5 lane 又继续收掉了一条 startup
  snapshot owner materialize 之后仍退回 raw runtime state 的 create-response
  seam。当前：
  - `backend-rs/src/services/chapter_batch_generation_write_workflow_service.rs`
    里的 `batch_generation_create_response_payload(...)` 之前虽然已经处在
    `BatchGenerationQueuedSnapshotPlan` materialize 之后
  - 但旧实现仍平行回退到 `workflow_runtime_state` 去恢复：
    - `quality_runtime_context`
    - `active_story_repair_payload`
    - `quality_history_context`
  - 具体表现为它继续依赖：
    - `resolve_batch_quality_runtime_context_from_snapshot_and_runtime_state(...)`
    - `active_story_repair_payload_from_runtime_state(...)`
    - 手工 `workflow_runtime_state.get("quality_history_context")`
  - 这代表 create lane 虽然已经有 startup snapshot owner，但 response
    builder 仍保留着一条 Python-era 的
    “owner materialize -> drop back to raw runtime state -> local re-derive”
    旧 hop
  - 本轮已把 response-ready 质量语义收口到
    `BatchGenerationQueuedSnapshotPlan`：
    - 在
      `backend-rs/src/services/chapter_batch_generation_snapshot_service.rs`
      上新增 response-ready owner accessors：
      `quality_runtime_context()`、
      `quality_metrics_summary()`、
      `active_story_repair_payload()`、
      `quality_history_context()`
    - `batch_generation_create_response_payload(...)` 改为直接消费
      `&BatchGenerationQueuedSnapshotPlan`
    - `BatchGenerationCreateLaunchPersistencePlan::from_workflow_launch(...)`
      现在把 `&startup_snapshot_plan` 直接传给 response builder，保持 startup
      owner 边界一直延续到 create response 组装完成

  这条 seam 的意义同样不只是 “response helper 参数更整洁”。它真正回答的是：
  当 batch create lane 已经 materialize 出 startup snapshot owner 之后，
  create-response 是否还要各自从 raw `workflow_runtime_state` 再恢复一遍质量
  runtime context / active story-repair / quality history context。现在这条重复
  hop 已经被删掉，response 侧终于与 persistence 侧共享同一条 owner 恢复边界。

  对 Phase 5 的价值同样直接：
  - cutover 审计时更容易回答“哪一个 Rust owner 负责 batch create response
    的质量语义恢复”
  - fallback shrink / rollback / stronger smoke 在 create response lane 上又少
    了一条隐式 raw-runtime 恢复支路
  - batch create 的 response / persistence 现在更接近共享同一条 startup owner
    graph，而不是 owner 先 materialize、下游再各自平行重建

  这条 seam 已通过：
  - `cargo test should_build_batch_generation_create_response_payload --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-create-response-owner" -- --nocapture`
  - `cargo test should_build_batch_generation_create_launch_persistence_plan_from_create_parts --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-create-response-owner" -- --nocapture`
  - `cargo test should_expose_response_ready_quality_contract_from_batch_generation_queued_snapshot_plan --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-create-response-owner" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-create-response-owner"`

- 2026-06-03 同一条 batch create Phase 5 lane 又继续收掉了一条
  “owner 已存在但 owner 自己仍回退 raw runtime state 才能回答下游语义”的薄壳
  startup seam。当前：
  - 上一条 create-response seam 已经让
    `batch_generation_create_response_payload(...)` 直接消费
    `BatchGenerationQueuedSnapshotPlan`
  - 但 `BatchGenerationQueuedSnapshotPlan` 本身仍只是一个包着
    `runtime_state` 的薄壳 owner，它的 response-ready accessors 还在内部继续：
    - 从 raw `runtime_state` 恢复 `quality_runtime_context`
    - 从 raw `runtime_state` 恢复 `active_story_repair_payload`
    - 从 raw `runtime_state` / 隐式 summary 恢复 `quality_history_context`
  - 这代表 create lane 虽然已经显式 materialize 了 queued startup owner，
    但这个 owner 自己仍保留着一条 Python-era 的
    “owner exists -> accessor falls back to raw runtime -> local re-derive”
    旧 hop
  - 本轮已把这条恢复语义真正前移到 owner materialization 本身：
    - `backend-rs/src/services/chapter_batch_generation_snapshot_service.rs`
      中的 `BatchGenerationQueuedSnapshotPlan` 现在在构造时就 materialize：
      - `BatchGenerationQualityRuntimeContext`
      - startup `active_story_repair_payload`
      - startup `quality_history_context`
    - `quality_runtime_context()`、`active_story_repair_payload()`、
      `quality_history_context()` 现在都直接消费 owner 已 materialize 的字段，
      不再在 accessor 内部回退 raw `runtime_state`
    - `quality_history_context` 额外保留了显式 runtime payload 优先级，避免把
      startup 阶段已经存在的上下文静默降级掉

  这条 seam 的意义同样不只是 “owner 多存了几个字段”。它真正回答的是：
  queued startup owner 在 Phase 5 语义上到底是不是一个真实 Rust owner，
  还是只是一个外壳对象，真正的质量语义仍在每次 accessor 调用时从 raw
  runtime state 临时恢复。现在这条隐式 fallback 已经被删掉，queued startup
  owner 本身终于成为 response-ready 的 materialized 边界。

  对 Phase 5 的价值同样直接：
  - cutover 审计时更容易回答“batch create queued startup owner 到底拥有哪一层
    质量语义，而不是只拥有一份原始 runtime JSON”
  - create response / create persistence 现在不仅共享同一个 owner 名义边界，
    也共享同一份 owner 内部已 materialize 的 startup 语义
  - fallback shrink / rollback / stronger smoke 在 batch create startup lane
    上又少了一条隐藏在 owner accessor 里的 raw-runtime 恢复支路

  这条 seam 已通过：
  - `cargo test should_expose_response_ready_quality_contract_from_batch_generation_queued_snapshot_plan --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-queued-startup-owner" -- --nocapture`
  - `cargo test should_build_batch_generation_create_response_payload --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-queued-startup-owner" -- --nocapture`
  - `cargo test should_build_batch_generation_create_launch_persistence_plan_from_create_parts --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-queued-startup-owner" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-queued-startup-owner"`

- 2026-06-03 同一条 `chapter_generation` Phase 5 lane 又继续回到 single background
  startup -> response 邻域，收掉了一条 startup owner 已存在但 response helper 仍
  平行回退 raw runtime state 的 seam。当前：
  - `SingleGenerationBackgroundLaunchPersistencePlan::from_restored_launch(...)`
    已经先 materialize `SingleGenerationStartupSnapshotPlan`
  - 但旧实现里的
    `single_generation_background_create_response_payload(...)` 仍直接吃 raw
    `workflow_runtime_state`，并本地重建：
    - `GenerationQualityRuntimeContext`
    - `active_story_repair_payload`
    - `quality_history_context`
  - 这代表 single background lane 虽然已经显式拥有 startup snapshot owner，
    但 response builder 仍保留着一条 Python-era 的
    “owner materialize -> drop back to raw runtime state -> local re-derive”
    旧 hop
  - 本轮已把 response-ready 语义收口到
    `SingleGenerationStartupSnapshotPlan`：
    - 在
      `backend-rs/src/services/chapter_batch_generation_snapshot_service.rs`
      上新增 single response-ready owner accessors：
      `quality_runtime_context()`、
      `latest_quality_metrics()`、
      `quality_metrics_history()`、
      `quality_metrics_summary_state()`、
      `quality_metrics_summary()`、
      `active_story_repair_payload()`、
      `quality_history_context()`
    - `single_generation_background_create_response_payload(...)` 改为直接消费
      `&SingleGenerationStartupSnapshotPlan`
    - `SingleGenerationBackgroundLaunchPersistencePlan::from_restored_launch(...)`
      现在把 `&startup_snapshot_plan` 直接传给 response builder，保持 startup
      owner 边界从 snapshot materialize 一直延续到 response 组装完成

  这条 seam 的意义同样不只是 “single helper 参数更规整”。它真正回答的是：
  当 single background lane 已经 materialize 出 startup owner 之后，
  create-response 是否还要各自从 raw `workflow_runtime_state` 再恢复一遍质量
  runtime context / active story-repair / quality history context。现在这条重复
  hop 已经被删掉，single background response 终于和 startup owner 保持同一条
  Rust 恢复边界。

  对 Phase 5 的价值同样直接：
  - cutover 审计时更容易回答“哪一个 Rust owner 负责 single background
    create-response 的质量语义恢复”
  - single background lane 与 batch create / batch resume 更接近共享同一套
    startup-owner narrowing 规则，而不是 owner 已存在、response 侧仍各自平行恢复
  - fallback shrink / rollback / stronger smoke 在 single startup lane 上又少了
    一条隐式 raw-runtime 恢复支路

  这条 seam 已通过：
  - `cargo test should_expose_response_ready_quality_contract_from_single_generation_startup_snapshot_plan --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-single-startup-response-owner" -- --nocapture`
  - `cargo test should_preserve_richer_quality_runtime_contract_on_single_generation_background_create_payload_safe --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-single-startup-response-owner" -- --nocapture`
  - `cargo test should_build_single_generation_background_launch_persistence_plan_from_prepared_owner --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-single-startup-response-owner" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-single-startup-response-owner"`

- 2026-06-03 同一条 `chapter_generation` Phase 5 lane 又继续收掉了一条
  “owner 已存在但 owner 自己仍回退 raw runtime state 才能回答下游语义”的 single
  startup 薄壳 seam。当前：
  - 上一条 single create-response seam 已经让
    `single_generation_background_create_response_payload(...)` 直接消费
    `SingleGenerationStartupSnapshotPlan`
  - 但 `SingleGenerationStartupSnapshotPlan` 本身仍只是一个包着
    `runtime_state` 的薄壳 owner，它的 response-ready accessors 还在内部继续：
    - 从 raw `runtime_state` 恢复 `GenerationQualityRuntimeContext`
    - 从 raw `runtime_state` 读取
      `latest_quality_metrics` / `quality_metrics_history` /
      `quality_metrics_summary_state` / `quality_metrics_summary`
    - 从 raw `runtime_state` 恢复 `active_story_repair_payload` 与
      `quality_history_context`
  - 这代表 single startup lane 虽然已经显式 materialize 了 startup owner，
    但 owner 自己仍保留着一条 Python-era 的
    “owner exists -> accessor falls back to raw runtime -> local re-derive”
    旧 hop
  - 本轮已把这条恢复语义真正前移到 owner materialization 本身：
    - `backend-rs/src/services/chapter_batch_generation_snapshot_service.rs`
      中的 `SingleGenerationStartupSnapshotPlan` 现在在构造时就 materialize：
      - `GenerationQualityRuntimeContext`
      - startup `active_story_repair_payload`
      - startup `quality_history_context`
    - `quality_runtime_context()`、`latest_quality_metrics()`、
      `quality_metrics_history()`、`quality_metrics_summary_state()`、
      `quality_metrics_summary()`、`active_story_repair_payload()`、
      `quality_history_context()` 现在都直接消费 owner 已 materialize 的字段，
      不再在 accessor 内部回退 raw `runtime_state`
    - `quality_history_context` 额外保留了显式 runtime payload 优先级，避免把
      startup 阶段已经存在的上下文静默降级掉

  这条 seam 的意义同样不只是 “single owner 多存了几个字段”。它真正回答的是：
  single startup owner 在 Phase 5 语义上到底是不是一个真实 Rust owner，
  还是只是一个外壳对象，真正的质量语义仍在每次 accessor 调用时从 raw
  runtime state 临时恢复。现在这条隐式 fallback 已经被删掉，single startup
  owner 本身终于成为 response-ready 的 materialized 边界。

  对 Phase 5 的价值同样直接：
  - cutover 审计时更容易回答“single startup owner 到底拥有哪一层质量语义，
    而不是只拥有一份原始 runtime JSON”
  - single background response / persistence 现在不仅共享同一个 owner 名义边界，
    也共享同一份 owner 内部已 materialize 的 startup 语义
  - fallback shrink / rollback / stronger smoke 在 single startup lane 上又少了
    一条隐藏在 owner accessor 里的 raw-runtime 恢复支路

  这条 seam 已通过：
  - `cargo test should_expose_response_ready_quality_contract_from_single_generation_startup_snapshot_plan --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-single-startup-materialized-owner" -- --nocapture`
  - `cargo test should_preserve_richer_quality_runtime_contract_on_single_generation_background_create_payload_safe --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-single-startup-materialized-owner" -- --nocapture`
  - `cargo test should_build_single_generation_background_launch_persistence_plan_from_prepared_owner --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-single-startup-materialized-owner" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-single-startup-materialized-owner"`

- 2026-06-03 同一条 `chapter_generation` Phase 5 lane 又继续收掉了一条
  “restored launch owner 已存在，但 write-workflow 仍从 raw launch parts 本地重建
  startup lifecycle 语义”的 single background seam。当前：
  - `PreparedSingleChapterGenerationRestoredRuntimeLaunch` 之前已经 materialize：
    - `chapter_target`
    - restored single runtime-state payload
    - resolved runtime launch input
  - 但 `chapter_single_generation_write_workflow_service.rs` 的
    `SingleGenerationBackgroundLaunchPersistencePlan::from_restored_launch(...)`
    仍先 `into_parts()`，然后再本地：
    - 基于 `chapter_target.pending_checkpoint()` 与 raw runtime payload 重建
      `SingleGenerationStartupSnapshotPlan`
    - 再把这个本地重建出来的 startup owner 用于 persistence / response
      assembly
  - 这代表 single background lane 仍保留一条 Python-era 的旧 hop：
    owner 已构造 -> caller 继续从 raw lifecycle parts 重建同一 startup 语义
  - 本轮已把这条生命周期语义真正前移到 restored launch owner 本身：
    - `backend-rs/src/services/chapter_single_generation_prepare_service.rs`
      中的 `PreparedSingleChapterGenerationRestoredRuntimeLaunch` 现在构造时就
      materialize `SingleGenerationStartupSnapshotPlan`
    - `into_parts()` 现在直接下发：
      - `chapter_target`
      - materialized `SingleGenerationStartupSnapshotPlan`
      - resolved runtime launch input
    - `chapter_single_generation_write_workflow_service.rs` 中的
      `SingleGenerationBackgroundLaunchPersistencePlan::from_restored_launch(...)`
      现在直接消费这个 startup owner，不再本地用 raw runtime payload
      重建一遍 startup snapshot
    - 同时补了 focused owner test，证明 restored launch owner 本身已经持有
      startup-snapshot 语义，而不是只持有一份待下游重组的 raw payload

  这条 seam 的意义同样不只是 “owner 多带一个字段”。它真正回答的是：
  single restored launch owner 在 Phase 5 语义上到底是不是 single background
  lane 的真实 startup-lifecycle 边界，还是只是一个中转壳，真正的 startup
  snapshot 仍要在 write-workflow 侧再 materialize 一次。现在这条隐式重组 hop
  已经被删掉，restored launch owner 本身终于成为 startup snapshot 的显式
  Rust materialization 边界。

  对 Phase 5 的价值同样直接：
  - cutover 审计时更容易回答“single background launch 到底从哪个 Rust owner
    接手 startup snapshot 语义”
  - single startup / background persistence / response projection 现在共享更连续的
    owner 链，而不是 owner 已存在、write-workflow 仍保留一条 raw-launch
    rebuild 支路
  - fallback shrink / rollback / stronger smoke 在 single startup lane 上又少了
    一条隐藏在 write-workflow 内部的 lifecycle 重组支路

  这条 seam 已通过：
  - `cargo test should_build_single_generation_background_launch_persistence_plan_from_prepared_owner --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-single-startup-launch-owner" -- --nocapture`
  - `cargo test should_keep_single_generation_background_persistence_plan_restored_launch_owner_contract --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-single-startup-launch-owner" -- --nocapture`
  - `cargo test should_convert_restored_single_generation_launch_into_runtime_input --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-single-startup-launch-owner" -- --nocapture`
  - `cargo test should_materialize_single_generation_startup_snapshot_inside_restored_launch_owner --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-single-startup-launch-owner" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-single-startup-launch-owner"`

- 2026-06-03 同一条 `chapter_generation` Phase 5 lane 又继续收掉了一条
  “batch create runtime-seed owner 已存在，但 workflow launch 仍从 raw runtime
  payload 本地重建 queued startup lifecycle 语义”的 batch create seam。当前：
  - `BatchGenerationCreateRuntimeSeed` 之前已经 materialize：
    - startup runtime-state payload
    - resolved compat options
  - 但 `PreparedBatchGenerationCreateWorkflowLaunch::from_runtime_seed(...)`
    仍先 `into_parts()`，然后再本地：
    - 基于 raw runtime-state payload 重建
      `BatchGenerationQueuedSnapshotPlan`
    - 再把这个本地重建出来的 queued startup owner 用于 workflow-launch
      persistence / response assembly
  - 这代表 batch create lane 仍保留一条 Python-era 的旧 hop：
    owner 已构造 -> caller 继续从 raw runtime payload 重建同一 queued startup
    语义
  - 本轮已把这条生命周期语义真正前移到 runtime-seed owner 本身：
    - `backend-rs/src/services/chapter_batch_generation_write_workflow_service.rs`
      中的 `BatchGenerationCreateRuntimeSeed` 现在提供
      `into_workflow_runtime_parts(total_chapters)`，可直接投影：
      - materialized `BatchGenerationQueuedSnapshotPlan`
      - resolved compat options
    - `PreparedBatchGenerationCreateWorkflowLaunch::from_runtime_seed(...)`
      现在直接消费这个 owner projection，不再本地用 raw runtime payload
      重建一遍 queued startup snapshot
    - 同时补了 focused owner test，证明 runtime-seed owner 本身已经持有
      queued-startup 语义，而不是只持有一份待下游重组的 raw payload

  这条 seam 的意义同样不只是 “runtime seed 多带一个 helper”。它真正回答的是：
  batch create runtime-seed owner 在 Phase 5 语义上到底是不是 batch create lane
  的真实 queued-startup 边界，还是只是一个中转壳，真正的 queued snapshot
  仍要在 workflow-launch 侧再 materialize 一次。现在这条隐式重组 hop
  已经被删掉，runtime-seed owner 本身终于成为 queued startup 的显式
  Rust materialization 边界。

  对 Phase 5 的价值同样直接：
  - cutover 审计时更容易回答“batch create workflow launch 到底从哪个 Rust owner
    接手 queued snapshot 语义”
  - batch create startup / workflow launch / persistence / response projection
    现在共享更连续的 owner 链，而不是 owner 已存在、workflow-launch
    仍保留一条 raw-runtime rebuild 支路
  - fallback shrink / rollback / stronger smoke 在 batch create lane 上又少了
    一条隐藏在 workflow-launch 内部的 lifecycle 重组支路

  这条 seam 已通过：
  - `cargo test should_keep_batch_generation_create_runtime_seed_contract --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-create-runtime-seed-owner" -- --nocapture`
  - `cargo test should_keep_batch_generation_create_workflow_launch_runtime_seed_owner_contract --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-create-runtime-seed-owner" -- --nocapture`
  - `cargo test should_materialize_batch_generation_queued_snapshot_inside_runtime_seed_owner --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-create-runtime-seed-owner" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-create-runtime-seed-owner"`

- 2026-06-04 同一条 `chapter_generation` Phase 5 lane 又继续收掉了一条
  “restored launch owner 已存在，但 single background write-workflow 仍在本地
  重组 persistence-plan / response handoff 语义”的 seam。当前：
  - `PreparedSingleChapterGenerationRestoredRuntimeLaunch` 在上一轮已经
    materialize：
    - `chapter_target`
    - `SingleGenerationStartupSnapshotPlan`
    - resolved `SingleGenerationRuntimeLaunchInput`
  - 但
    `chapter_single_generation_write_workflow_service.rs` 里的
    `PreparedSingleGenerationBackgroundLaunch::prepare_from_target(...)`
    仍然把这个 owner 交给
    `SingleGenerationBackgroundLaunchPersistencePlan::from_restored_launch(...)`
    再本地重组：
    - 先 `into_parts()`
    - 再本地计算 `estimated_minutes`
    - 再本地重建 single background create-response payload
    - 再本地装回最终 `SingleGenerationBackgroundLaunchPersistencePlan`
  - 这代表 single background write lane 仍保留一条 Python-era 的旧 hop：
    owner 已构造 -> caller 继续从同一批 restored lifecycle parts 本地重组
    persistence-plan 语义
  - 本轮已把这条生命周期语义真正前移到 restored launch owner 本身：
    - `backend-rs/src/services/chapter_single_generation_write_workflow_service.rs`
      中新增
      `PreparedSingleChapterGenerationRestoredRuntimeLaunch::into_background_launch_persistence_plan(...)`
    - restored launch owner 现在可直接投影：
      - final `SingleGenerationBackgroundLaunchPersistencePlan`
      - derived background response payload
      - persistence-ready startup/runtime handoff contract
    - `PreparedSingleGenerationBackgroundLaunch::prepare_from_target(...)`
      现在直接消费这个 owner projection，不再保留 caller 侧的 local
      persistence-plan restitch
    - 同时补了 focused owner test，证明 restored launch owner 本身已经能
      materialize single background persistence plan，而不是只交付一组待下游
      重组的 lifecycle parts

  这条 seam 的意义同样不只是 “owner 多带一个 helper”。它真正回答的是：
  single restored launch owner 在 Phase 5 语义上到底是不是 single background
  write lane 的真实 persistence handoff 边界，还是只是一个中转壳，真正的
  background response / persistence-plan 仍要在 write-workflow 侧再拼一次。
  现在这条隐式重组 hop 已经被删掉，restored launch owner 本身终于成为
  single background persistence handoff 的显式 Rust materialization 边界。

  对 Phase 5 的价值同样直接：
  - cutover 审计时更容易回答“single background launch 到底从哪个 Rust owner
    接手 persistence-plan 语义”
  - single restore / startup snapshot / background persistence / response
    projection 现在共享更连续的 owner 链，而不是 owner 已存在、
    write-workflow 仍保留一条 local plan rebuild 支路
  - fallback shrink / rollback / stronger smoke 在 single background lane 上
    又少了一条隐藏在 write-workflow 内部的 lifecycle 重组支路

  这条 seam 已通过：
  - `cargo test should_project_single_generation_background_persistence_plan_from_restored_launch_owner --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-single-background-restored-launch-owner" -- --nocapture`
  - `cargo test should_keep_single_generation_background_persistence_plan_restored_launch_owner_contract --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-single-background-restored-launch-owner" -- --nocapture`
  - `cargo test should_build_single_generation_background_launch_persistence_plan_from_prepared_owner --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-single-background-restored-launch-owner" -- --nocapture`
  - `cargo test should_preserve_richer_quality_runtime_contract_on_single_generation_background_create_payload_safe --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-single-background-restored-launch-owner" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-single-background-restored-launch-owner"`

- 2026-06-04 同一条 `chapter_generation` Phase 5 lane 又继续收掉了一条
  “shared read-context owner 已存在，但 single existing-background caller 仍在
  本地重组 compat payload 语义”的 read-side seam。当前：
  - `BatchGenerationReadContext` 之前已经 materialize：
    - `task`
    - `workflow_runtime_state`
    - `quality_status_context`
  - 但
    `chapter_single_generation_write_workflow_service.rs` 里的
    `single_generation_existing_background_task_payload(...)`
    仍在这个 owner 之后本地重组：
    - shared task view payload
    - single-specific `task_id/chapter_id/status/message`
    - `estimated_time_minutes`
  - 这代表 single existing-background lane 仍保留一条 Python-era 的旧 hop：
    owner 已构造 -> caller 继续从同一份 read-context 本地恢复 compat payload
    语义
  - 本轮已把这条 read-side 语义真正前移到 shared read-context owner 本身：
    - `backend-rs/src/services/chapter_batch_generation_read_context_service.rs`
      中新增
      `BatchGenerationReadContext::into_single_generation_existing_background_task_payload(...)`
    - shared read-context owner 现在可直接投影：
      - final single existing-background compat payload
      - merged task/runtime/quality read-side contract
    - `chapter_single_generation_write_workflow_service.rs` 中的
      `single_generation_existing_background_task_payload(...)`
      现在直接消费这个 owner projection，不再本地重建 payload 字段
    - 同时补了 focused owner test，证明 shared read-context owner 本身已经能
      materialize single existing-background payload，而不是只交付一组待下游
      重组的 read-side fields

  这条 seam 的意义同样不只是 “read-context 多带一个 helper”。它真正回答的是：
  shared read-context owner 在 Phase 5 语义上到底是不是 single
  existing-background lane 的真实 compat payload 边界，还是只是一个中转壳，
  真正的 single-specific payload 仍要在 caller 侧再拼一次。现在这条隐式重组
  hop 已经被删掉，shared read-context owner 本身终于成为 single
  existing-background payload 的显式 Rust materialization 边界。

  对 Phase 5 的价值同样直接：
  - cutover 审计时更容易回答“single existing-background payload 到底从哪个
    Rust owner 接手 read-side 语义”
  - batch read-context / single existing-background reuse 现在共享更连续的
    owner 链，而不是 owner 已存在、caller 仍保留一条 local payload rebuild
    支路
  - fallback shrink / rollback / stronger smoke 在 single existing-task lane 上
    又少了一条隐藏在 write-workflow 内部的 compat 重组支路

  这条 seam 已通过：
  - `cargo test should_build_single_generation_existing_background_task_payload_from_read_context --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-single-existing-background-read-context-owner" -- --nocapture`
  - `cargo test should_preserve_richer_quality_runtime_contract_on_existing_single_generation_background_payload --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-single-existing-background-read-context-owner" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-single-existing-background-read-context-owner"`

- 2026-06-03 同一条 `chapter_generation` Phase 5 lane 又继续收掉了一条
  “single restore seed owner 已存在，但 prepare 侧仍从 raw payload 本地重组
  startup snapshot / runtime launch 语义”的 single startup-to-runtime seam。
  当前：
  - `RestoredSingleGenerationRuntimeState` 之前已经 materialize：
    - restored single runtime seed payload
    - explicit seed-source semantics
  - 但
    `PreparedSingleChapterGenerationRestoredRuntimeLaunch::prepare_from_prepared_execution(...)`
    仍在这个 owner 之后本地继续做两段重组：
    - 先取出 raw runtime payload
    - 再用 `chapter_target.pending_checkpoint()` + raw payload 重建
      `SingleGenerationStartupSnapshotPlan`
    - 再用同一份 raw payload 重建最终 runtime launch input
  - 这代表 single startup-to-runtime lane 仍保留一条 Python-era 的旧 hop：
    owner 已构造 -> caller 继续从 raw payload 重建同一 startup/runtime 语义
  - 本轮已把这条生命周期语义真正前移到 restore seed owner 本身：
    - `backend-rs/src/services/chapter_single_generation_prepare_service.rs`
      中的 `RestoredSingleGenerationRuntimeState` 现在直接持有
      `SingleGenerationStartupSnapshotPlan`
    - 新增
      `into_startup_runtime_launch_parts(...)`，可直接投影：
      - materialized `SingleGenerationStartupSnapshotPlan`
      - resolved `SingleGenerationRuntimeLaunchInput`
    - `restore_single_generation_runtime_state(...)` 现在在 owner 构造时就把
      `chapter_target.pending_checkpoint()` 一并前移进去
    - `PreparedSingleChapterGenerationRestoredRuntimeLaunch::prepare_from_prepared_execution(...)`
      现在直接消费这个 owner projection，不再本地回退到 raw payload 去重建
      startup/runtime 边界

  这条 seam 的意义同样不只是 “restore seed owner 多带一个 startup 字段”。
  它真正回答的是：single restore seed owner 在 Phase 5 语义上到底是不是
  single lane 的真实 startup/runtime 边界，还是只是一个中转壳，真正的
  startup snapshot 和 runtime launch 仍要在 prepare 侧再组装一次。现在这条
  隐式重组 hop 已经被删掉，restore seed owner 本身终于成为 single
  startup/runtime 的显式 Rust materialization 边界。

  对 Phase 5 的价值同样直接：
  - cutover 审计时更容易回答“single startup/runtime 到底从哪个 Rust owner
    接手 restore seed 语义”
  - single restore / startup snapshot / runtime launch 现在共享更连续的 owner
    链，而不是 owner 已存在、prepare 侧仍保留一条 raw-payload rebuild 支路
  - fallback shrink / rollback / stronger smoke 在 single lane 上又少了一条
    隐藏在 prepare owner 内部的 startup/runtime 重组支路

  这条 seam 已通过：
  - `cargo test should_project_restored_single_generation_runtime_state_into_startup_and_runtime_launch_owner --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-single-restore-seed-owner" -- --nocapture`
  - `cargo test should_build_single_generation_background_launch_persistence_plan_from_prepared_owner --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-single-restore-seed-owner" -- --nocapture`
  - `cargo test should_build_single_generation_runtime_launch_input_from_restored_seed --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-single-restore-seed-owner" -- --nocapture`
  - `cargo test should_convert_restored_single_generation_launch_into_runtime_input --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-single-restore-seed-owner" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-single-restore-seed-owner"`

- 2026-06-06 同一条 `chapter_generation` Phase 5 lane 又继续收掉了一条
  “single restored-launch / background-write owner 已经显式持有 startup
  snapshot planning inputs，但 chapter-scoped startup snapshot owner 仍挂在
  batch snapshot 文件里”的 single startup snapshot owner seam。当前：
  - single lane 之前已经显式 materialize：
    - restored runtime-state projection
    - pending-checkpoint + runtime-seed startup planning inputs
    - background response payload
    - runtime launch input
  - 但
    `chapter_batch_generation_snapshot_service.rs`
    之前仍保留一条 chapter-only 的 owner：
    - `SingleGenerationStartupSnapshotPlan`
  - 这代表 single startup snapshot lane 在 Phase 5 上仍保留一条
    Python-era 旧 hop：single owner 已存在，但 startup snapshot contract 仍寄居在
    batch snapshot 邻域，cutover 审计时会继续把一个 chapter-only owner 误判为
    batch shared boundary
  - 本轮已把这条 chapter-scoped startup snapshot contract 真正前移回
    single-generation 模块本身：
    - 新增
      `backend-rs/src/services/chapter_single_generation_snapshot_service.rs`
    - 该文件现在直接拥有：
      - `SingleGenerationStartupSnapshotPlan`
      - `from_pending_checkpoint(...)`
      - `runtime_state()`
      - `quality_runtime_context()`
      - `latest_quality_metrics()`
      - `quality_metrics_history()`
      - `quality_metrics_summary_state()`
      - `quality_metrics_summary()`
      - `active_story_repair_payload()`
      - `quality_history_context()`
      - `persist(...)`
    - `chapter_single_generation_prepare_service.rs`
      和
      `chapter_single_generation_write_workflow_service.rs`
      现在都直接消费这个 local owner
    - `chapter_batch_generation_snapshot_service.rs`
      则回退成只保留 batch shared 的 queued / resume / persistence 边界

  这条 seam 的意义不只是“把 struct 挪了个文件”。它真正回答的是：
  single restore / startup snapshot / write-runtime lane 一旦已经被 Rust owner
  接住，chapter-scoped startup snapshot contract 到底是不是继续属于 single
  lane，还是还要在 batch snapshot 邻域上挂一个看似 shared 的旧 owner。现在这条
  chapter-only owner 已被拉回 single 模块，single startup snapshot 的真实 Rust
  边界更容易被审计和继续 cutover。

  对 Phase 5 的价值同样直接：
  - cutover 审计时更容易回答“single startup snapshot 到底由哪个 Rust owner
    接手”
  - single restore / startup snapshot / background write / runtime dispatch
    现在共享更连续的 owner 链，而不是 single owner 已存在、snapshot owner
    仍挂在 batch 邻域
  - fallback shrink / rollback / stronger smoke 在 single lane 上又少了一条
    隐藏在 batch snapshot 文件里的 chapter-only compat 支路

  这条 seam 已通过：
  - `cargo test chapter_single_generation_snapshot_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-startup-snapshot-owner-collapse" -- --nocapture`
  - `cargo test chapter_single_generation_prepare_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-startup-snapshot-owner-collapse" -- --nocapture`
  - `cargo test chapter_single_generation_write_workflow_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-startup-snapshot-owner-collapse" -- --nocapture`
  - `cargo test chapter_single_generation --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-startup-snapshot-owner-collapse" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-startup-snapshot-owner-collapse"`

- 2026-06-06 同一条 `chapter_generation` Phase 5 lane 又继续收掉了一条
  “single background write-workflow 已经只剩 existing-payload 分支选择与
  launch 分支，但 existing-background 的 active task 查询 / recovered
  read-state / compat payload projection 仍整块挂在 write-workflow 文件里”的
  single existing-background query file seam。当前：
  - single background write lane 之前已经显式拥有：
    - existing-task short-circuit branch decision
    - restored background launch preparation
    - persist-and-dispatch workflow entry
  - 但
    `chapter_single_generation_write_workflow_service.rs`
    之前仍整块保留：
    - active single-generation background task query
    - recovered snapshot/read-state loading
    - existing-background compat payload projection
  - 这代表 single existing-background lane 在 Phase 5 上仍保留一条
    Python-era 的旧 hop：write-workflow owner 已经基本明确，但 query/load/
    payload 这整条相邻 read-side owner 仍和 workflow body 混在同一个文件里，
    cutover 审计时会继续把 mixed file 误判成不可拆分的单个 owner
  - 本轮已把这条 existing-background query/load/projection contract 真正前移成
    dedicated single-generation owner file：
    - 新增
      `backend-rs/src/services/chapter_single_generation_existing_background_query_service.rs`
    - 该文件现在直接拥有：
      - `SingleGenerationExistingBackgroundTaskContext`
      - `load_owned_single_generation_existing_background_task_payload(...)`
      - `into_single_generation_existing_background_task_payload(...)`
    - `chapter_single_generation_write_workflow_service.rs`
      现在只消费这个 focused query owner，保留：
      - existing payload vs prepared launch 分支选择
      - workflow launch / persist-and-dispatch
    - existing-background 的 owner 级测试也一起迁到了新文件，不再继续绑在
      write-workflow 测试模块里

  这条 seam 的意义不只是“又拆了一个 service 文件”。它真正回答的是：
  single existing-background branch 一旦已经由 Rust write owner 接住，
  existing-background query/load/payload 这整条相邻 read-side contract 到底
  是不是还应该继续和 workflow body 混在一起。现在这条 mixed owner 文件
  已被收窄，single existing-background query lane 更接近真实、可审计的
  Rust owner boundary。

  对 Phase 5 的价值同样直接：
  - cutover 审计时更容易回答“single existing-background query/load/payload
    到底由哪个 Rust owner 接手”
  - single existing-background query / payload projection / workflow branch
    现在共享更连续但更清晰的 owner 链，而不是 query/read-state 与 workflow
    body 混在一个文件里
  - fallback shrink / rollback / stronger smoke 在 single background lane 上
    又少了一条隐藏在 write-workflow 文件内部的 mixed read/write compat 支路

  这条 seam 已通过：
  - `cargo test chapter_single_generation_existing_background_query_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-existing-background-query-file-collapse" -- --nocapture`
  - `cargo test chapter_single_generation_write_workflow_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-existing-background-query-file-collapse" -- --nocapture`
  - `cargo test chapter_single_generation --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-existing-background-query-file-collapse" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-existing-background-query-file-collapse"`

- 2026-06-06 同一条 `chapter_generation` Phase 5 lane 又继续收掉了一条
  “single startup snapshot / runtime lane 已经是 chapter-scoped owner，
  但 runtime-state merge 与 snapshot upsert 仍直接重开 batch snapshot helper”的
  single snapshot persistence / merge seam。当前：
  - single-generation startup snapshot / runtime 邻域之前已经显式拥有：
    - chapter-scoped startup snapshot planning
    - single runtime checkpoint stage projection
    - single runtime lifecycle persistence branches
  - 但
    `chapter_single_generation_snapshot_service.rs`
    和
    `chapter_single_generation_runtime_state_service.rs`
    之前仍直接重开：
    - `project_merged_batch_generation_runtime_state(...)`
    - `upsert_batch_generation_runtime_snapshot(...)`
  - 这代表 single snapshot persistence lane 在 Phase 5 上仍保留一条
    Python-era 的旧 hop：single owner 已存在，但 chapter-scoped merge / persist
    contract 仍寄居在 batch snapshot helper 邻域，cutover 审计时会继续把
    一个 chapter-only persistence boundary误判为 batch shared helper 依赖
  - 本轮已把这条 chapter-scoped snapshot merge / persist contract 真正前移回
    single-generation 模块本身：
    - `backend-rs/src/services/chapter_single_generation_snapshot_service.rs`
      现在直接拥有：
      - `merge_single_generation_runtime_state(...)`
      - `upsert_single_generation_runtime_snapshot(...)`
      - `SingleGenerationStartupSnapshotPlan::persist(...)`
    - `chapter_single_generation_runtime_state_service.rs`
      现在直接消费这条 single snapshot owner，不再重开 batch snapshot helper
    - batch snapshot 文件则继续只暴露底层 shared persistence 实现，不再作为
      single runtime lane 的直接生产 owner 入口

  这条 seam 的意义不只是“换了调用入口”。它真正回答的是：
  single-generation 的 startup snapshot / runtime checkpoint 一旦已经被 Rust
  owner 接住，chapter-scoped merge 和 snapshot upsert 到底是不是还要回到
  batch snapshot 邻域再走一遍。现在这条 duplicate 已被删掉，single snapshot
  persistence lane 更接近真实、可审计的 Rust owner boundary。

  对 Phase 5 的价值同样直接：
  - cutover 审计时更容易回答“single snapshot merge / persist 到底由哪个 Rust owner 接手”
  - single startup snapshot / runtime checkpoint / terminal persistence
    现在共享更连续的 owner 链
  - fallback shrink / rollback / stronger smoke 在 single-generation 邻域上
    又少了一条隐藏在 batch snapshot helper 里的 direct reopen 支路

  这条 seam 已通过：
  - `cargo test chapter_single_generation_snapshot_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-snapshot-persistence-owner-collapse" -- --nocapture`
  - `cargo test chapter_single_generation_runtime_state_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-snapshot-persistence-owner-collapse" -- --nocapture`
  - `cargo test chapter_single_generation --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-snapshot-persistence-owner-collapse" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-snapshot-persistence-owner-collapse"`

- 2026-06-06 同一条 `chapter_generation` Phase 5 lane 随后又继续收掉了一条
  “single runtime snapshot write lane 虽然已经不再直接挂 batch 的 query /
  recovery helper，但底层 snapshot merge / replace persistence 仍寄居在
  batch snapshot 文件名下”的 shared snapshot persistence seam。当前：
  - shared snapshot-query / task-recovery owner 已经完成后，
    single-generation 生产链不再直接依赖 batch-named 的 lower-level
    query / recovery 入口
  - 但
    `chapter_single_generation_snapshot_service.rs`
    之前仍通过：
    - `chapter_batch_generation_snapshot_service::upsert_batch_generation_runtime_snapshot(...)`
    回到 batch snapshot 邻域执行底层 snapshot merge / persist
  - 这代表 single snapshot write lane 在 Phase 5 上仍保留一条旧 hop：
    chapter-scoped startup / runtime owner 已存在，但真正 shared 的
    `task id + runtime state -> snapshot persistence` lower-level contract
    仍寄居在 batch 文件名下，cutover 审计时会继续把这条 shared write seam
    误判成 single 对 batch 的直接依赖
  - 本轮已把这条真正 shared 的 lower-level write owner 前移回
    chapter-generation 邻域本身：
    - `backend-rs/src/services/chapter_generation_snapshot_persistence_service.rs`
      现在直接拥有：
      - `ChapterGenerationSnapshotWriteMode`
      - `merge_chapter_generation_runtime_state(...)`
      - `persist_chapter_generation_runtime_snapshot(...)`
      - `upsert_chapter_generation_runtime_snapshot(...)`
      - snapshot persistence 期间的 quality-column sync / backfill helper
    - `chapter_batch_generation_snapshot_service.rs`
      现在保留 batch-local queued/resume snapshot plan 与 batch public API，
      但底层 merge / replace persistence 已下沉到 shared owner
    - `chapter_single_generation_snapshot_service.rs`
      现在直接消费这条 shared persistence owner，
      不再通过 batch snapshot 文件名回跳

  这条 seam 的意义不只是“多了一个 shared service 文件”。它真正回答的是：
  一旦 shared lower-level read/recover owner 已经被抬出 batch 邻域，
  snapshot write 这条同样 shared 的 lower-level production contract
  到底是不是还应该继续寄居在 batch snapshot 文件名下。现在这条 write seam
  也已显式 lifted，single/batch 邻域更接近真实、可审计的 Rust owner boundary。

  对 Phase 5 的价值同样直接：
  - cutover 审计时更容易回答“shared snapshot merge / persist 到底由哪个 Rust owner 接手”
  - single-generation 的 runtime snapshot write lane 不再因为文件命名关系
    被误判为 batch 模块依赖
  - fallback shrink / rollback / stronger smoke 在 chapter-generation
    shared seam 上又少了一条隐藏的 batch-named write hop

  这条 seam 已通过：
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-shared-snapshot-persistence-owner"`
  - `cargo test chapter_generation_snapshot_persistence_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-shared-snapshot-persistence-owner" -- --nocapture`
  - `cargo test chapter_batch_generation_snapshot_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-shared-snapshot-persistence-owner" -- --nocapture`
  - `cargo test chapter_single_generation_snapshot_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-shared-snapshot-persistence-owner" -- --nocapture`
  - `cargo test chapter_batch_generation --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-shared-snapshot-persistence-owner" -- --nocapture`
  - `cargo test chapter_single_generation --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-shared-snapshot-persistence-owner" -- --nocapture`

- 2026-06-06 同一条 `chapter_generation` Phase 5 lane 又继续收掉了一条
  “看起来像 single / generation 对 batch 的依赖，但其实只是 shared lower-level
  chapter access owner 仍寄居在 batch 文件名下”的 shared chapter-access seam。当前：
  - shared snapshot-query / task-recovery 与 shared snapshot persistence
    owner 都已经 lifted 之后，single-generation、generation runtime 和 batch resume
    邻域已经不再直接依赖 batch-named 的 lower-level query / recovery / persistence
    入口
  - 但多个非 batch 专属生产链之前仍共同通过：
    - `chapter_batch_generation_access_service.rs`
    - `load_accessible_chapter_for_generation(...)`
    - `load_accessible_chapters_for_generation(...)`
    去执行底层的 chapter access 语义
  - 这代表 chapter access lane 在 Phase 5 上仍保留一条旧 hop：
    batch / single / generation 邻域真正共享的 lower-level
    `chapter id(s) -> accessible generation chapter(s)` contract
    仍寄居在 batch 文件名下，cutover 审计时会继续把这条 shared access seam
    误判成 single 或 generation 对 batch 的直接依赖
  - 本轮已把这条真正 shared 的 lower-level access owner 前移回
    chapter-generation 邻域本身：
    - `backend-rs/src/services/chapter_generation_access_service.rs`
      现在直接拥有：
      - `LoadAccessibleChapterForGenerationError`
      - `load_accessible_chapter_for_generation(...)`
      - `load_accessible_chapters_for_generation(...)`
    - `chapter_generation_runtime_service.rs`
      现在直接消费这条 shared access owner
    - `chapter_batch_generation_resume_task_command_service.rs`
      现在也直接消费这条 shared access owner
    - `chapter_single_generation_prepare_service.rs`
      和
      `chapter_single_generation_stream_workflow_service.rs`
      现在直接消费这条 shared access owner，不再通过 batch access 文件名回跳
    - `backend-rs/src/api/chapter_generation_error_mapper.rs`
      也已切到新的 shared access error/type owner
    - `chapter_batch_generation_access_service.rs`
      已删除，因为它已不再承担独立的 batch compatibility boundary

  这条 seam 的意义不只是“把 access helper 换了个名字”。它真正回答的是：
  当 lower-level chapter access 语义已经被 batch / single / generation
  多条生产链共同消费时，到底是不是还应该继续寄居在 batch 文件名下。现在这条
  shared access seam 也已显式 lifted，chapter-generation 邻域更接近真实、
  可审计的 Rust owner boundary。

  对 Phase 5 的价值同样直接：
  - cutover 审计时更容易回答“shared chapter access 到底由哪个 Rust owner 接手”
  - single-generation / generation runtime / batch resume 不再因为文件命名关系
    被误判为继续依赖 batch 模块
  - fallback shrink / rollback / stronger smoke 在 chapter-generation
    shared seam 上又少了一条隐藏的 batch-named access hop

  这条 seam 已通过：
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/shared-generation-access-owner"`
  - `cargo test chapter_generation_runtime_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/shared-generation-access-owner" -- --nocapture`
  - `cargo test chapter_batch_generation_resume_task_command_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/shared-generation-access-owner" -- --nocapture`
  - `cargo test chapter_single_generation_prepare_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/shared-generation-access-owner" -- --nocapture`
  - `cargo test chapter_single_generation_stream_workflow_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/shared-generation-access-owner" -- --nocapture`

- 2026-06-06 同一条 `chapter_generation` Phase 5 shared lane 又继续收掉了一条
  “shared snapshot persistence owner 已经 lifted，但 persisted quality runtime
  context 仍回跳到 batch 文件名”的 shared quality runtime-context seam。当前：
  - `chapter_generation_snapshot_persistence_service.rs`
    之前已经是 chapter-generation 邻域里的 shared snapshot persistence owner
  - 但它之前仍通过：
    - `chapter_batch_generation_quality_runtime_context_service::resolve_batch_quality_runtime_context_from_persisted_sources(...)`
    回到 batch 文件名邻域重建 persisted quality runtime context
  - 这代表 shared snapshot persistence lane 在 Phase 5 上仍保留一条旧 hop：
    真正 shared 的 lower-level
    `persisted quality columns + summary state -> quality runtime context`
    contract 仍寄居在 batch 文件名下，cutover 审计时会继续把这条 shared quality seam
    误判成 chapter-generation shared owner 对 batch 模块的直接依赖
  - 本轮已把这条 lower-level persisted-source quality owner 进一步压回
    chapter-generation 邻域本身：
    - `chapter_generation_snapshot_persistence_service.rs`
      现在直接消费：
      - `resolve_generation_quality_runtime_context_from_persisted_sources("batch", ...)`
    - `chapter_generation_quality_runtime_context_service.rs`
      现在补上了 batch-scope focused regression，
      证明 shared owner 仍保持 batch 历史顺序和 summary-state 语义

  这条 seam 的意义不只是“把一个 helper 换到了 shared 文件”。它真正回答的是：
  当 snapshot persistence 本身已经被 lifted 成 chapter-generation shared owner 后，
  persisted quality runtime context 这条同样 shared 的 lower-level contract
  到底是不是还应该继续寄居在 batch 文件名下。现在这条 hop 也已显式收掉，
  shared snapshot persistence 链更接近真实、可审计的 Rust owner boundary。

  对 Phase 5 的价值同样直接：
  - cutover 审计时更容易回答“shared persisted quality runtime context 到底由哪个 Rust owner 接手”
  - chapter-generation shared persistence lane 不再因为文件命名关系
    被误判为继续依赖 batch quality owner
  - fallback shrink / rollback / stronger smoke 在 chapter-generation
    shared seam 上又少了一条隐藏的 batch-named quality hop

  这条 seam 已通过：
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/shared-generation-quality-context-owner"`
  - `cargo test chapter_generation_quality_runtime_context_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/shared-generation-quality-context-owner" -- --nocapture`
  - `cargo test chapter_generation_snapshot_persistence_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/shared-generation-quality-context-owner" -- --nocapture`

- 2026-06-06 同一条 `chapter_generation` Phase 5 lane 又继续收掉了一条
  “single background create / runtime lane 已经是 chapter-scoped owner，
  但 task insert seed 与 task-stage mutation 仍直接重开 batch task-model shape”的
  single task-model seam。当前：
  - single-generation background create / runtime 邻域之前已经显式拥有：
    - chapter-scoped startup snapshot planning
    - single background response payload projection
    - single runtime checkpoint stage projection
    - single runtime lifecycle persistence branches
  - 但
    `chapter_single_generation_prepare_service.rs`
    和
    `chapter_single_generation_runtime_state_service.rs`
    之前仍分别重开：
    - batch task seed 语义
    - file-local `ModelFieldUpdate` / `TaskTimestampUpdate` / `SingleGenerationTaskStage`
  - 这代表 single task-model lane 在 Phase 5 上仍保留一条
    Python-era 的旧 hop：single owner 已存在，但 chapter-scoped task insert /
    mutation contract 仍部分寄居在 batch task-model shape 或 runtime 文件局部复制里，
    cutover 审计时会继续把一个 chapter-only task-model boundary误判为 batch shared
    helper 或 file-local patch 逻辑
  - 本轮已把这条 chapter-scoped task-model contract 真正前移回
    single-generation 模块本身：
    - `backend-rs/src/services/chapter_single_generation_task_model_service.rs`
      现在直接拥有：
      - `SingleGenerationTaskPersistenceSeed`
      - `build_single_generation_background_task_persistence_seed(...)`
      - `build_single_generation_background_task_active_model(...)`
      - `ModelFieldUpdate`
      - `TaskTimestampUpdate`
      - `SingleGenerationTaskStage`
      - `SingleGenerationTaskStage::persist_for_task(...)`
      - `SingleGenerationTaskStage::apply_to_active_model(...)`
    - `chapter_single_generation_prepare_service.rs`
      现在直接消费这条 single task-seed owner，不再重开 batch task seed 语义
    - `chapter_single_generation_runtime_state_service.rs`
      现在也直接消费这条 single task-stage owner，不再保留同一套 task mutation contract
      的文件内联复制

  这条 seam 的意义不只是“多了一个 service 文件”。它真正回答的是：
  single-generation 的 background task insert 和 runtime task-stage persistence
  一旦已经被 Rust owner 接住，chapter-scoped task-model contract 到底是不是还要
  回到 batch task-model shape 或 runtime 文件内部再走一遍。现在这条 duplicate
  已被删掉，single task-model lane 更接近真实、可审计的 Rust owner boundary。

  对 Phase 5 的价值同样直接：
  - cutover 审计时更容易回答“single task insert / task-stage mutation 到底由哪个 Rust owner 接手”
  - single background create / runtime persistence 现在共享更连续的 owner 链
  - fallback shrink / rollback / stronger smoke 在 single-generation 邻域上
    又少了一条隐藏在 batch task-model shape 或 file-local mutation helper 里的 direct reopen 支路

  这条 seam 已通过：
  - `cargo test chapter_single_generation_task_model_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-task-model-owner-collapse" -- --nocapture`
  - `cargo test chapter_single_generation_prepare_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-task-model-owner-collapse" -- --nocapture`
  - `cargo test chapter_single_generation_runtime_state_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-task-model-owner-collapse" -- --nocapture`
  - `cargo test chapter_single_generation --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-task-model-owner-collapse" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-task-model-owner-collapse"`

- 2026-06-06 同一条 `chapter_generation` Phase 5 lane 又继续收掉了一条
  “single existing-background lane 已经是 dedicated single owner，
  但底层 recovery / snapshot query 仍直接挂在 batch 文件名上”的
  shared lower-level seam。当前：
  - single-generation existing-background 邻域之前已经显式拥有：
    - existing-background task lookup
    - recovered read-state / payload projection
    - dedicated single-generation file-local owner
  - 但
    `chapter_single_generation_existing_background_query_service.rs`
    之前仍直接重开：
    - `recover_batch_generation_task_if_needed(...)`
    - `load_batch_generation_snapshot_map(...)`
  - 这代表当前 remaining dependency 已经不再是“single 还没 own 住 query 语义”，
    而是一个真实 shared lower-level owner 仍寄居在 batch 邻域文件名下，
    cutover 审计时会继续把 single production lane 误判成直接依赖 batch file owner
  - 本轮没有再加一层 fake single facade，而是把真正共享的底层 owner
    直接前移回 chapter-generation shared 邻域：
    - `backend-rs/src/services/chapter_generation_task_recovery_service.rs`
      现在直接拥有：
      - `resolve_generation_task_auto_recovery_error(...)`
      - `recover_generation_task_if_needed(...)`
    - `backend-rs/src/services/chapter_generation_snapshot_query_service.rs`
      现在直接拥有：
      - `load_chapter_generation_snapshot(...)`
      - `load_chapter_generation_snapshot_map(...)`
    - 然后由这些 owner 直接被：
      - `chapter_single_generation_existing_background_query_service.rs`
      - `chapter_batch_generation_owned_task_query_service.rs`
      - `chapter_batch_generation_read_context_service.rs`
      - `chapter_batch_generation_runtime_state_service.rs`
      - `chapter_batch_generation_snapshot_service.rs`
      消费

  这条 seam 的意义不是“改了更中性的名字”。它真正回答的是：
  当 batch / single 都已经共享同一条底层 recovery / snapshot query 语义时，
  我们到底是继续保留 single -> batch file name 的误导性直连，
  还是把真正共享的 lower-level owner 显式抬出来。现在这条 direct attach
  已被删掉，single-generation lane 和 batch lane 都消费同一条可审计 shared owner。

  对 Phase 5 的价值同样直接：
  - cutover 审计时更容易回答“single existing-background 的底层 recovery / snapshot query 到底由哪个 Rust owner 接手”
  - single-generation 不再直接挂在 batch read-context / snapshot 文件名上
  - batch / single 仍继续共享同一份 lower-level 逻辑，没有再制造一层新的 forwarding facade
  - fallback shrink / rollback / stronger smoke 在 single-generation 邻域上
    又少了一条语义上已经失真的 batch file-name 依赖

  这条 seam 已通过：
  - `cargo test chapter_generation_task_recovery_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-shared-query-recovery-owner" -- --nocapture`
  - `cargo test chapter_batch_generation_read_context_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-shared-query-recovery-owner" -- --nocapture`
  - `cargo test chapter_batch_generation_owned_task_query_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-shared-query-recovery-owner" -- --nocapture`
  - `cargo test chapter_single_generation_existing_background_query_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-shared-query-recovery-owner" -- --nocapture`
  - `cargo test chapter_batch_generation --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-shared-query-recovery-owner" -- --nocapture`
  - `cargo test chapter_single_generation --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-shared-query-recovery-owner" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-shared-query-recovery-owner"`

- 2026-06-03 同一条 `chapter_generation` Phase 5 lane 又继续收掉了一条
  “batch resume persisted runtime owner 已存在，但 resume command 仍在本地重组
  restored request-state / quality-status / runtime-seed 语义”的 batch resume seam。
  当前：
  - `BatchGenerationPersistedRuntimeContext` 之前已经 materialize：
    - restored quality runtime context
    - resolved active story-repair payload
    - restored compat options
    - quality-status restoration inputs
  - 但
    `chapter_batch_generation_resume_task_command_service.rs` 里的
    `RestoredResumeRuntimeState::from_persisted_runtime_context(...)`
    仍在 persisted owner 之后本地重组：
    - `BatchGenerationRequestRuntimeState`
    - `BatchGenerationQualityStatusContext`
    - resume `runtime_state_seed`
  - 这代表 batch resume lane 仍保留一条 Python-era 的旧 hop：
    owner 已构造 -> caller 继续从 persisted sources 本地恢复同一 restored
    resume-state 语义
  - 本轮已把这条 restored-state 生命周期语义真正前移到 persisted owner 本身：
    - `backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs`
      中新增 `RestoredResumeRuntimeStateProjection`
    - `BatchGenerationPersistedRuntimeContext` 现在直接提供
      `build_restored_resume_runtime_state(...)`，可直接投影：
      - restored `BatchGenerationQualityStatusContext`
      - restored `BatchGenerationRequestRuntimeState`
      - restored resume `runtime_state_seed`
    - 同时新增 `build_resume_runtime_state_seed(...)`，把最后一段
      resume seed projection 也收回 runtime-state owner
    - `chapter_batch_generation_resume_task_command_service.rs` 中的
      `RestoredResumeRuntimeState::from_persisted_runtime_context(...)`
      现在直接消费这个 owner projection，不再本地重建 request-state /
      quality-status / runtime-seed

  这条 seam 的意义同样不只是 “persisted owner 多带一个 projection”。
  它真正回答的是：batch resume persisted runtime owner 在 Phase 5 语义上到底
  是不是 batch resume lane 的真实 restored-state 边界，还是只是一个中转壳，
  真正的 restored request-state / quality-status / runtime-seed 仍要在
  resume command 侧再恢复一次。现在这条隐式重组 hop 已经被删掉，
  persisted runtime owner 本身终于成为 restored resume-state 的显式
  Rust materialization 边界。

  对 Phase 5 的价值同样直接：
  - cutover 审计时更容易回答“batch resume 到底从哪个 Rust owner 接手
    restored request-state / quality-status / runtime-seed 语义”
  - batch runtime / batch resume / persistence-plan 现在共享更连续的 owner 链，
    而不是 owner 已存在、resume command 仍保留一条本地 rebuild 支路
  - fallback shrink / rollback / stronger smoke 在 batch resume lane 上又少了
    一条隐藏在 command owner 内部的 restored-state 重组支路

  这条 seam 已通过：
  - `cargo test should_restore_resume_runtime_state_from_shared_persisted_runtime_context_owner --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-resume-restored-state-owner" -- --nocapture`
  - `cargo test should_build_resume_runtime_state_seed_owner_with_restored_contracts --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-resume-restored-state-owner" -- --nocapture`
  - `cargo test should_keep_resume_persistence_plan_restored_runtime_state_owner_contract --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-resume-restored-state-owner" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-resume-restored-state-owner"`

- 2026-06-04 同一条 `chapter_generation` Phase 5 lane 又继续收掉了一条
  “single restored runtime owner 已存在，但 startup/runtime launch 投影仍从
  raw runtime JSON 本地反解析 request owner 语义”的 seam。当前：
  - `RestoredSingleGenerationRuntimeState` 之前已经 materialize：
    - `SingleGenerationStartupSnapshotPlan`
    - restore seed source
    - 基于 request owner 构造出来的 runtime-state payload
  - 但
    `chapter_single_generation_prepare_service.rs` 里的
    `into_startup_runtime_launch_parts(...)`
    仍在这个 owner 之后本地继续做一跳旧式恢复：
    - 先取出 `startup_snapshot_plan.runtime_state()`
    - 再通过
      `parse_batch_generation_request_runtime_state(Some(...))`
      反解析一份 `BatchGenerationRequestRuntimeState`
    - 再把这份重新恢复出来的 request owner 交给
      `build_single_generation_runtime_launch_input(...)`
  - 这代表 single restore -> startup/runtime launch lane 仍保留一条
    Python-era 的旧 hop：
    owner 已构造 -> 邻近 launch 投影继续从 raw runtime JSON 重建同一份
    request owner 语义
  - 本轮已把这条生命周期语义真正前移到 restored runtime owner 本身：
    - `backend-rs/src/services/chapter_single_generation_prepare_service.rs`
      中的 `RestoredSingleGenerationRuntimeState` 现在直接持有
      `BatchGenerationRequestRuntimeState`
    - `into_startup_runtime_launch_parts(...)` 现在直接消费这份 owner，
      不再从 `startup_snapshot_plan.runtime_state()` 做二次反解析
    - 同时补了 focused owner assertion，证明 restored runtime owner 本身
      已经携带 launch 需要的 request-runtime contract，而不是只交付一份
      待下游再恢复的 raw runtime payload

  这条 seam 的意义同样不只是 “struct 多带了一个字段”。它真正回答的是：
  single restored runtime owner 在 Phase 5 语义上到底是不是 single
  startup/runtime launch lane 的真实 request-owner 边界，还是只是一个中转壳，
  真正的 request owner 仍要在 launch 投影侧再从 runtime JSON 恢复一次。
  现在这条隐式重组 hop 已经被删掉，restored runtime owner 本身终于成为
  single request-runtime contract 的显式 Rust materialization 边界。

  对 Phase 5 的价值同样直接：
  - cutover 审计时更容易回答“single startup/runtime launch 到底从哪个 Rust
    owner 接手 request-runtime 语义”
  - single restore / startup snapshot / runtime launch 现在共享更连续的
    owner 链，而不是 owner 已存在、launch 投影仍保留一条 raw-runtime
    reparse 支路
  - fallback shrink / rollback / stronger smoke 在 single startup lane 上
    又少了一条隐藏在 prepare owner 内部的 request-owner 重组支路

  这条 seam 已通过：
  - `cargo test should_project_restored_single_generation_runtime_state_into_startup_and_runtime_launch_owner --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-single-restored-request-owner" -- --nocapture`
  - `cargo test should_convert_restored_single_generation_launch_into_runtime_input --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-single-restored-request-owner" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-single-restored-request-owner"`

- 2026-06-04 同一条 `chapter_generation` Phase 5 lane 又继续收掉了一条
  “batch resume restored runtime owner 已存在，但 launch persistence 仍把
  request-runtime / reset runtime seed 当成两条 caller-local 投影分别重组”的
  batch resume seam。当前：
  - `RestoredResumeRuntimeStateProjection` 之前已经 materialize：
    - restored `BatchGenerationQualityStatusContext`
    - restored `BatchGenerationRequestRuntimeState`
    - resume `runtime_state_seed`
  - 但
    `BatchGenerationResumeLaunchPersistencePlan::prepare_from_validated_execution(...)`
    仍在这个 owner 之后本地做两件事：
    - clone `runtime_state_seed` 给 reset persistence plan
    - 把完整 restored projection 继续传给
      `ResumeExecutionDispatchPlan::from_validated_execution(...)`，让 dispatch
      再取出 request-runtime owner
  - 这代表 batch resume launch lane 仍保留一条 Python-era 的旧 hop：
    owner 已构造 -> launch persistence 继续把同一 restored owner 拆成 reset
    seed 和 dispatch request-state 两条本地支路
  - 本轮已把这条生命周期语义真正前移到 restored resume owner 本身：
    - `backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs`
      中新增 `RestoredResumeRuntimeLaunchParts`
    - `RestoredResumeRuntimeStateProjection::into_launch_parts()` 现在直接投影：
      - restored `BatchGenerationRequestRuntimeState`
      - restored resume `runtime_state_seed`
    - `backend-rs/src/services/chapter_batch_generation_resume_task_command_service.rs`
      中的 `BatchGenerationResumeLaunchPersistencePlan::prepare_from_validated_execution(...)`
      现在消费这个 owner projection，一次性把 request-runtime state 交给
      dispatch-plan assembly，把 runtime seed 交给 reset-persistence assembly
    - `ResumeExecutionDispatchPlan::from_validated_execution(...)` 现在接收
      已恢复的 request-runtime owner，而不是重新接收完整 restored projection

  这条 seam 的意义不只是 “多了一个 parts struct”。它真正回答的是：
  batch resume restored runtime owner 在 Phase 5 语义上到底是不是 resume
  launch lane 的真实 request-runtime / reset-seed 投影边界，还是只是一个
  中转壳，真正的 reset persistence 和 dispatch request-state 仍要在 launch
  侧再拆一次。现在这条隐式重组 hop 已经被删掉，restored runtime owner 本身
  成为 batch resume launch-parts 的显式 Rust materialization 边界。

  对 Phase 5 的价值同样直接：
  - cutover 审计时更容易回答“batch resume launch 到底从哪个 Rust owner
    接手 request-runtime / reset-seed 语义”
  - batch resume restored-state / reset persistence / dispatch plan 现在共享
    更连续的 owner 链，而不是 owner 已存在、launch persistence 仍保留一条
    caller-local split 支路
  - fallback shrink / rollback / stronger smoke 在 batch resume lane 上又少了
    一条隐藏在 launch persistence 内部的 restored-owner 重组支路

  这条 seam 已通过：
  - `cargo test should_project_restored_resume_runtime_state_into_launch_parts_owner --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-resume-launch-parts-owner" -- --nocapture`
  - `cargo test should_build_resume_dispatch_plan_from_validated_execution_and_restored_runtime_owner --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-resume-launch-parts-owner" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-resume-launch-parts-owner"`

- 2026-06-04 同一条 `chapter_batch_generation` 模块包内又继续收掉了一条
  “create / resume 两条 lane 已经拥有相邻 runtime/checkpoint/quality owner，
  但 task response payload 仍分别在 write workflow 和 runtime-state 两侧本地重组”的
  batch response seam。当前：
  - batch create 一侧已经有：
    - create workflow launch / persistence owner
    - shared quality/runtime payload semantics
  - batch resume 一侧已经有：
    - resume reset/runtime-state persistence owner
    - shared quality/runtime payload semantics
  - 但 create / resume response payload 之前仍分散组装：
    - `chapter_batch_generation_write_workflow_service.rs`
      本地组装 create response payload
    - `chapter_batch_generation_runtime_state_service.rs`
      本地组装 resume response payload，并额外保留一层
      Python-stage-field 兼容薄包装
  - 这代表 `chapter_batch_generation` 模块在 Phase 5 上仍保留一条
    Python-era 的旧 hop：
    owner 已 materialize -> 邻近服务仍分别重放同一批 compat response 字段

  本轮已把这条兼容响应语义真正前移到共享 payload owner 本身：
  - `backend-rs/src/services/chapter_batch_generation_task_payload_base_service.rs`
    中新增：
    - `BatchGenerationTaskResponseQualityPayload`
    - `BatchGenerationTaskResponsePayloadOptions`
    - `build_batch_generation_task_response_payload_from_runtime_parts(...)`
  - 共享 payload owner 现在直接统一投影：
    - checkpoint/runtime metadata payload
    - summary payload
    - batch/single quality payload
    - active story-repair payload
    - quality history context
    - extra compat fields
    - loading-stage compatibility fields
  - `backend-rs/src/services/chapter_batch_generation_write_workflow_service.rs`
    中的 `batch_generation_create_response_payload(...)`
    现在直接消费这个 owner
  - `backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs`
    中的
    `BatchGenerationResumeResetPersistencePlan::into_resume_response_payload(...)`
    现在也直接消费这个 owner
  - 原来只剩委托意义、已不再被调用的
    `apply_python_resume_response_stage_fields(...)`
    也一并删除，避免把过渡壳继续带到下一轮模块迁移

  这条 seam 的意义不只是“多了一个 options struct”。它真正回答的是：
  `chapter_batch_generation` 的 create / resume task response contract
  到底是不是一个共享 Rust payload owner 的边界，还是仍然要在
  write workflow / runtime-state 两条 lane 上分别拼出一套高度重叠的
  compat response。现在这条隐式双重组装 hop 已经被删掉，共享 payload
  owner 本身成为 batch create/resume response contract 的显式 Rust
  materialization 边界。

  对 Phase 5 的价值同样直接：
  - cutover 审计时更容易回答“batch create / resume 的 compat response
    到底从哪个 Rust owner 接手”
  - create / resume / runtime-state persistence / quality payload 现在共享
    更连续的 owner 链，而不是 owner 已存在、相邻服务仍各自重放一遍
    compat response 字段
  - fallback shrink / rollback / stronger smoke 在
    `chapter_batch_generation` 模块包上又少了一条隐藏在相邻服务内部的
    response rebuild 支路

  这条 seam 已通过：
  - `cargo test should_apply_shared_loading_stage_fields_for_response_payload --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-response-owner" -- --nocapture`
  - `cargo test should_build_batch_generation_create_response_payload --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-response-owner" -- --nocapture`
  - `cargo test should_build_resume_response_payload_from_owner --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-response-owner" -- --nocapture`
  - `cargo test should_keep_resume_persistence_plan_restored_runtime_state_owner_contract --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-response-owner" -- --nocapture`
  - `cargo test should_build_batch_generation_create_persistence_plan_task_and_response_payload --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-response-owner" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-response-owner"`

- 2026-06-04 同一条 `chapter_batch_generation` 模块包内又继续收掉了一条
  “read-context owner 已经拥有 base task-view payload、quality status
  context、stream-state 邻接 owner，但 status / active-project /
  active-task-list / existing-background 四类查询视图仍在 read-context 侧
  分支式本地重组”的 batch read/query seam。当前：
  - batch read lane 已经有：
    - base task-view/runtime checkpoint payload owner
    - read-context loading / auto-recovery owner
    - quality status context owner
    - stream-state projection owner
  - 但 `chapter_batch_generation_read_context_service.rs`
    之前仍分别手工组装：
    - status task payload
    - active project task payload
    - active task list item payload
    - single-generation existing background payload
  - 这代表 `chapter_batch_generation` 模块在 Phase 5 上仍保留一条
    Python-era 的旧 hop：
    相邻 payload/quality owner 已 materialize，但 read-context 邻居仍按
    查询分支各自补字段、删字段、加 terminal/retry/background 元数据

  本轮已把这条查询视图语义真正前移到共享 payload owner 本身：
  - `backend-rs/src/services/chapter_batch_generation_task_payload_base_service.rs`
    中新增：
    - `BatchGenerationTaskViewPayloadVariant`
    - `build_batch_generation_task_view_payload_with_quality_context(...)`
  - 共享 payload owner 现在直接统一投影：
    - base task-view payload
    - quality payload injection
    - status-task retry / terminal fields
    - active-project field trimming
    - existing-background task metadata
  - `backend-rs/src/services/chapter_batch_generation_read_context_service.rs`
    中的：
    - `into_active_project_task_payload(...)`
    - `into_status_task_payload(...)`
    - `into_single_generation_existing_background_task_payload(...)`
    现在都直接消费这个 owner
  - `into_active_task_list_item_payload(...)` 也继续沿用同一 shared owner path，
    `read_context` 本身退回到 read-context loading 与 stream-state projection
    的职责边界

  这条 seam 的意义不只是“多了一个 enum variant”。它真正回答的是：
  `chapter_batch_generation` 的 read/query task-view contract
  到底是不是一个共享 Rust payload owner 的边界，还是仍然要在
  read-context 层对不同查询场景手工重放四套高度重叠的 compat view。
  现在这条隐式多分支 view rebuild hop 已经被删掉，共享 payload owner
  本身成为 batch read/query task-view contract 的显式 Rust materialization
  边界。

  对 Phase 5 的价值同样直接：
  - cutover 审计时更容易回答“batch status / active task / existing
    background 这些查询视图到底从哪个 Rust owner 接手”
  - read-context / status query / active-task query / stream-state projection
    现在共享更连续的 owner 链，而不是 read-context 邻居仍保留一层分支式
    response rebuild 支路
  - fallback shrink / rollback / stronger smoke 在
    `chapter_batch_generation` 模块包上又少了一条隐藏在 read-context
    内部的 query payload rebuild 支路

  这条 seam 已通过：
  - `cargo test should_build_status_task_view_payload_with_shared_owner_variant --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-read-query-owner" -- --nocapture`
  - `cargo test should_build_single_generation_existing_background_payload_with_shared_owner_variant --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-read-query-owner" -- --nocapture`
  - `cargo test should_build_status_task_payload_from_read_context --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-read-query-owner" -- --nocapture`
  - `cargo test should_build_active_task_payload_without_status_only_fields --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-read-query-owner" -- --nocapture`
  - `cargo test should_keep_active_task_payload_loader_projection_contract --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-read-query-owner" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-read-query-owner"`

- 2026-06-04 同一条 `chapter_batch_generation` 模块包内又继续收掉了一条
  “active-task query owner 已经拥有同一批 task row 选择/排序边界，
  但 active-task list / active-project query / existing-background 仍逐条
  recover task -> reload snapshot -> rebuild read-context”的 batch active-query
  bulk-read seam。当前：
  - batch read lane 已经有：
    - active-task row selection / ordering owner
    - snapshot persistence / read owner
    - read-context projection owner
    - quality status projection owner
  - 但 `chapter_batch_generation_task_view_query_service.rs`
    之前仍在三条相邻查询路径上重复逐条走：
    - auto-recovery
    - snapshot load
    - read-context assembly
  - 这代表 `chapter_batch_generation` 模块在 Phase 5 上仍保留一条
    Python-era 的旧 hop：
    query lane 明明已经拿到了同一批 active task row，
    但相邻 read lane 仍按 task 粒度重新进入 snapshot / read-context owner

  本轮已把这条 active-query bulk-read 语义真正前移到共享 owner 本身：
  - `backend-rs/src/services/chapter_batch_generation_snapshot_service.rs`
    中新增：
    - `load_batch_generation_snapshot_map(...)`
  - `backend-rs/src/services/chapter_batch_generation_read_context_service.rs`
    中新增：
    - `load_batch_generation_read_contexts_for_tasks(...)`
    - `load_active_batch_generation_read_contexts_for_tasks(...)`
  - `backend-rs/src/services/chapter_batch_generation_task_view_query_service.rs`
    现在直接消费这条批量 owner 链，不再在 list / active-project /
    existing-background 三条相邻查询路径里逐条重放 snapshot reload

  这条 seam 的意义不只是“把单条查询换成批量查询”。它真正回答的是：
  `chapter_batch_generation` 的 active-task read/query lane
  到底是不是一条连续 Rust owner 链，还是仍然要在 query owner 已经
  materialize 同一批 task rows 之后，再由邻层按 task 粒度逐条重复进入
  snapshot/read-context owner。现在这条隐式 per-task reload hop 已经被删掉，
  active-task list / active-project / existing-background
  三条查询边界共享同一条更连续的 Rust materialization lane。

  对 Phase 5 的价值同样直接：
  - cutover 审计时更容易回答“active-task list / active-project /
    existing-background 这些 batch 读链到底由哪个 Rust owner 接手”
  - read-context / active-task query / existing-background lookup
    现在共享更连续的 owner 链，而不是 query 邻居仍保留一层逐条 snapshot
    reload 支路
  - fallback shrink / rollback / stronger smoke 在
    `chapter_batch_generation` 模块包上又少了一条隐藏在 query/read 邻接层
    里的 per-task read-context rebuild 支路

  这条 seam 已通过：
  - `cargo test task_view_query --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-query-owner" -- --nocapture`
  - `cargo test read_context --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-query-owner" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-query-owner"`

- 2026-06-04 同一条 `chapter_batch_generation` 模块包内又继续收掉了一条
  “status-stream poll lane 明明只消费 stream-state owner，
  但每次轮询仍先 load owned read-context 再调用
  `context.into_stream_state()`”的 batch status-stream-state seam。当前：
  - batch stream lane 已经有：
    - owned task access owner
    - snapshot read owner
    - stream-state semantics owner
    - status stream polling / transport orchestration owner
  - 但 `chapter_batch_generation_status_stream_service.rs`
    之前仍在初始加载和每次 poll 上重复一条更宽的本地 hop：
    - load owned read context
    - 丢掉 read/query payload 侧字段
    - 只保留 `stream_state`
  - 这代表 `chapter_batch_generation` 模块在 Phase 5 上仍保留一条
    Python-era 的旧 hop：
    stream poll lane 其实只需要 stream-state materialization，
    但仍借道 read-context owner 才能拿到这份视图

  本轮已把这条 status-stream-state 语义真正前移到独立 owner 本身：
  - 新增
    `backend-rs/src/services/chapter_batch_generation_stream_state_query_service.rs`
  - 新 owner 直接承担：
    - `load_batch_generation_stream_state_for_task(...)`
    - `load_owned_batch_generation_stream_state(...)`
  - `backend-rs/src/services/chapter_batch_generation_status_stream_service.rs`
    现在直接消费这条 owner 链，不再在 poll lane 里先 materialize
    `BatchGenerationReadContext`
  - `backend-rs/src/services/chapter_batch_generation_read_context_service.rs`
    则退回 read/query payload owner 边界，不再继续挂着 stream-state
    projection 职责

  这条 seam 的意义不只是“多了一个 query service”。它真正回答的是：
  `chapter_batch_generation` 的 status-stream poll lane
  到底是不是一条连续的 Rust stream-state owner 链，还是仍然要在
  read-context owner 已 materialize 更宽视图之后，再由 poll 邻层丢弃大半
  字段只取 stream-state。现在这条隐式 read-context -> stream-state hop
  已经被删掉，status-stream poll / stream-state semantics / cursor-event
  resolution 共享同一条更连续的 Rust materialization lane。

  对 Phase 5 的价值同样直接：
  - cutover 审计时更容易回答“status-stream 轮询到底由哪个 Rust owner 接手”
  - status-stream poll / stream-state semantics / stream cursor-event batch
    现在共享更连续的 owner 链，而不是 poll 邻层仍保留一条
    read-context 中转支路
  - fallback shrink / rollback / stronger smoke 在
    `chapter_batch_generation` 模块包上又少了一条隐藏在 stream poll 邻层
    的 read-context 中转支路

  这条 seam 已通过：
  - `cargo test status_stream --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-stream-state-owner" -- --nocapture`
  - `cargo test stream_semantics --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-stream-state-owner" -- --nocapture`
  - `cargo test read_context --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-stream-state-owner" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-stream-state-owner"`

- 2026-06-04 同一条 `chapter_batch_generation` 模块包内又继续收掉了一条
  “stream event owner 已经拥有 cursor/state-driven event batch resolution，
  但 status stream service 仍本地构造 connected / task-not-found / timeout /
  heartbeat 这些系统事件与 transport 包装”的 batch status-stream seam。当前：
  - batch stream lane 已经有：
    - stream-state semantics owner
    - stream cursor / event-batch resolution owner
    - status stream polling / transport orchestration owner
  - 但 `chapter_batch_generation_status_stream_service.rs`
    之前仍本地组装：
    - connected payload
    - task-not-found payload
    - timeout payload
    - heartbeat comment / heartbeat event / data event wrapper
  - 这代表 `chapter_batch_generation` 模块在 Phase 5 上仍保留一条
    Python-era 的旧 hop：
    stream event owner 已 materialize，但相邻 transport owner 仍保留一层
    系统事件与 transport 包装的本地重建支路

  本轮已把这条 stream-system-event 语义真正前移到共享 event owner 本身：
  - `backend-rs/src/services/chapter_batch_generation_status_stream_event_service.rs`
    中新增：
    - `batch_generation_stream_connected_event_payload()`
    - `batch_generation_stream_task_not_found_event_payload()`
    - `batch_generation_stream_timeout_event_payload()`
    - `batch_generation_stream_heartbeat_comment()`
    - `batch_generation_stream_data_event(...)`
    - `batch_generation_stream_heartbeat_event()`
  - `backend-rs/src/services/chapter_batch_generation_status_stream_service.rs`
    现在直接消费这些 shared owner helpers，不再本地重放同一批
    transport/system event 契约

  这条 seam 的意义不只是“挪了几个 payload helper”。它真正回答的是：
  `chapter_batch_generation` 的 status-stream system event contract
  到底是不是共享 Rust event owner 的边界，还是仍然要在 polling/transport
  服务里手工重放一套 connected / timeout / heartbeat 兼容事件。现在这条
  隐式 stream-event rebuild hop 已经被删掉，共享 stream event owner 本身
  成为 batch status-stream system-event contract 的显式 Rust materialization
  边界。

  对 Phase 5 的价值同样直接：
  - cutover 审计时更容易回答“batch status stream 的系统事件到底从哪个
    Rust owner 接手”
  - stream-state semantics / cursor resolution / status polling /
    transport wrappers 现在共享更连续的 owner 链，而不是 transport 邻居
    仍保留一层 system-event rebuild 支路
  - fallback shrink / rollback / stronger smoke 在
    `chapter_batch_generation` 模块包上又少了一条隐藏在 status-stream
    service 内部的 event wrapper rebuild 支路

  这条 seam 已通过：
  - `cargo test should_build_status_stream_system_event_payloads_from_event_owner --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-stream-owner" -- --nocapture`
  - `cargo test should_build_status_stream_transport_events_from_event_owner --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-stream-owner" -- --nocapture`
  - `cargo test should_keep_status_stream_system_event_owner_contract --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-stream-owner" -- --nocapture`
  - `cargo test should_build_python_compatible_stream_connected_event_payload --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-stream-owner" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-stream-owner"`

- 2026-06-04 同一条 `chapter_batch_generation` 模块包内又继续收掉了一条
  “stream-state owner 已经 materialize 了 phase / event-status /
  analysis-started / quality payload 等流式语义，但 cursor 仍只用
  `status/completed/progress/message` 四个字段做本地变更判定”的
  batch stream observation seam。当前：
  - batch stream lane 已经有：
    - stream-state semantics owner
    - state-driven event batch owner
    - status stream polling / transport orchestration owner
  - 但旧的 `BatchGenerationStreamCursor`
    之前仍本地缓存并比较：
    - `status`
    - `completed`
    - `progress`
    - `message`
  - 这代表 `chapter_batch_generation` 模块在 Phase 5 上仍保留一条隐藏的
    Python-era 旧 hop：
    state owner 已经回答了更多 stream 语义，但相邻 cursor owner 仍保留一条
    “只观察局部字段、决定要不要发下一批 SSE event”的本地裁剪支路

  本轮已把这条 stream observation contract 真正前移到共享 stream-state owner：
  - `backend-rs/src/services/chapter_batch_generation_stream_semantics_service.rs`
    中新增：
    - `BatchGenerationStreamObservationKey`
    - `BatchGenerationStreamState::observation_key()`
  - `backend-rs/src/services/chapter_batch_generation_status_stream_event_service.rs`
    里的 `BatchGenerationStreamCursor`
    现在改为保存 owner-ready 的 `observation`
  - `backend-rs/src/services/chapter_batch_generation_status_stream_service.rs`
    也同步切到新的 observation owner，不再自己维护那组局部字段缓存

  这条 seam 的意义不只是“cursor 多比较了几个字段”。它真正回答的是：
  `chapter_batch_generation` 的 status stream 到底由谁定义
  “什么时候应该再次发出一批 SSE event” 这条契约。现在这条契约已经和
  stream-state owner 放在同一个 Rust materialization 边界上，而不再让
  cursor 邻层偷偷保留一份缩减版观察模型。

  对 Phase 5 的价值同样直接：
  - cutover 审计时更容易回答“phase-only / quality-gate / analysis-started
    的变化是否一定会被 stream 看见”
  - stream-state semantics / observation key / event batch / status polling
    现在形成了更连续的 owner 链
  - fallback shrink / rollback / stronger smoke 在
    `chapter_batch_generation` 模块包上又少了一条隐藏在 cursor 内部的
    partial-state compare 支路

  这条 seam 已通过：
  - `cargo test should_build_stream_observation_key_from_state_owner --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-stream-observation-owner" -- --nocapture`
  - `cargo test should_emit_stream_event_batch_when_phase_changes_under_same_progress --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-stream-observation-owner" -- --nocapture`
  - `cargo test should_emit_stream_event_batch_when_analysis_started_fields_change --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-stream-observation-owner" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-stream-observation-owner"`

- 2026-06-04 同一条 `chapter_batch_generation` 模块包内又继续收掉了一条
  “status lane 明明只消费 status-task payload owner，但 route status query /
  cancel persistence response 仍先 materialize 更宽的 read-context 再只取
  status payload”的 batch status-task-query seam。当前：
  - batch status lane 已经有：
    - owned task access owner
    - snapshot loading owner
    - quality-status projection owner
    - shared status-task payload owner
  - 但两个相邻 caller 之前仍保留一条更宽的本地 hop：
    - route status query：
      `load_owned_batch_generation_read_context(...) -> into_status_task_payload()`
    - cancel persistence response：
      `task + merged runtime state -> BatchGenerationReadContext -> status payload`
  - 这代表 `chapter_batch_generation` 模块在 Phase 5 上仍保留一条
    Python-era 的旧 hop：
    caller 明明只需要 status-task payload materialization，
    但仍借道主要服务于 read/query 视图的 read-context owner

  本轮已把这条 status-task-query 语义真正前移到独立 owner 本身：
  - 新增
    `backend-rs/src/services/chapter_batch_generation_status_task_query_service.rs`
  - 新 owner 直接承担：
    - `build_batch_generation_status_task_payload_with_quality_context(...)`
    - `build_batch_generation_status_task_payload_from_task_and_snapshot_projection(...)`
    - `load_owned_batch_generation_status_payload(...)`
  - `backend-rs/src/api/chapter_batch_generation.rs`
    的 status query 现在直接消费这条 owner 链
  - `backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs`
    的 cancel persistence response 现在也直接消费同一条 owner 链，
    不再先 materialize `BatchGenerationReadContext`
  - `backend-rs/src/services/chapter_batch_generation_read_context_service.rs`
    则进一步退回 read/query payload 与 stream-state 邻接职责，
    不再继续挂着 status-task payload 这条生产链

  这条 seam 的意义不只是“多了一个 query service”。它真正回答的是：
  `chapter_batch_generation` 的 status-task payload contract
  到底是不是一条连续的 Rust owner 链，还是仍然要在
  read-context owner 已 materialize 更宽视图之后，再由 route/write 邻层
  丢弃大半字段只取 status payload。现在这条隐式
  read-context -> status payload hop 已经被删掉，route status query /
  cancel persistence response / shared status payload owner 共享同一条更连续的
  Rust materialization lane。

  对 Phase 5 的价值同样直接：
  - cutover 审计时更容易回答“batch status payload 到底由哪个 Rust owner 接手”
  - route status query / cancel persistence / quality status projection
    现在共享更连续的 owner 链，而不是 route/write 邻层仍保留一条
    read-context 中转支路
  - fallback shrink / rollback / stronger smoke 在
    `chapter_batch_generation` 模块包上又少了一条隐藏在
    read/write 邻接层里的 status payload rebuild 支路

  这条 seam 已通过：
  - `cargo test status_task_query --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-status-task-owner" -- --nocapture`
  - `cargo test task_view_query --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-status-task-owner" -- --nocapture`
  - `cargo test read_context --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-status-task-owner" -- --nocapture`
  - `cargo test should_aggregate_recent_history_quality_summaries_before_seeding_batch_runtime_state --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-status-task-owner" -- --nocapture`
  - `cargo test should_aggregate_recent_story_repair_quality_summaries --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-status-task-owner" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-status-task-owner"`

- 2026-06-04 同一条 `chapter_generation` Phase 5 lane 又继续沿
  batch resume -> single dispatch 的相邻 owner 链再收掉了一条
  “single runtime launch owner 已经存在，但 resume 单章分支仍本地重组
  `SingleGenerationRuntimeLaunchInput`” 的 lifecycle seam。当前：
  - `chapter_single_generation_prepare_service.rs`
    已经拥有单章 request-runtime-state -> execution-config ->
    runtime-launch-input 的主链 owner；
  - 但 `chapter_batch_generation_resume_task_command_service.rs`
    之前仍在 single-chapter resume 分支本地重复做同一组拼装：
    - 调 `prepare_single_chapter_generation_execution_config_from_runtime_state(...)`
    - 手工塞入 `chapter_id` / `user_id`
    - 手工塞入 `target_word_count`
    - 再把 `request_runtime_state.compat_options` 克隆回
      `SingleGenerationRuntimeLaunchInput`
  - 这代表 `chapter_generation` 的 single startup-to-runtime owner 链
    与 batch resume -> single dispatch 邻链之间仍保留一条
    Python-era 的旧 hop：owner 已 materialize，但相邻 resume 分支仍保留
    一份本地 runtime-launch 重组支路

  本轮已把这条 single resume launch contract 真正前移回 shared single owner：
  - `backend-rs/src/services/chapter_single_generation_prepare_service.rs`
    中新增：
    - `build_single_generation_runtime_launch_input_from_request_runtime_state(...)`
    - `prepare_single_chapter_runtime_launch_input_from_request_runtime_state(...)`
  - `backend-rs/src/services/chapter_batch_generation_resume_task_command_service.rs`
    的 single-chapter resume 分支现在直接消费该 shared owner，
    不再手工重组 `SingleGenerationRuntimeLaunchInput`
  - 为了让 focused test 继续走真实 provider-payload 解析路径，
    resume 单章测试库也补齐了最小必要表：
    - `character`
    - `career`
    - `character_career`
    - `story_memory`
    - `foreshadow`

  这条 seam 的意义不只是“抽了一个 helper”。它真正回答的是：
  `chapter_generation` 在 single runtime launch 这条契约上，到底是由一个
  shared single owner 统一 materialize，还是要让 background start / stream /
  batch resume single dispatch 各自再拼一份 launch 输入。现在 batch resume
  的 single 分支已经和 single startup lane 并回同一个 Rust owner 边界上，
  这条隐藏的 lifecycle duplicate 已被删掉。

  对 Phase 5 的价值同样直接：
  - cutover 审计时更容易回答“single-chapter resume 的 runtime launch
    到底由哪个 Rust owner 接手”
  - batch resume / single startup-to-runtime 两条相邻 owner 链现在共享更一致的
    launch-input materialization contract
  - 后续 fallback shrink / rollback / stronger smoke 若继续沿
    `chapter_generation` 邻域推进，又少了一条藏在 resume command service
    内部的 single-launch rebuild 支路

  这条 seam 已通过：
  - `cargo test should_build_single_generation_runtime_launch_input_from_request_runtime_state_owner --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-single-resume-launch-owner" -- --nocapture`
  - `cargo test should_build_single_chapter_resume_dispatch_plan_from_validated_execution_and_restored_runtime_owner --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-single-resume-launch-owner" -- --nocapture`
  - `cargo test should_build_resume_dispatch_plan_from_validated_execution_and_restored_runtime_owner --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-single-resume-launch-owner" -- --nocapture`
  - `cargo test should_project_restored_single_generation_runtime_state_into_startup_and_runtime_launch_owner --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-single-resume-launch-owner" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-single-resume-launch-owner"`

- 2026-06-04 同一条 `chapter_generation` Phase 5 lane 又继续沿
  batch create / batch resume / batch runtime 的共享 launch owner 再收掉了一条
  “shared batch launch owner 已经存在，但 create / resume 邻层仍本地 parse /
  rebuild 同一 launch contract” 的 lifecycle seam。当前：
  - `backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs`
    已经拥有 shared batch runtime launch owner，负责：
    - runtime compat restore
    - batch runtime execution input 组装
    - create lane 的 startup snapshot + runtime launch input 投影
  - 但相邻 create / resume lane 之前仍各自保留两条旧 hop：
    - batch resume 仍在
      `chapter_batch_generation_resume_task_command_service.rs`
      本地重建 batch runtime launch input
    - batch create 仍在
      `chapter_batch_generation_write_workflow_service.rs`
      里先把 `runtime_state_seed` 再 parse 回
      `request_runtime_state`，然后再交回 shared launch owner
  - 这代表 `chapter_generation` 主线在 Phase 5 上仍保留一条 Python-era 旧支路：
    shared batch launch owner 已经 materialize，但 create / resume 邻层仍会先
    本地重放同一份 parse/rebuild contract，再进入 dispatch / startup persistence

  本轮已把这条 shared batch launch contract 真正前移回统一 owner：
  - `backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs`
    保持并继续成为唯一 launch owner，负责：
    - `restore_batch_generation_runtime_compat_options_from_runtime_state_seed(...)`
    - `build_batch_generation_runtime_launch_input_from_runtime_state_seed(...)`
    - `prepare_batch_generation_runtime_launch_input_from_request_runtime_state(...)`
  - `build_batch_generation_startup_snapshot_and_runtime_launch_input_from_runtime_state_seed(...)`
    现在会在 shared owner 内部直接从 `runtime_state_seed` 恢复
    `batch_request_runtime_state`，不再要求 create 邻层先 parse 再回传
  - `BatchGenerationCreateRuntimeSeed` 现在也进一步瘦身成只持有
    `runtime_state_payload`，create workflow launch 直接把这份 owner 交给
    shared batch launch owner，而不再在本地缓存并重放 compat restore 语义
  - batch resume lane 则继续直接消费 shared batch launch owner，不再本地重组
    batch runtime execution input

  这条 seam 的意义不只是“少了一次 parse”。它真正回答的是：
  `chapter_generation` 的 batch runtime launch contract
  到底是不是由一条 shared Rust owner 统一 materialize，还是仍允许 create /
  resume 邻层各自保留一条平行 parse/rebuild 支路。现在这条隐藏的 duplicate
  已被删掉，batch create / batch resume / batch runtime 在 launch-input 这一层
  更接近同一条 cutover-ready owner 链。

  对 Phase 5 的价值同样直接：
  - cutover 审计时更容易回答“batch runtime launch 到底由哪个 Rust owner 接手”
  - create startup / resume dispatch / runtime launch 现在共享更连续的 owner 链
  - fallback shrink / rollback / stronger smoke 在这条主链上又少了一条隐藏的
    parse/rebuild 支路

  这条 seam 已通过：
  - `cargo test should_materialize_batch_generation_create_workflow_launch_parts_inside_runtime_seed_owner --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-launch-owner" -- --nocapture`
  - `cargo test should_build_batch_generation_launch_input_from_runtime_state_seed_owner --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-launch-owner" -- --nocapture`
  - `cargo test should_build_resume_dispatch_plan_from_validated_execution_and_restored_runtime_owner --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-launch-owner" -- --nocapture`
  - `cargo test should_build_single_chapter_resume_dispatch_plan_from_validated_execution_and_restored_runtime_owner --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-launch-owner" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-launch-owner"`

- 2026-06-04 同一条 `chapter_generation` Phase 5 lane 又继续沿 batch create
  write-lane 再收掉了一条
  “workflow-launch owner 已经 materialize，但 persistence 邻层仍本地重组
  queued create response contract” 的 seam。当前：
  - `PreparedBatchGenerationCreateWorkflowLaunch`
    之前已经拥有 batch create workflow-launch 的主链 owner：
    - task spec
    - chapter targets
    - startup snapshot plan
    - runtime launch input
  - 但
    `BatchGenerationCreateLaunchPersistencePlan::from_workflow_launch(...)`
    之前仍会本地再组一次：
    - queued create response payload
    - 然后才进入 persistence / dispatch
  - 这代表 batch create write-lane 在 Phase 5 上仍保留一条 Python-era 旧 hop：
    workflow-launch owner 已 materialize，但紧邻 persistence owner 仍保留一条
    本地 queued response rebuild 支路

  本轮已把这条 persistence-ready response contract 真正前移回
  workflow-launch owner：
  - `backend-rs/src/services/chapter_batch_generation_write_workflow_service.rs`
    中新增
    `PreparedBatchGenerationCreateWorkflowPersistenceParts`
  - `PreparedBatchGenerationCreateWorkflowLaunch::into_persistence_parts(...)`
    现在会直接 materialize 一份 persistence-ready owner contract，包含：
    - `task_id`
    - `project_id`
    - task spec
    - chapter targets
    - startup snapshot plan
    - queued response payload
    - runtime launch input
  - `into_persistence_plan(...)` 现在直接消费这份 owner parts
  - `BatchGenerationCreateLaunchPersistencePlan`
    也改成消费 owner-projected persistence parts，不再自己本地 rebuild queued
    response payload

  这条 seam 的意义不只是“response helper 换了个位置”。它真正回答的是：
  batch create workflow launch 一旦已经被 Rust owner materialize，queued create
  response contract 到底是不是继续由这个 owner 往前传递，还是要在 persistence
  邻层重新拼一次。现在这条 duplicate 已被删掉，batch create write-lane 在
  response-ready ownership 上又向前收了一层。

  对 Phase 5 的价值同样直接：
  - cutover 审计时更容易回答“queued create response 到底从哪个 Rust owner 出来”
  - batch create 的 workflow-launch / response / persistence 现在共享更连续的
    owner 链
  - fallback shrink / rollback / stronger smoke 在 batch create write-lane 上
    又少了一条隐藏在 persistence 邻层的 response rebuild 支路

  这条 seam 已通过：
  - `cargo test should_materialize_batch_generation_create_persistence_parts_inside_workflow_launch_owner --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-create-persistence-owner" -- --nocapture`
  - `cargo test should_build_batch_generation_create_workflow_launch_into_persistence_plan --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-create-persistence-owner" -- --nocapture`
  - `cargo test should_build_batch_generation_create_persistence_plan_task_and_response_payload --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-create-persistence-owner" -- --nocapture`
  - `cargo test should_keep_batch_generation_create_persistence_plan_owner_contract_explicit --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-create-persistence-owner" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-create-persistence-owner"`

- 2026-06-04 同一条 `chapter_generation` Phase 5 lane 又继续沿 batch create
  write-lane 再收掉了一条
  “workflow/persistence owner 已经 materialize，但 persistence 邻层仍本地重组
  task active-model insert contract” 的 seam。当前：
  - `PreparedBatchGenerationCreateWorkflowPersistenceParts`
    之前已经拥有 batch create persistence-ready owner：
    - startup snapshot plan
    - queued response payload
    - runtime launch input
  - 但
    `BatchGenerationCreateLaunchPersistencePlan::background_task_active_model(...)`
    之前仍会本地再组一次：
    - `task_id`
    - `project_id`
    - task spec 字段
    - chapter count / chapter ids
    - runtime input user / target-word-count
    - 然后才进入 `task.insert(...)`
  - 这代表 batch create write-lane 在 Phase 5 上仍保留一条 Python-era 旧 hop：
    workflow / persistence owner 已 materialize，但紧邻 persistence step 仍保留一条
    task insert contract 的本地 field-by-field rebuild 支路

  本轮已把这条 persistence-ready task contract 真正前移回 owner：
  - `backend-rs/src/services/chapter_batch_generation_task_model_service.rs`
    中新增
    `BatchGenerationTaskPersistenceSeed`
  - `backend-rs/src/services/chapter_batch_generation_write_workflow_service.rs`
    中
    `PreparedBatchGenerationCreateWorkflowLaunch::into_persistence_parts(...)`
    现在会直接 materialize 一份 persistence-ready task seed，包含除了
    persistence-time `now` 之外的全部 task insert inputs
  - `BatchGenerationCreateLaunchPersistencePlan`
    现在改成保存 owner-projected `task_seed`，不再保留
    `task_spec + chapters_to_generate` 再本地重组 task model
  - `BatchGenerationTaskPersistenceSeed::into_active_model(now)`
    保留了最后一步 `created_at` 的 persistence-time 注入，确保 owner boundary
    收窄但不把 insert-time 语义错误前移到 workflow launch

  这条 seam 的意义不只是“task builder 换了个参数对象”。它真正回答的是：
  batch create workflow launch / persistence parts 一旦已经被 Rust owner
  materialize，task insert contract 到底是不是继续由这个 owner 往前传递，
  还是要在 persistence 邻层把同一批字段重新拼一次。现在这条 duplicate
  已被删掉，batch create write-lane 在 task-persistence ownership 上又向前收了
  一层。

  对 Phase 5 的价值同样直接：
  - cutover 审计时更容易回答“batch create task insert 到底由哪个 Rust owner
    提供 contract”
  - batch create 的 workflow-launch / response / task persistence /
    startup snapshot / runtime dispatch 现在共享更连续的 owner 链
  - fallback shrink / rollback / stronger smoke 在 batch create write-lane 上
    又少了一条隐藏在 persistence 邻层的 task-model rebuild 支路

  这条 seam 已通过：
  - `cargo test batch_generation_create --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-create-task-seed-owner" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-create-task-seed-owner"`

- 2026-06-04 同一条 `chapter_generation` Phase 5 lane 也继续沿
  single background write-lane 再收掉了一条
  “restored launch / startup snapshot / response owner 已经 materialize，但
  persistence 邻层仍本地重组 single background task insert contract” 的 seam。
  当前：
  - `PreparedSingleGenerationBackgroundLaunchParts`
    已经拥有 single background persistence-ready owner：
    - startup snapshot plan
    - background response payload
    - runtime launch input
  - 但
    `SingleGenerationBackgroundLaunchPersistencePlan::background_task_active_model(...)`
    之前仍会本地再组一次：
    - `task_id`
    - `chapter_target`
    - runtime input 派生的 `user_id`
    - runtime input 派生的 `target_word_count`
    - 然后才进入 `task.insert(...)`
  - 这代表 single background write-lane 在 Phase 5 上仍保留一条
    Python-era 旧 hop：owner 已经 materialize，但 persistence 邻层仍保留一条
    task active-model insert contract 的本地 rebuild 支路

  本轮已把这条 persistence-ready task contract 真正前移回 owner：
  - `backend-rs/src/services/chapter_single_generation_prepare_service.rs`
    中
    `SingleChapterGenerationTarget`
    现在显式提供
    `background_task_persistence_seed(...)`
  - `PreparedSingleGenerationBackgroundLaunchParts`
    现在直接携带 owner-projected `task_seed`
  - `backend-rs/src/services/chapter_single_generation_write_workflow_service.rs`
    中
    `SingleGenerationBackgroundLaunchPersistencePlan`
    现在改成保存
    `BatchGenerationTaskPersistenceSeed`
    而不再保留 `task_id + chapter_target` 再本地重组 task model
  - `task_seed.into_active_model(now)` 保留最后一步 persistence-time
    `created_at` 注入，确保 owner boundary 收窄但不把 insert-time 语义错误前移

  这条 seam 的意义不只是“single background task builder 换了个参数对象”。
  它真正回答的是：single background write-lane 的 restored launch /
  response / persistence parts 一旦已经被 Rust owner materialize，task insert
  contract 到底是不是继续由这个 owner 往前传递，还是要在 persistence 邻层
  把同一批字段重新拼一次。现在这条 duplicate 已被删掉，single background
  write-lane 在 task-persistence ownership 上又向前收了一层。

  对 Phase 5 的价值同样直接：
  - cutover 审计时更容易回答“single background task insert 到底由哪个 Rust
    owner 提供 contract”
  - single background 的 response / task persistence / startup snapshot /
    runtime dispatch 现在共享更连续的 owner 链
  - fallback shrink / rollback / stronger smoke 在 single background
    write-lane 上又少了一条隐藏在 persistence 邻层的 task-model rebuild 支路

  这条 seam 已通过：
  - `cargo fmt --manifest-path "backend-rs/Cargo.toml"`
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-single-background-task-seed-owner"`
  - `cargo test single_generation_background --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-single-background-task-seed-owner" -- --nocapture`
  - `cargo test should_build_single_generation_background_launch_persistence_plan_from_prepared_owner --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-single-background-task-seed-owner" -- --nocapture`

- 2026-06-04 同一条 `chapter_generation` Phase 5 lane 也继续沿
  batch cancel write-lane 再收掉了一条
  “cancelled persistence / merged runtime checkpoint / status payload owner
  已经 materialize，但 cancel workflow 邻层仍本地补最后一层
  `Batch generation cancelled` response envelope” 的 seam。当前：
  - `BatchGenerationCancelledPersistencePlan`
    已经拥有：
    - merged cancelled runtime state
    - quality-aware cancelled status payload
    - task progress source
  - 但
    `cancel_owned_batch_generation_task(...)`
    之前仍会在 owner 外部再做一次：
    - 本地重建 `BatchGenerationCommandProgressSummary`
    - 本地 merge `Batch generation cancelled`
    - 然后才返回最终 cancel response
  - 这代表 batch cancel write-lane 在 Phase 5 上仍保留一条
    Python-era 旧 hop：owner 已经 materialize，但 workflow 邻层仍保留一条
    final response-envelope 的本地 rebuild 支路

  本轮已把这条 final response-envelope contract 真正前移回 owner：
  - `backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs`
    中
    `BatchGenerationCancelledPersistencePlan::build_response_payload_for_task(...)`
    现在同时拥有：
    - cancelled status payload projection
    - final command-summary response-envelope projection
  - `backend-rs/src/services/chapter_batch_generation_cancel_service.rs`
    中
    `cancel_owned_batch_generation_task(...)`
    现在直接返回 owner-projected payload，
    不再本地 append summary fields
  - focused regression 也把 final cancel envelope 的关键断言前移到
    runtime-state owner 测试，保护真正的 owner 边界

  这条 seam 的意义不只是“cancel helper 少了一层 map extend”。它真正回答的是：
  batch cancel write-lane 的 cancelled persistence / runtime-state merge /
  status payload 一旦已经被 Rust owner materialize，最终 response envelope
  到底是不是继续由这个 owner 往前传递，还是要在 workflow 邻层把同一批
  summary 字段再拼一次。现在这条 duplicate 已被删掉，batch cancel
  write-lane 在 final response ownership 上又向前收了一层。

  对 Phase 5 的价值同样直接：
  - cutover 审计时更容易回答“batch cancel 最终 response envelope 到底由哪个
    Rust owner 提供 contract”
  - batch cancel 的 persistence / runtime checkpoint merge / status payload /
    final response 现在共享更连续的 owner 链
  - fallback shrink / rollback / stronger smoke 在 batch cancel write-lane 上
    又少了一条隐藏在 workflow 邻层的 local summary rebuild 支路

  这条 seam 已通过：
  - `cargo fmt --manifest-path "backend-rs/Cargo.toml"`
  - `cargo test cancel --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-cancel-response-owner" -- --nocapture`
  - `cargo test read_context --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-cancel-response-owner" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-cancel-response-owner"`

- 2026-06-04 同一条 `chapter_generation` Phase 5 lane 也继续沿
  single resume / restored-launch 邻链再收掉了一条
  “restored runtime owner 已经 materialize，但 single resume command 分支仍本地
  reopen `into_launch_parts()` 再重组 runtime launch input” 的 seam。当前：
  - `RestoredResumeRuntimeStateProjection`
    已经拥有：
    - restored request runtime state
    - runtime state seed
    - quality status context
  - 但
    `ResumeExecutionDispatchPlan::from_validated_execution(...)`
    之前在 single 分支仍会：
    - 本地 `into_launch_parts()`
    - 本地拿 `request_runtime_state`
    - 本地单独透传 `runtime_state_seed`
    - 然后才在 command 邻层重组 single runtime launch input
  - 这代表 single resume lane 在 Phase 5 上仍保留一条 Python-era 旧 hop：
    owner 已经 materialize，但 command 邻层仍保留一条 restored-launch source
    的本地 reopen / split 支路

  本轮已把这条 restored-launch contract 真正前移回 owner：
  - `backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs`
    中新增
    `PreparedSingleChapterResumeRuntimeLaunch`
    与
    `prepare_single_chapter_resume_runtime_launch_from_restored_state(...)`
  - 这个 owner 现在直接投影：
    - single runtime launch input
    - resume runtime-state seed handoff
  - `backend-rs/src/services/chapter_batch_generation_resume_task_command_service.rs`
    中
    single 分支现在直接消费 owner 结果，
    不再本地 reopen `into_launch_parts()`

  这条 seam 的意义不只是“少调用了一个 helper”。它真正回答的是：
  single resume lane 的 restored runtime-state 一旦已经被 Rust owner
  materialize，single dispatch launch 到底是不是继续由这个 owner 往前传递，
  还是要在 command 邻层把同一批 restored source 再拆一次。现在这条
  duplicate 已被删掉，single resume lane 在 restored-launch ownership 上又向前
  收了一层。

  对 Phase 5 的价值同样直接：
  - cutover 审计时更容易回答“single resume launch 到底由哪个 Rust owner
    提供 contract”
  - single resume restored-state / launch / dispatch plan 现在共享更连续的
    owner 链
  - fallback shrink / rollback / stronger smoke 在 single resume lane 上又少了
    一条隐藏在 command 邻层的 restored-source reopen 支路

  这条 seam 已通过：
  - `cargo fmt --manifest-path "backend-rs/Cargo.toml"`
  - `cargo test resume --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-single-resume-launch-owner" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-single-resume-launch-owner"`

- 2026-06-05 同一条 `chapter_generation` Phase 5 lane 又沿
  single background / existing-task query 邻链再收掉了一条
  “existing-background payload owner 已经 materialize，但 write workflow 仍本地
  reopen `BatchGenerationReadContext` 再重组 compat payload” 的 seam。当前：
  - `chapter_batch_generation` / `chapter_single_generation`
    邻域已经有：
    - active task selection owner
    - read-context loading / quality-runtime projection owner
    - shared existing-background payload variant owner
  - 但
    `chapter_single_generation_write_workflow_service.rs`
    之前在已有后台任务短路分支仍会：
    - 先拿 `BatchGenerationReadContext`
    - 本地 reopen read context
    - 然后才转成
      `single-generation existing background payload`
  - 这代表 single background write lane 在 Phase 5 上仍保留一条
    Python-era 旧 hop：
    owner 已经 materialize，但 workflow 邻层仍保留一条 read-context ->
    compat payload 的本地 reopen / projection 支路

  本轮已把这条 existing-background payload contract 真正前移回 query owner：
  - `backend-rs/src/services/chapter_batch_generation_task_view_query_service.rs`
    中新增
    `load_existing_single_generation_background_task_payload(...)`
  - 这个 owner 现在直接投影：
    - active existing-background task compat payload
  - `backend-rs/src/services/chapter_single_generation_write_workflow_service.rs`
    中
    已有后台任务短路分支现在直接消费 owner payload，
    不再本地 reopen `BatchGenerationReadContext`

  这条 seam 的意义不只是“少了一层 map”。它真正回答的是：
  single background write lane 在判断“已有后台任务正在执行”时，
  compat payload 到底由哪个 Rust owner 提供 contract。现在这条 duplicate
  已被删掉，single background write lane 在 existing-task query ownership
  上又向前收了一层。

  对 Phase 5 的价值同样直接：
  - cutover 审计时更容易回答“single existing-background payload 到底由哪个
    Rust owner 提供 contract”
  - active-task lookup / read projection / compat payload 现在共享更连续的
    owner 链
  - fallback shrink / rollback / stronger smoke 在 single background
    write-lane 上又少了一条隐藏在 workflow 邻层的 local payload projection
    支路

  这条 seam 已通过：
  - `cargo fmt --manifest-path "backend-rs/Cargo.toml"`
  - `cargo test existing_background --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-single-existing-background-owner" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-single-existing-background-owner"`

- 2026-06-05 同一条 `chapter_generation` Phase 5 lane 又沿
  single stream / terminal completion 邻链再收掉了一条
  “stream workflow 已经拿到 terminal `result + analysis`，但 completion
  message / SSE follow-up events / compat result payload 仍在 workflow 邻层
  分散本地重组” 的 seam。当前：
  - `chapter_single_generation` stream 邻域已经有：
    - terminal generation result owner
    - plot / quality analysis owner
    - 终态 payload / SSE 兼容 helper
  - 但
    `chapter_single_generation_stream_workflow_service.rs`
    之前在终态收尾分支仍会：
    - 基于同一组 `result + analysis` 源分别重组 completion message
    - 再分别重组 quality metrics event / quality gate event
    - 再分别重组 terminal response payload
    - 再分别重组 analysis-started follow-up event
  - 这代表 single stream lane 在 Phase 5 上仍保留一条
    Python-era 旧 hop：
    owner 已经 materialize，但 workflow 邻层仍保留一条 terminal source ->
    completion / event / payload contract 的本地 reopen / projection 支路

  本轮已把这条 terminal completion contract 真正前移回 stream owner：
  - `backend-rs/src/services/chapter_single_generation_stream_workflow_service.rs`
    中新增
    `SingleGenerationStreamCompletionProjection`
  - 这个 owner 现在统一投影：
    - completion message
    - quality metrics event
    - quality gate event
    - terminal result payload
    - analysis-started follow-up event
  - `launch_owned_single_chapter_generation_stream(...)`
    现在先构造 completion owner，再统一消费 owner 发 complete /
    metrics event / gate event / result / analysis-started，
    不再在 workflow 邻层把同一组 terminal sources 分散重组

  这条 seam 的意义不只是“几个 helper 被包到 struct 里”。它真正回答的是：
  single stream lane 在 generation 完成后，终态 completion contract 到底由哪个
  Rust owner 提供。现在这条 duplicate 已被删掉，single stream lane 在
  terminal completion ownership 上又向前收了一层。

  对 Phase 5 的价值同样直接：
  - cutover 审计时更容易回答“single stream terminal completion 到底由哪个
    Rust owner 提供 contract”
  - completion message / follow-up SSE / compat result payload 现在共享更连续
    的 owner 链
  - fallback shrink / rollback / stronger smoke 在 single stream lane 上又少了
    一条隐藏在 workflow 邻层的 local terminal projection 支路

  这条 seam 已通过：
  - `cargo fmt --manifest-path "backend-rs/Cargo.toml"`
  - `cargo test stream_workflow --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-single-stream-completion-owner" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-single-stream-completion-owner"`

- 2026-06-05 同一条 `chapter_generation` Phase 5 lane 又沿
  single stream / success follow-up 邻链再收掉了一条
  “stream workflow 已经拿到 generated result、runtime user、launch intent，
  但 story runtime contract / follow-up analysis / latest history quality sync /
  terminal completion 仍在 workflow 邻层本地串联” 的 seam。当前：
  - `chapter_single_generation` stream 邻域已经有：
    - generated chapter result owner
    - stream story-runtime contract helper
    - follow-up analysis outcome owner
    - terminal completion projection owner
  - 但
    `chapter_single_generation_stream_workflow_service.rs`
    之前在成功收尾分支仍会：
    - 本地构造 story runtime contract
    - 本地执行 stream follow-up analysis
    - 在拿到 quality metrics 后本地回写 latest generated history quality
    - 然后再把这些结果交给 terminal completion owner
  - 这代表 single stream lane 在 Phase 5 上仍保留一条
    Python-era 旧 hop：
    success sources 已经 materialize，但 workflow 邻层仍保留一条
    generated result -> post-success follow-up contracts 的本地 orchestration
    支路

  本轮已把这条 success follow-up contract 真正前移回 stream owner：
  - `backend-rs/src/services/chapter_single_generation_stream_workflow_service.rs`
    中新增
    `SingleGenerationStreamSuccessFollowUpProjection`
  - 这个 owner 现在统一负责：
    - story runtime contract assembly
    - follow-up analysis
    - latest generated history quality sync
    - terminal completion projection
  - `launch_owned_single_chapter_generation_stream(...)`
    现在先构造 success follow-up owner，再统一消费 owner 发 complete /
    metrics event / gate event / result / analysis-started，
    不再在 workflow 邻层本地串联同一条 success follow-up 语义链

  这条 seam 的意义不只是“成功分支多了一个 struct”。它真正回答的是：
  single stream lane 在 generation success 之后，谁负责把生成结果推进到
  follow-up analysis、latest history quality、terminal completion 这一整条 Rust
  contract。现在这条 duplicate 已被删掉，single stream lane 在 success
  follow-up ownership 上又向前收了一层。

  对 Phase 5 的价值同样直接：
  - cutover 审计时更容易回答“single stream success follow-up 到底由哪个
    Rust owner 提供 contract”
  - generated result / analysis / history quality sync / completion output
    现在共享更连续的 owner 链
  - fallback shrink / rollback / stronger smoke 在 single stream lane 上又少了
    一条隐藏在 workflow 邻层的 local success orchestration 支路

  这条 seam 已通过：
  - `cargo fmt --manifest-path "backend-rs/Cargo.toml"`
  - `cargo test stream_workflow --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-single-stream-success-followup-owner" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-single-stream-success-followup-owner"`

- 2026-06-05 同一条 `chapter_generation` Phase 5 lane 又沿
  single stream / success emission 邻链再收掉了一条
  “stream workflow 明明已经拿到了 success follow-up owner / terminal completion
  owner，但最外层仍本地按顺序发 complete / quality events / result /
  analysis-started / done” 的 seam。当前：
  - `chapter_single_generation` stream 邻域已经有：
    - success follow-up analysis owner
    - terminal completion projection owner
    - compat result payload owner
    - quality metrics / quality gate / analysis-started 事件 payload owner
  - 但
    `chapter_single_generation_stream_workflow_service.rs`
    之前在成功收尾分支仍会：
    - 本地发 `complete`
    - 本地发 quality metrics event
    - 本地发 quality gate event
    - 本地发 terminal result payload
    - 本地发 analysis-started event
    - 本地发 `done`
    也就是仍在 workflow 邻层逐条重放同一组 completion sources
  - 这代表 single stream lane 在 Phase 5 上仍保留一条
    Python-era 旧 hop：
    completion owner 邻域已经足够完整，但 workflow 邻层仍保留一条 final
    ordered success emission 支路

  本轮已把这条 success emission contract 真正前移回 stream owner：
  - `backend-rs/src/services/chapter_single_generation_stream_workflow_service.rs`
    中新增
    `SingleGenerationStreamSuccessEmissionPlan`
  - 同时新增
    `SingleGenerationStreamSuccessEventPayload`
  - `SingleGenerationStreamCompletionProjection`
    现在直接暴露 `emission_plan()`
  - `SingleGenerationStreamSuccessFollowUpProjection`
    现在直接暴露 `emit(...)`
  - `launch_owned_single_chapter_generation_stream(...)`
    现在只消费 owner 投影出来的 ordered emission plan，
    统一发出：
    - `complete`
    - quality events
    - terminal result
    - analysis-started
    - `done`
    不再在 workflow 邻层本地逐条重组与发送

  这条 seam 的意义不只是“发事件的代码挪进 struct”。它真正回答的是：
  single stream lane 在 success follow-up 之后，到底由哪个 Rust owner 负责把
  terminal SSE completion sequence 作为一个连续 contract 提供出来。现在这条
  duplicate 已被删掉，single stream lane 在 success emission ownership 上又
  向前收了一层。

  对 Phase 5 的价值同样直接：
  - cutover 审计时更容易回答“single stream terminal SSE sequence 到底由哪个
    Rust owner 提供 contract”
  - completion message / quality events / compat result / analysis-started /
    done 现在共享更连续的 owner 链
  - fallback shrink / rollback / stronger smoke 在 single stream lane 上又少了
    一条隐藏在 workflow 邻层的 local ordered emission 支路

  这条 seam 已通过：
  - `cargo fmt --manifest-path "backend-rs/Cargo.toml"`
  - `cargo test stream_workflow --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-single-stream-emission-owner" -- --nocapture`
  - `cargo test should_project_single_generation_stream_success_emission_plan_owner_order --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-single-stream-emission-owner" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-single-stream-emission-owner"`

- 2026-06-05 同一条 `chapter_generation` Phase 5 lane 又沿
  single stream / prepare owner 邻链再收掉了一条
  “stream workflow 只需要 runtime launch input，但仍绕经 single background
  write workflow helper 才拿到 prepare owner 已经 materialize 的 launch
  contract” 的 seam。当前：
  - `chapter_single_generation` prepare / stream 邻域已经有：
    - request normalization owner
    - restored runtime launch owner
    - runtime launch input projection owner
  - 但
    `chapter_single_generation_stream_workflow_service.rs`
    之前仍通过
    `chapter_single_generation_write_workflow_service::prepare_owned_single_generation_runtime_launch_input(...)`
    来拿 stream lane 自己要消费的 runtime launch input
  - 而这条 helper 本身并不拥有 single background persistence 或 write
    policy，只是把 prepare owner 的
    `PreparedSingleChapterGenerationRestoredRuntimeLaunch::prepare(...).into_runtime_launch_input()`
    再转手一次
  - 这代表 single stream lane 在 Phase 5 上仍保留一条
    Python-era 旧 hop：
    prepare owner 已经 materialize，但 stream workflow 邻层仍保留一条
    stream -> write-workflow -> prepare owner 的跨 lane detour

  本轮已把这条 prepare-owner contract 真正前移回 prepare service：
  - `backend-rs/src/services/chapter_single_generation_prepare_service.rs`
    现在直接暴露
    `prepare_owned_single_generation_runtime_launch_input(...)`
  - `backend-rs/src/services/chapter_single_generation_stream_workflow_service.rs`
    现在直接消费 prepare owner，不再经由
    `chapter_single_generation_write_workflow_service.rs`
  - `chapter_single_generation_write_workflow_service.rs`
    中的同名 forwarding helper 已移除

  这条 seam 的意义不只是“少了一个 import”。它真正回答的是：
  single stream lane 的 runtime launch input 到底属于哪个 Rust owner 提供。
  现在这条 duplicate 已被删掉，single stream lane 在 prepare-owner
  ownership 上又向前收了一层。

  对 Phase 5 的价值同样直接：
  - cutover 审计时更容易回答“single stream launch input 到底由哪个 Rust
    owner 提供 contract”
  - request prepare / restored launch / stream runtime handoff 现在共享更连续
    的 owner 链
  - fallback shrink / rollback / stronger smoke 在 single stream lane 上又少了
    一条隐藏在 workflow 邻层的 cross-lane detour 支路

  这条 seam 已通过：
  - `cargo fmt --manifest-path "backend-rs/Cargo.toml"`
- `cargo test chapter_single_generation_prepare_service --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-single-stream-prepare-owner" -- --nocapture`
- `cargo test stream_workflow --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-single-stream-prepare-owner" -- --nocapture`
- `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-single-stream-prepare-owner"`

- 2026-06-05 同一条 `chapter_generation` Phase 5 lane 又沿
  single background / workflow entry 邻链再收掉了一条
  “background write workflow 明明已经同时拥有 existing-task compat payload owner
  与 prepared background launch owner，但最外层 workflow body 仍本地串
  `load target -> existing-task short-circuit -> prepare launch -> persist and
  dispatch`” 的 seam。当前：
  - `chapter_single_generation` single background 邻域已经有：
    - owned chapter target loader
    - existing background payload query owner
    - prepared background launch persistence/runtime owner
  - 但
    `chapter_single_generation_write_workflow_service.rs`
    之前最外层入口仍会：
    - 本地加载 chapter target
    - 本地判断是否已有后台任务
    - 若无则本地切到 prepared launch 分支
    - 然后再把 launch 交给 persistence / dispatch
  - 这代表 single background lane 在 Phase 5 上仍保留一条
    Python-era 旧 hop：
    reuse branch 与 launch branch 的最终决策明明都已具备 owner contract，
    但 workflow 邻层仍保留一条 outer orchestration 支路

  本轮已把这条 background workflow-entry contract 真正前移回 write owner：
  - `backend-rs/src/services/chapter_single_generation_write_workflow_service.rs`
    中新增 `SingleGenerationBackgroundWorkflowEntry`
  - 这个 owner 现在统一负责：
    - target lookup
    - existing-task payload reuse
    - prepared background launch selection
    - branch-aware terminal handoff to persist / dispatch
  - `start_owned_single_generation_background_write_workflow(...)`
    现在只消费这个 workflow-entry owner，不再在 outer workflow body 本地保留
    reuse-vs-launch 编排支路

  这条 seam 的意义不只是“入口函数少了几行”。它真正回答的是：
  single background lane 在进入后台写工作流时，到底由哪个 Rust owner 决定
  “复用已有任务”还是“启动新的 launch contract”。现在这条 duplicate 已被删掉，
  single background lane 在 workflow-entry ownership 上又向前收了一层。

  对 Phase 5 的价值同样直接：
  - cutover 审计时更容易回答“single background workflow entry 到底由哪个 Rust
    owner 提供 contract”
  - target access / existing-task reuse / launch preparation / persistence handoff
    现在共享更连续的 owner 链
  - fallback shrink / rollback / stronger smoke 在 single background lane 上又少了
    一条隐藏在 workflow 邻层的 local branch orchestration 支路

  这条 seam 已通过：
- `cargo fmt --manifest-path "backend-rs/Cargo.toml"`
- `cargo test chapter_single_generation_write_workflow_service --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-single-background-workflow-owner" -- --nocapture`
- `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-single-background-workflow-owner"`

- 2026-06-05 同一条 `chapter_generation` Phase 5 lane 又沿
  single background / workflow start 邻链再收掉了一条
  “workflow entry owner 明明已经决定了 existing-task reuse 还是 prepared launch，
  但最外层 write workflow 仍本地串
  `prepare(...) -> persist_and_dispatch(...)`” 的 seam。当前：
  - `chapter_single_generation` single background 邻域已经有：
    - owned chapter target loader
    - existing background payload query owner
    - prepared background launch persistence/runtime owner
    - workflow-entry branch owner
  - 但
    `start_owned_single_generation_background_write_workflow(...)`
    之前最外层入口仍会：
    - 先调用 `SingleGenerationBackgroundWorkflowEntry::prepare(...)`
    - 再把拿到的 branch contract 继续本地交给
      `persist_and_dispatch(...)`
  - 这代表 single background lane 在 Phase 5 上仍保留一条
    Python-era 旧 hop：
    workflow-entry owner 已经 materialize，但 write workflow 邻层仍保留一条
    final branch-handoff orchestration 支路

  本轮已把这条 background workflow-start contract 真正前移回 write owner：
  - `backend-rs/src/services/chapter_single_generation_write_workflow_service.rs`
    中新增 `PreparedSingleGenerationBackgroundWorkflowStart`
  - 这个 owner 现在统一负责：
    - workflow-entry preparation
    - terminal persist-and-dispatch handoff
  - `start_owned_single_generation_background_write_workflow(...)`
    现在只消费这个 workflow-start owner，不再在 outer workflow body 本地保留
    `prepare -> persist_and_dispatch` 这条最终支路

  这条 seam 的意义不只是“入口函数又短了一点”。它真正回答的是：
  single background lane 在 workflow-entry owner 已经决定 reuse-vs-launch 之后，
  到底由哪个 Rust owner 负责把这条分支 contract 推进到最终 persistence 与
  runtime dispatch。现在这条 duplicate 已被删掉，single background lane 在
  workflow-start ownership 上又向前收了一层。

  对 Phase 5 的价值同样直接：
  - cutover 审计时更容易回答“single background workflow start 到底由哪个 Rust
    owner 提供 contract”
  - target lookup / existing-task reuse / launch preparation / persistence /
    runtime dispatch 现在共享更连续的 owner 链
  - fallback shrink / rollback / stronger smoke 在 single background lane 上又少了
    一条隐藏在 workflow 邻层的 local final-handoff orchestration 支路

  这条 seam 已通过：
  - `cargo fmt --manifest-path "backend-rs/Cargo.toml"`
  - `cargo test should_keep_single_generation_background_workflow_start_existing_payload_owner_contract --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-single-background-workflow-start-owner" -- --nocapture`
  - `cargo test should_keep_single_generation_background_workflow_start_launch_owner_contract --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-single-background-workflow-start-owner" -- --nocapture`
  - `cargo test chapter_single_generation_write_workflow_service --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-single-background-workflow-start-owner" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-single-background-workflow-start-owner"`

- 2026-06-05 同一条 `chapter_generation` Phase 5 lane 又沿
  single runtime / lifecycle owner 邻链再收掉了一条
  “single runtime lane 明明已经同时拥有 preparing persistence owner、
  generation execute owner、terminal persistence routing owner，但 runtime
  driver 仍本地串 `prepare -> execute -> persist terminal result`” 的 seam。当前：
  - `chapter_single_generation` runtime 邻域已经有：
    - preparing-stage persistence owner
    - single runtime execution owner
    - terminal persistence routing owner
  - 但
    `chapter_single_generation_runtime_state_service.rs`
    之前仍由 `SingleGenerationRuntimeDriver` 最外层直接：
    - 落 preparing persistence
    - 执行 generation runtime
    - 再根据结果落 terminal persistence
  - 这代表 single runtime lane 在 Phase 5 上仍保留一条
    Python-era 旧 hop：
    生命周期三段语义明明都已有 owner contract，但 driver 邻层仍保留一条
    outer lifecycle orchestration 支路

  本轮已把这条 runtime lifecycle contract 真正前移回 runtime owner：
  - `backend-rs/src/services/chapter_single_generation_runtime_state_service.rs`
    中新增 `SingleGenerationRuntimeLifecyclePlan`
  - 这个 owner 现在统一负责：
    - preparing persistence
    - generation execution
    - terminal persistence routing
  - `SingleGenerationRuntimeDriver`
    现在只持有 lifecycle owner 并委托执行，不再在自身本地保留整条
    prepare/execute/persist 生命周期支路

  这条 seam 的意义不只是“driver 少了几行”。它真正回答的是：
  single runtime lane 在运行时启动后，到底由哪个 Rust owner 负责把准备态、
  generation 执行和 terminal persistence 串成一条完整 contract。现在这条
  duplicate 已被删掉，single runtime lane 在 lifecycle ownership 上又向前收了一层。

  对 Phase 5 的价值同样直接：
  - cutover 审计时更容易回答“single runtime lifecycle 到底由哪个 Rust owner
    提供 contract”
  - preparing persistence / runtime execute / terminal persistence
    现在共享更连续的 owner 链
  - fallback shrink / rollback / stronger smoke 在 single runtime lane 上又少了
    一条隐藏在 driver 邻层的 local lifecycle orchestration 支路

  这条 seam 已通过：
  - `cargo fmt --manifest-path "backend-rs/Cargo.toml"`
  - `cargo test chapter_single_generation_runtime_state_service --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-single-runtime-lifecycle-owner" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-single-runtime-lifecycle-owner"`

- 2026-06-05 同一条 `chapter_generation` Phase 5 lane 又回到
  batch resume / command restore 邻链再收掉了一条
  “persisted runtime restore、manual-review blocker、workflow runtime handoff
  明明都已经可以由一条 owner 链 materialize，但 resume command 邻层仍本地
  重放这些 launch-source prechecks” 的 seam。当前：
  - `chapter_batch_generation` batch resume 邻域已经有：
    - invalid-status gate owner
    - persisted runtime-context restore owner
    - manual-review blocker owner
    - workflow-runtime-state handoff owner
    - validated execution / launch-persistence owner
  - 但
    `prepare_batch_generation_resume(...)`
    之前仍会：
    - 在 command 层本地包一层 restored launch-source wrapper
    - 再把同一批 restored sources 继续交给 launch-persistence prepare
  - 这代表 batch resume command lane 在 Phase 5 上仍保留一条
    Python-era 旧 hop：
    restored-source owner 邻域已经足够完整，但 command 邻层仍保留一条
    local launch-source replay 支路

  本轮已把这条 batch resume launch-sources contract 真正前移回 command owner：
  - `backend-rs/src/services/chapter_batch_generation_resume_task_command_service.rs`
    中新增 `PreparedBatchGenerationResumeLaunchSources`
  - 这个 owner 现在统一负责：
    - invalid-status validation
    - persisted runtime-context restore
    - manual-review blocker gate
    - workflow-runtime-state handoff
  - `prepare_batch_generation_resume(...)`
    现在直接消费这个 launch-sources owner，再进入 validated execution 与
    launch-persistence 准备，不再在 outer command body 本地重放同一批 restore
    语义

  这条 seam 的意义不只是“多了一个 prepare struct”。它真正回答的是：
  batch resume lane 在进入 validated execution 之前，到底由哪个 Rust owner
  负责把 restored runtime sources 稳定地交付给下游 launch-persistence 链。
  现在这条 duplicate 已被删掉，batch resume lane 在 launch-sources ownership
  上又向前收了一层。

  对 Phase 5 的价值同样直接：
  - cutover 审计时更容易回答“batch resume launch sources 到底由哪个 Rust owner
    提供 contract”
  - status validation / persisted-runtime restore / manual-review blocker /
    workflow-runtime handoff / launch preparation 现在共享更连续的 owner 链
  - fallback shrink / rollback / stronger smoke 在 batch resume lane 上又少了
    一条隐藏在 command 邻层的 local restored-source replay 支路

  这条 seam 已通过：
  - `cargo fmt --manifest-path "backend-rs/Cargo.toml"`
  - `cargo test should_keep_resume_launch_sources_owner_contract --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-resume-launch-sources-owner" -- --nocapture`
  - `cargo test resume_task_command_service --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-resume-launch-sources-owner" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-resume-launch-sources-owner"`

- 2026-06-05 同一条 `chapter_generation` Phase 5 lane 又回到
  batch resume / write workflow 邻链再收掉了一条
  “resume command owner 明明已经 materialize 出 launch persistence plan，
  但 write workflow 最外层仍本地串
  `prepare_owned_batch_generation_resume(...) -> persist_and_dispatch(...)`”
  的 seam。当前：
  - `chapter_batch_generation` batch resume 邻域已经有：
    - owned task + snapshot loading owner
    - validated execution / restored runtime owner
    - reset persistence + dispatch-ready launch plan owner
  - 但
    `chapter_batch_generation_write_workflow_service.rs`
    之前最外层入口仍会：
    - 先调用 `prepare_owned_batch_generation_resume(...)`
    - 再把拿到的 persistence plan 继续本地交给
      `persist_and_dispatch(...)`
  - 这代表 batch resume write lane 在 Phase 5 上仍保留一条
    Python-era 旧 hop：
    launch persistence owner 已经 materialize，但 write workflow 邻层仍保留
    一条 final prepare-vs-terminal-handoff 支路

  本轮已把这条 batch resume workflow-launch contract 真正前移回 write owner：
  - `backend-rs/src/services/chapter_batch_generation_write_workflow_service.rs`
    中新增 `PreparedBatchGenerationResumeWorkflowLaunch`
  - 这个 owner 现在统一负责：
    - owned resume preparation
    - terminal persist-and-dispatch handoff
  - `resume_owned_batch_generation_write_workflow(...)`
    现在只消费这个 workflow-launch owner，不再在 outer workflow body 本地保留
    `prepare -> persist_and_dispatch` 这条最终支路
  - 为了直接锁住这条 write-workflow owner contract，本轮还在
    `chapter_batch_generation_resume_task_command_service.rs`
    增加了一个仅测试可见的 plan 构造入口，不影响生产 API

  这条 seam 的意义不只是“入口函数少了几行”。它真正回答的是：
  batch resume lane 在进入 write workflow 之后，到底由哪个 Rust owner 负责把
  “已准备好的 resume launch persistence plan”推进到最终 reset persistence
  与 runtime dispatch。现在这条 duplicate 已被删掉，batch resume lane 在
  workflow-launch ownership 上又向前收了一层。

  对 Phase 5 的价值同样直接：
  - cutover 审计时更容易回答“batch resume workflow launch 到底由哪个 Rust
    owner 提供 contract”
  - owned resume loading / validated launch preparation / reset persistence /
    runtime dispatch 现在共享更连续的 owner 链
  - fallback shrink / rollback / stronger smoke 在 batch resume lane 上又少了
    一条隐藏在 write workflow 邻层的 local final-handoff orchestration 支路

  这条 seam 已通过：
  - `cargo fmt --manifest-path "backend-rs/Cargo.toml"`
  - `cargo test should_keep_batch_generation_resume_workflow_launch_owner_contract --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-resume-workflow-launch-owner" -- --nocapture`
- `cargo test should_keep_batch_generation_resume_workflow_launch_persistence_owner_contract --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-resume-workflow-launch-owner" -- --nocapture`
- `cargo test resume --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-resume-workflow-launch-owner" -- --nocapture`
- `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-resume-workflow-launch-owner"`

- 2026-06-05 同一条 `chapter_generation` Phase 5 lane 又沿
  batch create / write workflow 邻链再收掉了一条
  “project access 已经在外层完成，但 create write workflow 最外层仍本地串
  `prepare(...) -> persist_and_dispatch(...)`” 的 seam。当前：
  - `chapter_batch_generation` batch create 邻域已经有：
    - owned project access validation owner
    - create launch preparation owner
    - persistence + dispatch handoff owner
  - 但
    `chapter_batch_generation_write_workflow_service.rs`
    之前在 access check 之后最外层入口仍会：
    - 先本地调用 create workflow preparation
    - 再把拿到的 plan 继续本地交给
      `persist_and_dispatch(...)`
  - 这代表 batch create write lane 在 Phase 5 上仍保留一条
    Python-era 旧 hop：
    create owner 已经足够完整，但 write workflow 邻层仍保留一条 final
    create-entry orchestration 支路

  本轮已把这条 batch create workflow-entry contract 真正前移回 write owner：
  - `backend-rs/src/services/chapter_batch_generation_write_workflow_service.rs`
    中新增 `PreparedBatchGenerationCreateWorkflowEntry`
  - 这个 owner 现在统一负责：
    - create workflow preparation
    - terminal persist-and-dispatch handoff
  - `start_owned_batch_generation_write_workflow(...)`
    现在在 access check 之后只消费这个 workflow-entry owner，不再在 outer
    workflow body 本地保留 `prepare -> persist_and_dispatch` 这条最终支路
  - 同时外层仍保留 `ensure_owned_project_access(...)`，因此 access boundary
    没有被糊进一个过大的 helper，owner 图反而更清晰

  这条 seam 的意义不只是“入口函数又短了一点”。它真正回答的是：
  batch create lane 在进入 write workflow 且完成 access check 之后，到底由哪个
  Rust owner 负责把“已准备好的 create launch persistence contract”推进到最终
  persistence 与 runtime dispatch。现在这条 duplicate 已被删掉，batch create
  lane 在 workflow-entry ownership 上又向前收了一层。

  对 Phase 5 的价值同样直接：
  - cutover 审计时更容易回答“batch create workflow entry 到底由哪个 Rust
    owner 提供 contract”
  - access check / create launch preparation / persistence / runtime dispatch
    现在共享更连续的 owner 链
  - fallback shrink / rollback / stronger smoke 在 batch create lane 上又少了
    一条隐藏在 write workflow 邻层的 local final-handoff orchestration 支路

  这条 seam 已通过：
  - `cargo fmt --manifest-path "backend-rs/Cargo.toml"`
  - `cargo test should_keep_batch_generation_create_workflow_entry_owner_contract --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-create-workflow-entry-owner" -- --nocapture`
- `cargo test should_keep_batch_generation_create_workflow_entry_persistence_owner_contract --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-create-workflow-entry-owner" -- --nocapture`
- `cargo test chapter_batch_generation_write_workflow_service --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-create-workflow-entry-owner" -- --nocapture`
- `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-create-workflow-entry-owner"`

- 2026-06-05 同一条 `chapter_generation` Phase 5 lane 又沿
  batch create / batch resume 的 write entry 邻链继续收掉了一层
  “workflow-entry / workflow-launch owner 明明已经 materialize，但 outer
  write entry 仍本地串 `prepare(...) -> persist_and_dispatch(...)`” 的 seam。
  当前：
  - `chapter_batch_generation` batch write 邻域已经有：
    - create access-check 之后的 workflow-entry owner
    - resume prepare 之后的 workflow-launch owner
    - final persistence + runtime dispatch handoff owner
  - 但
    `chapter_batch_generation_write_workflow_service.rs`
    之前两个最外层入口仍会：
    - 先 materialize create workflow-entry 或 resume workflow-launch owner
    - 再把同一份 branch contract 继续本地交给
      `persist_and_dispatch(...)`
  - 这代表 batch write lane 在 Phase 5 上仍保留一条
    Python-era 旧 hop：
    branch owner 已经存在，但 outer write entry 邻层仍保留一条 local final
    handoff orchestration 支路

  本轮已把这条 batch write workflow-start contract 真正前移回 write owner：
  - `backend-rs/src/services/chapter_batch_generation_write_workflow_service.rs`
    中新增：
    - `PreparedBatchGenerationCreateWorkflowStart`
    - `PreparedBatchGenerationResumeWorkflowStart`
  - 这两个 owner 现在统一负责：
    - create/resume branch preparation
    - final persist-and-dispatch handoff
  - `start_owned_batch_generation_write_workflow(...)` 与
    `resume_owned_batch_generation_write_workflow(...)`
    现在都只消费 workflow-start owner，不再在 outer entry body 本地保留
    `prepare -> persist_and_dispatch` 这条最终支路
  - create 外层的 `ensure_owned_project_access(...)` 仍然显式保留，因此 access
    boundary 没有被糊进过大的 helper，owner 图反而更清晰

  这条 seam 的意义不只是“入口函数又短了一点”。它真正回答的是：
  batch write lane 在 create/resume branch owner 已经存在之后，到底由哪个 Rust
  owner 负责把这条 branch contract 推进到最终 persistence 与 runtime dispatch。
  现在这条 duplicate 已被删掉，batch create / batch resume 两条 write lane
  在 workflow-start ownership 上同时又向前收了一层。

  对 Phase 5 的价值同样直接：
  - cutover 审计时更容易回答“batch write workflow start 到底由哪个 Rust
    owner 提供 contract”
  - access check / create prepare / resume prepare / persistence / runtime
    dispatch 现在共享更连续的 owner 链
  - fallback shrink / rollback / stronger smoke 在 batch write lane 上又少了
    一条隐藏在 outer entry 邻层的 local final-handoff orchestration 支路

  这条 seam 已通过：
  - `cargo fmt --manifest-path "backend-rs/Cargo.toml"`
  - `cargo test should_keep_batch_generation_create_workflow_start_owner_contract --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-workflow-start-owner" -- --nocapture`
  - `cargo test should_keep_batch_generation_resume_workflow_start_owner_contract --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-workflow-start-owner" -- --nocapture`
  - `cargo test chapter_batch_generation_write_workflow_service --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-workflow-start-owner" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-workflow-start-owner"`

- 2026-06-05 同一条 `chapter_generation` Phase 5 lane 又沿
  batch cancel 邻链再收掉了一条
  “cancel 外层入口仍本地串
  `load owned task -> reject terminal status -> load snapshot -> persist`”
  的 seam。当前：
  - `chapter_batch_generation` batch cancel 邻域已经有：
    - owned task loading owner
    - cancelled persistence owner
    - cancelled status payload / summary owner
  - 但
    `chapter_batch_generation_cancel_service.rs`
    之前最外层入口仍会：
    - 先本地加载 owned task
    - 再本地做 terminal status gate
    - 再本地加载 snapshot
    - 最后才把结果交给 cancel persistence
  - 这代表 batch cancel lane 在 Phase 5 上仍保留一条
    Python-era 旧 hop：
    cancel owner 邻域已经足够完整，但 outer cancel workflow 仍保留一条
    final terminal orchestration 支路

  本轮已把这条 batch cancel workflow contract 真正前移回 cancel owner：
  - `backend-rs/src/services/chapter_batch_generation_cancel_service.rs`
    中新增 `PreparedBatchGenerationCancelWorkflow`
  - 这个 owner 现在统一负责：
    - owned task loading / task-not-found mapping
    - cancellable status validation
    - snapshot loading
    - cancel persistence handoff
  - `cancel_owned_batch_generation_task(...)`
    现在只消费这个 cancel workflow owner，不再在 outer function body 本地保留
    `load -> validate -> snapshot -> persist` 这条最终支路
  - 为了直接锁住这条 cancel owner contract，本轮还在
    `chapter_batch_generation_runtime_state_service.rs`
    增加了一个仅测试可见的 cancelled response payload 读口，不影响生产 API

  这条 seam 的意义不只是“cancel 函数短了一点”。它真正回答的是：
  batch cancel lane 在进入 cancel workflow 之后，到底由哪个 Rust owner 负责把
  “已加载的 owned task + snapshot sources”推进到最终 cancelled persistence
  与 response payload。现在这条 duplicate 已被删掉，batch cancel lane 在
  workflow ownership 上又向前收了一层。

  对 Phase 5 的价值同样直接：
  - cutover 审计时更容易回答“batch cancel workflow 到底由哪个 Rust owner
    提供 contract”
  - owned task loading / cancel validation / snapshot restore input /
    cancelled persistence / response payload 现在共享更连续的 owner 链
  - fallback shrink / rollback / stronger smoke 在 batch cancel lane 上又少了
    一条隐藏在 cancel 邻层的 local terminal orchestration 支路

  这条 seam 已通过：
- `cargo fmt --manifest-path "backend-rs/Cargo.toml"`
- `cargo test cancel --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-cancel-workflow-owner" -- --nocapture`
- `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-cancel-workflow-owner"`

- 2026-06-05 同一条 `chapter_generation` Phase 5 lane 又沿
  batch cancel write 邻链完成了一次真正的整文件收口：
  `chapter_batch_generation_cancel_service.rs`
  已不再保留为独立生产模块。当前：
  - `chapter_batch_generation` batch cancel 邻域已经有：
    - shared owned task + snapshot sources owner
    - terminal status gating
    - cancelled persistence-plan preparation
    - cancel workflow launch / final write-workflow start
  - 并且 cancel 已经先前并入 shared batch write-workflow lane
  - 但在这次收口前，仍保留一个相邻文件：
    `chapter_batch_generation_cancel_service.rs`
    来再次串接：
    - validate cancellable status
    - prepare cancelled persistence plan
    - hand off to cancel workflow launch
  - 这代表 batch cancel lane 在 Phase 5 上仍保留一条
    “owner 已在 write lane 邻域齐备，但模块边界还没真正收口”的
    compatibility file seam

  本轮已把这条 file seam 真正收回
  `chapter_batch_generation_write_workflow_service.rs`：
  - cancel service 生产依赖已从 `services/mod.rs` 删除
  - `chapter_batch_generation_cancel_service.rs` 已删除
  - write-workflow owner 现在直接持有：
    - `prepare_cancel_batch_generation_persistence_plan_from_owned_sources(...)`
    - `prepare_owned_batch_generation_cancel_workflow(...)`
    - `PreparedBatchGenerationCancelWorkflowLaunch::prepare(...)`
  - 新增 focused tests 锁住这条 contract：
    - `should_keep_cancel_batch_generation_prepare_owner_contract`
    - `should_reject_terminal_status_inside_cancel_prepare_owner`

  这条 seam 的意义不只是“又少了一个文件”。它真正回答的是：
  batch cancel 在已经进入 batch write-workflow lane 之后，到底还要不要再保留
  一个 Python-era 兼容文件边界来中转同一条 owner chain。现在答案已经变成
  不需要，cancel lane 在文件级 owner 上也收到了更连续的 Rust 边界：

  - `public cancel start -> owned cancel prepare -> cancelled persistence`

  对 Phase 5 的价值同样直接：
  - cutover 审计时更容易回答“batch cancel 最终生产 owner 到底在哪个 Rust
    文件里闭合”
  - shared owned sources / terminal gating / cancelled persistence / workflow
    launch 现在留在同一条 write-lane file-local owner chain
  - fallback shrink / rollback / stronger smoke 在 batch cancel lane 上又少了
    一个死掉的 compatibility module seam

  这条 seam 已通过：
  - `rustfmt --edition 2021 "backend-rs/src/services/chapter_batch_generation_write_workflow_service.rs"`
  - `cargo test chapter_batch_generation_write_workflow_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-cancel-file-collapse" -- --nocapture`
  - `cargo test chapter_batch_generation --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-cancel-file-collapse" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-cancel-file-collapse"`

- 2026-06-05 同一条 `chapter_generation` Phase 5 lane 又沿
  batch status-stream 邻链完成了一次真正的整文件收口：
  `chapter_batch_generation_stream_state_query_service.rs`
  已不再保留为独立生产模块。当前：
  - `chapter_batch_generation` batch status-stream 邻域已经有：
    - shared owned task read-state owner
    - stream-state semantics projection
    - poll / cursor advance
    - SSE event emission / close behavior
  - 并且 status/stream 的 read-state projection 先前已经并入同一条 owner 链
  - 但在这次收口前，仍保留一个相邻文件：
    `chapter_batch_generation_stream_state_query_service.rs`
    来再次串接：
    - task + snapshot -> stream state projection
    - owned read-state -> stream state projection
  - 这代表 batch status-stream lane 在 Phase 5 上仍保留一条
    “owner 已在 stream lane 邻域齐备，但模块边界还没真正收口”的
    compatibility file seam

  本轮已把这条 file seam 真正收回
  `chapter_batch_generation_status_stream_service.rs`：
  - stream-state query 生产依赖已从 `services/mod.rs` 删除
  - `chapter_batch_generation_stream_state_query_service.rs` 已删除
  - status-stream owner 现在直接持有：
    - `build_batch_generation_stream_state_from_task_and_snapshot(...)`
    - `load_owned_batch_generation_stream_state(...)`
  - 新增 focused tests 锁住这条 contract：
    - `should_build_stream_state_from_task_and_snapshot_owner_inside_status_stream_service`
    - `should_build_terminal_stream_state_from_task_and_snapshot_owner_inside_status_stream_service`
    - `should_build_stream_state_from_shared_owned_read_state_owner_inside_status_stream_service`

  这条 seam 的意义不只是“又少了一个文件”。它真正回答的是：
  batch status-stream 在已经进入 shared stream owner lane 之后，到底还要不要再
  保留一个 Python-era 兼容文件边界来中转同一条 stream projection chain。
  现在答案已经变成不需要，status-stream lane 在文件级 owner 上也收到了更连续
  的 Rust 边界：

  - `owned read-state -> stream state -> poll / emit`

  对 Phase 5 的价值同样直接：
  - cutover 审计时更容易回答“batch status-stream 最终生产 owner 到底在哪个
    Rust 文件里闭合”
  - shared owned read-state / stream-state projection / poll / SSE emission
    现在留在同一条 stream file-local owner chain
  - fallback shrink / rollback / stronger smoke 在 batch status-stream lane 上
    又少了一个死掉的 compatibility module seam

  这条 seam 已通过：
  - `rustfmt --edition 2021 "backend-rs/src/services/chapter_batch_generation_status_stream_service.rs"`
  - `cargo test chapter_batch_generation_status_stream_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-stream-state-file-collapse" -- --nocapture`
  - `cargo test chapter_batch_generation --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-stream-state-file-collapse" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-stream-state-file-collapse"`

- 2026-06-06 同一条 `chapter_generation` Phase 5 lane 又沿
  batch status query 邻链完成了一次真正的整文件收口：
  `chapter_batch_generation_status_task_query_service.rs`
  已不再保留为独立生产模块。当前：
  - `chapter_batch_generation` batch status query 邻域已经有：
    - shared owned task read-state owner
    - task + snapshot -> quality-context materialization
    - final status payload projection
  - 并且 owned read-state 与 status payload contract 先前已经并入同一条
    read-side owner 链
  - 但在这次收口前，仍保留一个相邻文件：
    `chapter_batch_generation_status_task_query_service.rs`
    来再次串接：
    - owned read-state -> status payload projection
    - task + snapshot -> status payload projection
  - 这代表 batch status query lane 在 Phase 5 上仍保留一条
    “owner 已在 read/query 邻域齐备，但模块边界还没真正收口”的
    compatibility file seam

  本轮已把这条 file seam 真正收回
  `chapter_batch_generation_read_context_service.rs`：
  - status-task query 生产依赖已从 `services/mod.rs` 删除
  - `chapter_batch_generation_status_task_query_service.rs` 已删除
  - read-context owner 现在直接持有：
    - `build_batch_generation_status_task_payload_with_quality_context(...)`
    - `build_batch_generation_status_task_payload_from_task_and_snapshot_projection(...)`
    - `load_owned_batch_generation_status_payload(...)`
  - 新增 focused tests 锁住这条 contract：
    - `should_build_status_task_payload_from_task_and_snapshot_projection_owner_inside_read_context_service`
    - `should_build_status_task_payload_from_quality_context_owner_inside_read_context_service`
    - `should_keep_owned_status_payload_loader_error_contract_inside_read_context_service`
    - `should_keep_owned_status_payload_read_state_projection_contract_inside_read_context_service`

  这条 seam 的意义不只是“又少了一个文件”。它真正回答的是：
  batch status query 在已经进入 shared read-side owner lane 之后，到底还要不要再
  保留一个 Python-era 兼容文件边界来中转同一条 status payload chain。
  现在答案已经变成不需要，status query lane 在文件级 owner 上也收到了更连续
  的 Rust 边界：

  - `owned read-state -> status payload`

  对 Phase 5 的价值同样直接：
  - cutover 审计时更容易回答“batch status query 最终生产 owner 到底在哪个
    Rust 文件里闭合”
  - shared owned read-state / quality-context materialization / status payload
    projection 现在留在同一条 read-side file-local owner chain
  - fallback shrink / rollback / stronger smoke 在 batch status query lane 上
    又少了一个死掉的 compatibility module seam

  这条 seam 已通过：
  - `rustfmt --edition 2021 "backend-rs/src/services/chapter_batch_generation_read_context_service.rs" "backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs" "backend-rs/src/services/chapter_batch_generation_task_view_query_service.rs" "backend-rs/src/api/chapter_batch_generation.rs"`
  - `cargo test chapter_batch_generation_read_context_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-status-file-collapse" -- --nocapture`
  - `cargo test chapter_batch_generation --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-status-file-collapse" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-status-file-collapse"`

- 2026-06-05 同一条 `chapter_generation` Phase 5 lane 又回到
  batch read/query 邻链再收掉了一条
  “query service 明明已经拿到了 active task set + read-context owner，
  但最外层仍本地做 `map / next / find` 来投影 payload” 的 seam。当前：
  - `chapter_batch_generation` batch read/query 邻域已经有：
    - active task row selection owner
    - snapshot-backed read-context projection owner
    - payload variant owner：
      - active task list item
      - active project task
      - single-generation existing background task
  - 但
    `chapter_batch_generation_task_view_query_service.rs`
    之前仍会：
    - 先加载 active `BatchGenerationReadContext` 列表
    - 再本地 `map(...)` 成 active-list payload
    - 再本地 `next()` 成 active-project payload
    - 再本地 `find(...)` 成 existing-background payload
  - 这代表 batch read/query lane 在 Phase 5 上仍保留一条
    Python-era 旧 hop：
    read-context owner 邻域已经足够完整，但 query 邻层仍保留一条 final
    payload projection 支路

  本轮已把这条 batch task-view payload contract 真正前移回 read-context owner：
  - `backend-rs/src/services/chapter_batch_generation_read_context_service.rs`
    现在直接拥有：
    - active-list payload projection
    - active-project first-item payload projection
    - existing-background payload projection
  - `backend-rs/src/services/chapter_batch_generation_task_view_query_service.rs`
    现在只保留：
    - query request 验证
    - active task 选择
    - project access boundary
    不再在 outer query body 本地保留 `map / next / find` 这条最终 payload
    支路
  - 同时章节匹配逻辑也回收到 read-context owner 邻域，不再在 query 层重复
    一份 chapter-id lookup contract

  这条 seam 的意义不只是“query service 少了几行投影代码”。它真正回答的是：
  batch read/query lane 在拿到 active task set 之后，到底由哪个 Rust owner 负责把
  “snapshot-backed read-context set”推进到最终 compat payload。现在这条 duplicate
  已被删掉，batch read/query lane 在 payload ownership 上又向前收了一层。

  对 Phase 5 的价值同样直接：
  - cutover 审计时更容易回答“active list / active project / existing background
    payload 到底由哪个 Rust owner 提供 contract”
  - active task selection / snapshot-backed read-context projection /
    compat payload materialization 现在共享更连续的 owner 链
  - fallback shrink / rollback / stronger smoke 在 batch read/query lane 上又少了
    一条隐藏在 query 邻层的 local payload projection 支路

  这条 seam 已通过：
  - `cargo test task_view_query --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-task-view-owner" -- --nocapture`
  - `cargo test read_context --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-task-view-owner" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-task-view-owner"`

- 2026-06-05 同一条 `chapter_generation` Phase 5 lane 又沿
  batch status query 邻链再收掉了一条
  “owned task query 明明已经拿到了 recovered task + snapshot sources，
  但 outer query 仍本地把它们投影成最终 status payload” 的 seam。当前：
  - `chapter_batch_generation` status query 邻域已经有：
    - owned task loading owner
    - timeout auto-recovery owner
    - snapshot loading owner
    - status payload projection owner
  - 但
    `chapter_batch_generation_status_task_query_service.rs`
    之前仍会：
    - 先加载 owned task
    - 再执行 timeout recovery
    - 再加载 snapshot
    - 最后在 outer query body 本地投影 compat status payload
  - 这代表 batch status query lane 在 Phase 5 上仍保留一条
    Python-era 旧 hop：
    query owner 邻域已经足够完整，但 outer status query 仍保留一条 final
    payload projection 支路

  本轮已把这条 batch status payload query contract 真正前移回 owned query owner：
  - `backend-rs/src/services/chapter_batch_generation_status_task_query_service.rs`
    中新增 `PreparedOwnedBatchGenerationStatusPayloadQuery`
  - 这个 owner 现在统一负责：
    - owned task loading / task-not-found mapping
    - timeout recovery
    - snapshot loading
    - final compat status payload projection
  - `load_owned_batch_generation_status_payload(...)`
    现在只消费这个 owned query owner，不再在 outer function body 本地保留
    `load -> recover -> snapshot -> payload` 这条最终支路
  - 同时新增 focused owner test
    `should_keep_owned_status_payload_query_owner_contract`
    直接锁住这条 owner contract

  这条 seam 的意义不只是“status query 函数短了一点”。它真正回答的是：
  batch status query lane 在进入 owned query path 之后，到底由哪个 Rust owner
  负责把“recovered owned task + snapshot sources”推进到最终 compat status
  payload。现在这条 duplicate 已被删掉，batch status query lane 在 query
  ownership 上又向前收了一层。

  对 Phase 5 的价值同样直接：
  - cutover 审计时更容易回答“owned status payload 到底由哪个 Rust owner
    提供 contract”
  - owned task loading / timeout recovery / snapshot restore / final status
    payload projection 现在共享更连续的 owner 链
  - fallback shrink / rollback / stronger smoke 在 batch status query lane 上
    又少了一条隐藏在 query 邻层的 local payload projection 支路

  这条 seam 已通过：
  - `cargo fmt --manifest-path "backend-rs/Cargo.toml"`
  - `cargo test status_task_query --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-status-payload-query-owner" -- --nocapture`
  - `cargo test should_keep_owned_status_payload_query_owner_contract --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-status-payload-query-owner" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-status-payload-query-owner"`

- 2026-06-05 同一条 `chapter_generation` Phase 5 lane 又沿
  batch active-query 邻链再收掉了一条
  “selected active-task payload 明明已经在 owner 链上，outer query 仍本地把它们
  包成最终 compat envelope” 的 seam。当前：
  - `chapter_batch_generation` active query 邻域已经有：
    - active task row selection owner
    - snapshot-backed read-context projection owner
    - active-list / active-project / existing-background payload projection owner
  - 但
    `chapter_batch_generation_task_view_query_service.rs`
    之前仍会：
    - 先选择 active task rows
    - 再向邻层 owner 取回最终 payload items
    - 最后在 outer query body 本地包出：
      - `{total, items}`
      - `{has_active_task, task}`
      - `Option<Value>` existing-background payload
  - 这代表 batch active-query lane 在 Phase 5 上仍保留一条
    Python-era 旧 hop：
    payload owner 邻域已经足够完整，但 active query 邻层仍保留一条 final
    response-envelope projection 支路

  本轮已把这条 batch active-query envelope contract 真正前移回 query owner：
  - `backend-rs/src/services/chapter_batch_generation_task_view_query_service.rs`
    中新增：
    - `PreparedActiveBatchGenerationTaskListView`
    - `PreparedActiveProjectBatchGenerationQuery`
    - `PreparedExistingSingleGenerationBackgroundTaskPayloadQuery`
  - 这些 owner 现在统一负责：
    - active task row selection 后的 payload loading
    - compat list/project envelope projection
    - existing-background payload ownership
  - `load_active_user_batch_generation_task_list_view(...)`
    与 `load_active_batch_generation_query(...)`
    现在只消费 prepared owner，不再在 outer function body 本地保留
    `{total, items}` / `{has_active_task, task}` 这条最终支路
  - 同时新增 focused owner test
    `should_keep_existing_single_generation_background_payload_query_owner_contract`
    和已有 envelope owner tests 一起锁住这条新 contract

  这条 seam 的意义不只是“query 入口少了几行 json 包装”。它真正回答的是：
  batch active-query lane 在拿到 selected active-task payload 之后，到底由哪个
  Rust owner 负责把它们推进到最终 compat response envelope。现在这条 duplicate
  已被删掉，batch active-query lane 在 query-response ownership 上又向前收了一层。

  对 Phase 5 的价值同样直接：
  - cutover 审计时更容易回答“active list / active project 最终 envelope
    到底由哪个 Rust owner 提供 contract”
  - active task selection / payload materialization / final compat response
    envelope projection 现在共享更连续的 owner 链
  - fallback shrink / rollback / stronger smoke 在 batch active-query lane 上
    又少了一条隐藏在 query 邻层的 local response-envelope projection 支路

  这条 seam 已通过：
  - `cargo fmt --manifest-path "backend-rs/Cargo.toml"`
  - `cargo test task_view_query --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-task-query-envelope-owner" -- --nocapture`
  - `cargo test read_context --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-task-query-envelope-owner" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-task-query-envelope-owner"`

- 2026-06-05 同一条 `chapter_generation` Phase 5 lane 又沿
  batch runtime 邻链收掉了一条
  “runtime driver 明明已经有 preparing persistence / step execution /
  stop-continue handoff owner，最外层仍本地串完整 lifecycle” 的 seam。当前：
  - `chapter_batch_generation` batch runtime 邻域已经有：
    - preparing-stage persistence owner
    - chapter-level runtime step execution owner
    - per-step progression stop/continue handoff
  - 但
    `chapter_batch_generation_runtime_state_service.rs`
    之前的 `BatchGenerationRuntimeDriver`
    仍会：
    - 先持久化 preparing 状态
    - 再本地遍历 `chapter_ids`
    - 再本地消费每一步的 stop/continue progression
  - 这代表 batch runtime lane 在 Phase 5 上仍保留一条
    Python-era 旧 hop：
    lifecycle owner 邻域已经足够完整，但 outer runtime driver 仍保留一条
    final lifecycle orchestration 支路

  本轮已把这条 batch runtime lifecycle contract 真正前移回 runtime owner：
  - `backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs`
    中新增 `BatchGenerationRuntimeLifecyclePlan`
  - 这个 owner 现在统一负责：
    - preparing persistence
    - chapter-id step loop
    - step progression stop/continue handoff
  - `BatchGenerationRuntimeDriver`
    现在只持有并执行这个 lifecycle owner，不再在 outer driver body 本地保留
    `prepare -> iterate -> handoff` 这条最终支路

  这条 seam 的意义不只是“driver 变短了一点”。它真正回答的是：
  batch runtime lane 在进入 runtime lifecycle 之后，到底由哪个 Rust owner 负责把
  “preparing persistence + chapter iteration + progression handoff”推进到最终
  runtime 执行序列。现在这条 duplicate 已被删掉，batch runtime lane 在
  lifecycle ownership 上又向前收了一层。

  对 Phase 5 的价值同样直接：
  - cutover 审计时更容易回答“batch runtime lifecycle 到底由哪个 Rust owner
    提供 contract”
  - preparing persistence / step execution / stop-continue handoff
    现在共享更连续的 owner 链
  - fallback shrink / rollback / stronger smoke 在 batch runtime lane 上又少了
    一条隐藏在 runtime driver 邻层的 local lifecycle orchestration 支路

  这条 seam 已通过：
  - `cargo fmt --manifest-path "backend-rs/Cargo.toml"`
  - `cargo test should_keep_batch_generation_runtime_lifecycle_owner_contract --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-runtime-lifecycle-owner" -- --nocapture`
  - `cargo test chapter_batch_generation_runtime_state_service --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-runtime-lifecycle-owner" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-runtime-lifecycle-owner"`

- 2026-06-05 同一条 `chapter_generation` Phase 5 lane 又沿
  batch runtime success 邻链再收掉了一条
  “content generation 成功后，外层 success branch 明明已经贴着
  post-write guard / follow-up analysis / quality-gate / post-generation
  persistence owner，仍本地串完整 success-only flow” 的 seam。当前：
  - `chapter_batch_generation` batch runtime success 邻域已经有：
    - post-write guard owner
    - follow-up analysis with failure contract owner
    - quality-gate routing owner
    - post-generation success persistence owner
  - 但
    `run_batch_generation_generation_attempt(...)`
    之前在 success branch 仍会：
    - 先本地检查 post-write guard
    - 再本地处理 stop vs continue
    - 再本地调用 follow-up analysis
    - 最后再本地把 continue outcome 持久化成 post-generation success
  - 这代表 batch runtime lane 在 Phase 5 上仍保留一条
    Python-era 旧 hop：
    success-path owner 邻域已经足够完整，但 outer generation-attempt
    success branch 仍保留一条 final success orchestration 支路

  本轮已把这条 batch runtime success-attempt contract 真正前移回 runtime owner：
  - `backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs`
    中新增 `BatchGenerationSuccessAttemptPlan`
  - 这个 owner 现在统一负责：
    - post-write guard verification
    - follow-up analysis handoff
    - post-generation success persistence
  - `run_batch_generation_generation_attempt(...)`
    的 success branch 现在只 materialize 并执行这个 success-attempt owner，
    不再在 outer branch body 本地保留
    `guard -> analysis -> persist success` 这条最终支路

  这条 seam 的意义不只是“success 分支又短了一点”。它真正回答的是：
  batch runtime lane 在单章内容已经生成成功之后，到底由哪个 Rust owner 负责把
  “guard / analysis / quality-gate / success persistence”推进到最终 post-generation
  outcome。现在这条 duplicate 已被删掉，batch runtime success lane 在
  ownership 上又向前收了一层。

  对 Phase 5 的价值同样直接：
  - cutover 审计时更容易回答“batch runtime success-attempt 到底由哪个 Rust owner
    提供 contract”
  - post-write guard / follow-up analysis / quality-gate routing /
    post-generation persistence 现在共享更连续的 owner 链
  - fallback shrink / rollback / stronger smoke 在 batch runtime lane 上又少了
    一条隐藏在 success branch 邻层的 local success-only orchestration 支路

  这条 seam 已通过：
  - `cargo fmt --manifest-path "backend-rs/Cargo.toml"`
  - `cargo test should_keep_batch_generation_success_attempt_owner_contract --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-runtime-success-attempt-owner" -- --nocapture`
  - `cargo test chapter_batch_generation_runtime_state_service --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-runtime-success-attempt-owner" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-runtime-success-attempt-owner"`

- 2026-06-05 同一条 `chapter_generation` Phase 5 lane 又沿
  batch runtime attempt 邻链再收掉了一条
  “outer attempt body 在进入 success/failure owner 之前，仍本地串
  started persistence / prerequisite / provider payload / generation dispatch”
  的 seam。当前：
  - `chapter_batch_generation` batch runtime attempt 邻域已经有：
    - chapter-started persistence owner
    - prerequisite gate handling
    - provider-payload preparation
    - generation dispatch
    - success/failure outcome routing
  - 但
    `run_batch_generation_generation_attempt(...)`
    之前仍会：
    - 先本地解析 compat overrides
    - 再本地持久化 chapter-started
    - 再本地处理 prerequisite gate
    - 再本地准备 provider payload
    - 最后才把结果交给 generation dispatch 和后续 success/failure owner
  - 这代表 batch runtime lane 在 Phase 5 上仍保留一条
    Python-era 旧 hop：
    attempt owner 邻域已经足够完整，但 outer attempt body 仍保留一条
    final pre-generation orchestration 支路

  本轮已把这条 batch runtime generation-attempt contract 真正前移回 runtime owner：
  - `backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs`
    中新增 `PreparedBatchGenerationGenerationAttempt`
  - 这个 owner 现在统一负责：
    - chapter-started persistence
    - prerequisite handling
    - provider-payload preparation
    - generation dispatch handoff
  - `run_batch_generation_generation_attempt(...)`
    现在只 materialize 并执行这个 prepared attempt owner，不再在 outer
    attempt body 本地保留
    `started -> prerequisite -> provider -> generate` 这条最终支路

  这条 seam 的意义不只是“attempt 函数又短了一点”。它真正回答的是：
  batch runtime lane 在每个 chapter attempt 真正开始之后，到底由哪个 Rust owner
  负责把“started persistence / prerequisite / provider / generate”推进到
  success 或 failure 的后续 routing。现在这条 duplicate 已被删掉，batch
  runtime attempt lane 在 ownership 上又向前收了一层。

  对 Phase 5 的价值同样直接：
  - cutover 审计时更容易回答“batch runtime generation-attempt 到底由哪个 Rust owner
    提供 contract”
  - chapter-started persistence / prerequisite gate / provider-payload preparation /
    generation dispatch / success-failure routing 现在共享更连续的 owner 链
  - fallback shrink / rollback / stronger smoke 在 batch runtime lane 上又少了
    一条隐藏在 attempt 邻层的 local pre-generation orchestration 支路

  这条 seam 已通过：
  - `cargo fmt --manifest-path "backend-rs/Cargo.toml"`
  - `cargo test should_keep_prepared_batch_generation_generation_attempt_owner_contract --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-runtime-generation-attempt-owner" -- --nocapture`
  - `cargo test chapter_batch_generation_runtime_state_service --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-runtime-generation-attempt-owner" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-runtime-generation-attempt-owner"`

- 2026-06-05 同一条 `chapter_generation` Phase 5 lane 又沿
  batch runtime step 邻链再收掉了一条
  “outer step boundary 在进入 per-chapter generation attempt 之前，仍本地串
  task reload / cancel gate / chapter lookup / project match validation”
  的 seam。当前：
  - `chapter_batch_generation` batch runtime step 邻域已经有：
    - owned task reload owner
    - cancelled-status persistence and stop routing
    - chapter lookup failure routing
    - project-match validation
  - 但
    `prepare_batch_generation_step_execution(...)`
    之前仍会：
    - 先本地加载 task model
    - 再本地处理 cancelled gate 和 cancelled runtime persistence
    - 再本地加载 chapter model
    - 最后才本地校验 project match 并返回 prepared step
  - 这代表 batch runtime lane 在 Phase 5 上仍保留一条
    Python-era 旧 hop：
    step-preparation owner 邻域已经足够完整，但 outer step boundary 仍保留一条
    final attempt-entry preparation orchestration 支路

  本轮已把这条 batch runtime step-preparation contract 真正前移回 runtime owner：
  - `backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs`
    中扩展 `PreparedBatchGenerationStepExecution`
  - 这个 owner 现在统一负责：
    - owned task reload
    - cancelled-status gate
    - chapter lookup
    - project-match validation
  - `prepare_batch_generation_step_execution(...)`
    现在只 materialize 这个 step-preparation owner，不再在 outer step body
    本地保留
    `load task -> cancel gate -> load chapter -> validate match`
    这条最终支路

  这条 seam 的意义不只是“prepare 函数又短了一点”。它真正回答的是：
  batch runtime lane 在每个 chapter step 进入 generation attempt 之前，到底由哪个
  Rust owner 负责把“task reload / cancelled persistence / chapter lookup /
  project validation”推进到最终 prepared step。现在这条 duplicate 已被删掉，
  batch runtime step lane 在 ownership 上又向前收了一层。

  对 Phase 5 的价值同样直接：
  - cutover 审计时更容易回答“batch runtime step-preparation 到底由哪个 Rust owner
    提供 contract”
  - task reload / cancel gate / chapter lookup / project validation /
    downstream generation-attempt execution 现在共享更连续的 owner 链
  - fallback shrink / rollback / stronger smoke 在 batch runtime lane 上又少了
    一条隐藏在 step boundary 邻层的 local attempt-entry orchestration 支路

  这条 seam 已通过：
  - `cargo fmt --manifest-path "backend-rs/Cargo.toml"`
  - `cargo test should_keep_prepared_batch_generation_step_execution_owner_contract --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-runtime-step-preparation-owner" -- --nocapture`
  - `cargo test chapter_batch_generation_runtime_state_service --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-runtime-step-preparation-owner" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-runtime-step-preparation-owner"`

- 2026-06-05 同一条 `chapter_generation` Phase 5 lane 又沿
  batch runtime step 邻链再收掉了一条
  “outer lifecycle/step boundary 在进入 chapter attempt 之后，仍本地串
  prepare / retry carry / execute attempt”
  的 seam。当前：
  - `chapter_batch_generation` batch runtime step 邻域已经有：
    - step chapter-id/progress input owner
    - step-preparation owner
    - retry-count carry semantics
    - prepared-step execute handoff
  - 但 runtime lifecycle 邻层之前仍会：
    - 先本地构造 step 输入
    - 再本地处理 step-preparation retry loop
    - 再本地把 retry count 回填到 prepared step
    - 最后才本地调用 prepared step execute
  - 这代表 batch runtime lane 在 Phase 5 上仍保留一条
    Python-era 旧 hop：
    step-execution owner 邻域已经足够完整，但 outer lifecycle body 仍保留一条
    final per-step orchestration 支路

  本轮已把这条 batch runtime step-execution contract 真正前移回 runtime owner：
  - `backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs`
    中新增 `BatchGenerationStepExecutionPlan`
  - 这个 owner 现在统一负责：
    - step chapter-id/progress input
    - step-preparation retry carry
    - retry-count patch-back
    - prepared-step execute handoff
  - `BatchGenerationRuntimeLifecyclePlan::execute(...)`
    现在只 materialize 并执行这个 step-execution owner，不再在 outer
    lifecycle body 本地保留
    `prepare -> retry carry -> execute attempt`
    这条最终支路

  这条 seam 的意义不只是“step 函数又短了一点”。它真正回答的是：
  batch runtime lane 在每个 chapter step 已经进入 execution phase 之后，到底由哪个
  Rust owner 负责把“prepare / retry carry / execute attempt”推进到最终
  runtime progression。现在这条 duplicate 已被删掉，batch runtime step lane
  在 ownership 上又向前收了一层。

  对 Phase 5 的价值同样直接：
  - cutover 审计时更容易回答“batch runtime step-execution 到底由哪个 Rust owner
    提供 contract”
  - step input / step-preparation retry semantics / prepared-step execution /
    downstream generation-attempt routing 现在共享更连续的 owner 链
  - fallback shrink / rollback / stronger smoke 在 batch runtime lane 上又少了
    一条隐藏在 lifecycle 邻层的 local per-step orchestration 支路

  这条 seam 已通过：
  - `cargo fmt --manifest-path "backend-rs/Cargo.toml"`
  - `cargo test should_keep_batch_generation_step_execution_plan_owner_contract --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-runtime-step-execution-owner" -- --nocapture`
  - `cargo test chapter_batch_generation_runtime_state_service --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-runtime-step-execution-owner" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-runtime-step-execution-owner"`

- 2026-06-05 同一条 `chapter_generation` Phase 5 lane 又沿
  batch runtime 后段邻链再收掉了一条
  “outer success branch 在 chapter content 已生成后，仍本地串
  analysis / quality gate / persist success-fail stop”
  的 seam。当前：
  - `chapter_batch_generation` batch runtime post-generation 邻域已经有：
    - follow-up analysis owner
    - quality-gate terminal routing owner
    - success persistence handoff
    - analysis-failure stop routing
  - 但 `BatchGenerationSuccessAttemptPlan::execute(...)` 之前仍会：
    - 先本地运行 follow-up analysis
    - 再本地判断 analysis success vs failure
    - 再本地消费 quality-gate outcome
    - 最后再本地把 success next-progress 持久化成 chapter succeeded
  - 这代表 batch runtime lane 在 Phase 5 上仍保留一条
    Python-era 旧 hop：
    post-generation owner 邻域已经足够完整，但 outer success branch 仍保留一条
    final terminal orchestration 支路

  本轮已把这条 batch runtime post-generation contract 真正前移回 runtime owner：
  - `backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs`
    中新增 `BatchGenerationPostGenerationPlan`
  - 这个 owner 现在统一负责：
    - follow-up analysis execution
    - analysis success/failure routing
    - quality-gate routing
    - success persistence handoff
  - `BatchGenerationSuccessAttemptPlan::execute(...)`
    现在只保留 post-write guard，然后 materialize 并执行这个
    post-generation owner，不再在 outer success body 本地保留
    `analysis -> quality gate -> persist success/fail stop`
    这条最终支路

  这条 seam 的意义不只是“success 分支又短了一点”。它真正回答的是：
  batch runtime lane 在 chapter content 已经落盘之后，到底由哪个 Rust owner
  负责把“analysis / quality gate / success persistence / fail stop”推进到最终
  runtime terminal outcome。现在这条 duplicate 已被删掉，batch runtime
  post-generation lane 在 ownership 上又向前收了一层。

  对 Phase 5 的价值同样直接：
  - cutover 审计时更容易回答“batch runtime post-generation 到底由哪个 Rust owner
    提供 contract”
  - follow-up analysis / quality-gate terminal routing / success persistence /
    analysis-failure stop 现在共享更连续的 owner 链
  - fallback shrink / rollback / stronger smoke 在 batch runtime lane 上又少了
    一条隐藏在 success 邻层的 local post-generation orchestration 支路

  这条 seam 已通过：
  - `cargo fmt --manifest-path "backend-rs/Cargo.toml"`
  - `cargo test should_keep_batch_generation_post_generation_plan_owner_contract --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-runtime-post-generation-owner" -- --nocapture`
  - `cargo test chapter_batch_generation_runtime_state_service --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-runtime-post-generation-owner" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-runtime-post-generation-owner"`

- 2026-06-05 同一条 `chapter_generation` Phase 5 lane 又沿
  batch runtime analysis 邻链再收掉了一条
  “outer post-generation body 在 post-write guard 之后，仍本地串
  enable_analysis gate / retry loop / execute attempt / complete-or-retry”
  的 seam。当前：
  - `chapter_batch_generation` batch runtime analysis 邻域已经有：
    - analysis-started persistence owner
    - prepared-analysis execute vs direct follow-up execute handoff
    - analysis completion persistence owner
    - analysis retry/stop routing owner
  - 但 `BatchGenerationPostGenerationPlan::execute(...)` 邻层之前仍会：
    - 先本地判断 `enable_analysis`
    - 再本地循环 `analysis_retry_count in 0..3`
    - 再本地执行 analysis attempt
    - 再本地判断 completed vs retry
    - 最后在预算耗尽时返回 `"章节分析失败"`
  - 这代表 batch runtime lane 在 Phase 5 上仍保留一条
    Python-era 旧 hop：
    analysis-workflow owner 邻域已经足够完整，但 outer post-generation body
    仍保留一条 final analysis orchestration 支路

  本轮已把这条 batch runtime analysis-workflow contract 真正前移回 runtime owner：
  - `backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs`
    中新增 `BatchGenerationFollowUpAnalysisPlan`
  - 这个 owner 现在统一负责：
    - `enable_analysis` gate
    - analysis retry budget loop
    - analysis-attempt execution handoff
    - completed vs retry resolution
    - terminal `"章节分析失败"` contract
  - `BatchGenerationPostGenerationPlan::execute(...)`
    现在直接 materialize 并执行这个 analysis-workflow owner，不再在 outer
    post-generation body 本地保留
    `gate -> retry loop -> execute attempt -> resolve result`
    这条最终支路

  这条 seam 的意义不只是“analysis 函数被包了一层 owner”。它真正回答的是：
  batch runtime lane 在 post-generation owner 已经决定进入 analysis 之后，到底由
  哪个 Rust owner 负责把“analysis gate / retry budget / attempt execution /
  completed-or-retry contract”推进到最终结果。现在这条 duplicate 已被删掉，
  batch runtime analysis lane 在 ownership 上又向前收了一层。

  对 Phase 5 的价值同样直接：
  - cutover 审计时更容易回答“batch runtime analysis-workflow 到底由哪个 Rust
    owner 提供 contract”
  - analysis gating / retry budgeting / attempt execution / retry routing /
    terminal failure semantics 现在共享更连续的 owner 链
  - fallback shrink / rollback / stronger smoke 在 batch runtime lane 上又少了
    一条隐藏在 post-generation 邻层的 local analysis orchestration 支路

  这条 seam 已通过：
  - `cargo fmt --manifest-path "backend-rs/Cargo.toml"`
  - `cargo test should_keep_batch_generation_follow_up_analysis_plan_owner_contract --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-runtime-analysis-workflow-owner" -- --nocapture`
  - `cargo test chapter_batch_generation_runtime_state_service --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-runtime-analysis-workflow-owner" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-runtime-analysis-workflow-owner"`

- 2026-06-05 同一条 `chapter_generation` Phase 5 lane 又沿
  batch runtime post-analysis terminal 邻链再收掉了一条
  “outer post-generation body 在 analysis workflow 已有结果后，仍本地串
  success/failure branching / quality-gate route / fail stop / persist success”
  的 seam。当前：
  - `chapter_batch_generation` batch runtime post-analysis 邻域已经有：
    - follow-up analysis workflow owner
    - quality-gate terminal routing owner
    - success progression persistence owner
    - analysis-failure stop owner
  - 但 `BatchGenerationPostGenerationPlan::execute(...)` 邻层之前仍会：
    - 先本地判断 analysis success vs failure
    - 再本地读取 retry context 以做 quality-gate 判定
    - 再本地消费 quality-gate outcome
    - 最后再本地把 next-progress 持久化成 chapter succeeded
  - 这代表 batch runtime lane 在 Phase 5 上仍保留一条
    Python-era 旧 hop：
    post-analysis-resolution owner 邻域已经足够完整，但 outer
    post-generation body 仍保留一条 final terminal resolution 支路

  本轮已把这条 batch runtime post-analysis-resolution contract 真正前移回
  runtime owner：
  - `backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs`
    中新增 `BatchGenerationPostAnalysisResolutionPlan`
  - 这个 owner 现在统一负责：
    - analysis success/failure branching
    - quality-gate routing
    - analysis-failure stop handoff
    - success progression persistence
  - `BatchGenerationPostGenerationPlan::execute(...)`
    现在只负责：
    - materialize 并执行 `BatchGenerationFollowUpAnalysisPlan`
    - 将分析结果 handoff 给 `BatchGenerationPostAnalysisResolutionPlan`
    不再在 outer post-generation body 本地保留
    `success/failure -> quality gate / fail stop -> persist success`
    这条最终支路

  这条 seam 的意义不只是“post-generation body 又短了一点”。它真正回答的是：
  batch runtime lane 在 follow-up analysis 已经完成之后，到底由哪个 Rust owner
  负责把“analysis outcome branching / quality gate / fail stop / success persist”
  推进到最终 terminal progression。现在这条 duplicate 已被删掉，batch runtime
  post-analysis lane 在 ownership 上又向前收了一层。

  对 Phase 5 的价值同样直接：
  - cutover 审计时更容易回答“batch runtime post-analysis-resolution 到底由哪个
    Rust owner 提供 contract”
  - analysis outcome branching / quality-gate routing / fail-stop handling /
    success progression persistence 现在共享更连续的 owner 链
  - fallback shrink / rollback / stronger smoke 在 batch runtime lane 上又少了
    一条隐藏在 post-generation 邻层的 local terminal-resolution 支路

  这条 seam 已通过：
  - `cargo fmt --manifest-path "backend-rs/Cargo.toml"`
  - `cargo test should_keep_batch_generation_post_analysis_resolution_plan_owner_contract --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-runtime-post-analysis-resolution-owner" -- --nocapture`
  - `cargo test chapter_batch_generation_runtime_state_service --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-runtime-post-analysis-resolution-owner" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-runtime-post-analysis-resolution-owner"`

- 2026-06-05 同一条 `chapter_generation` Phase 5 lane 又沿
  batch runtime quality-gate terminal 邻链再收掉了一条
  “outer post-analysis-resolution body 在 analysis success 之后，仍本地串
  snapshot load / workflow-state merge / terminal semantics / retry-manual-review route”
  的 seam。当前：
  - `chapter_batch_generation` batch runtime quality-gate 邻域已经有：
    - snapshot load owner
    - quality-gate terminal semantics owner
    - retry/manual-review persistence routing owner
    - success progression fallback
  - 但 `BatchGenerationPostAnalysisResolutionPlan::resolve_analysis_success_outcome(...)`
    邻层之前仍会：
    - 先本地读取 retry context
    - 再本地加载 snapshot
    - 再本地把 current quality-runtime state 与 persisted workflow state 合并
    - 最后才本地解析 terminal semantics 并消费 routing outcome
  - 这代表 batch runtime lane 在 Phase 5 上仍保留一条
    Python-era 旧 hop：
    quality-gate-resolution owner 邻域已经足够完整，但 outer
    post-analysis-resolution body 仍保留一条 final quality-gate routing 支路

  本轮已把这条 batch runtime quality-gate-resolution contract 真正前移回
  runtime owner：
  - `backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs`
    中新增 `BatchGenerationQualityGateResolutionPlan`
  - 这个 owner 现在统一负责：
    - snapshot load
    - workflow-state merge
    - terminal semantics resolution
    - retry/manual-review routing
  - `BatchGenerationPostAnalysisResolutionPlan::resolve_analysis_success_outcome(...)`
    现在只负责：
    - 读取 retry budget
    - materialize 并执行 `BatchGenerationQualityGateResolutionPlan`
    - 在无 gate block 时继续 success persistence
    不再在 outer post-analysis-resolution body 本地保留
    `load snapshot -> resolve semantics -> route gate`
    这条最终支路

  这条 seam 的意义不只是“quality-gate 函数被改成 struct”。它真正回答的是：
  batch runtime lane 在 analysis success 已经拿到 current quality-runtime state
  之后，到底由哪个 Rust owner 负责把“snapshot merge / terminal semantics /
  retry-manual-review route”推进到最终 terminal outcome。现在这条 duplicate
  已被删掉，batch runtime quality-gate lane 在 ownership 上又向前收了一层。

  对 Phase 5 的价值同样直接：
  - cutover 审计时更容易回答“batch runtime quality-gate-resolution 到底由哪个
    Rust owner 提供 contract”
  - quality-runtime-state merge / terminal semantics / retry-manual-review
    routing / success fallback 现在共享更连续的 owner 链
  - fallback shrink / rollback / stronger smoke 在 batch runtime lane 上又少了
    一条隐藏在 post-analysis-resolution 邻层的 local quality-gate 支路

  这条 seam 已通过：
  - `cargo fmt --manifest-path "backend-rs/Cargo.toml"`
  - `cargo test should_keep_batch_generation_quality_gate_resolution_plan_owner_contract --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-runtime-quality-gate-resolution-owner" -- --nocapture`
  - `cargo test chapter_batch_generation_runtime_state_service --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-runtime-quality-gate-resolution-owner" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-runtime-quality-gate-resolution-owner"`

- 2026-06-05 同一条 `chapter_generation` Phase 5 lane 又沿
  batch runtime helper-wrapper 邻链再收掉了一组
  “owner 已存在，但 outer runtime chain 仍保留只做 owner 转发的薄 helper hop”
  的 seam。当前：
  - `chapter_batch_generation` batch runtime 邻域已经有：
    - generic-failure routing owner
    - retry progression owner
    - post-write guard owner
    - prepared step owner
    - prepared generation-attempt owner
  - 但 runtime chain 之前仍保留几条只做 owner 转发的旧 helper：
    - `resolve_batch_generation_retry_attempt_progression(...)`
    - `resolve_batch_generation_generic_failure_outcome(...)`
    - `prepare_batch_generation_step_execution(...)`
    - `run_batch_generation_generation_attempt(...)`
    - `load_batch_generation_post_write_guard_outcome(...)`
    - `resolve_batch_generation_post_write_guard_outcome(...)`
  - 这代表 batch runtime lane 在 Phase 5 上仍保留一类
    Python-era 旧 hop：
    owner contract 明明已经齐备，但 outer runtime chain 仍通过 free helper
    重新打开 owner-ready input，再把同一批 handoff 语义送回 runtime owner

  本轮已把这组 batch runtime helper-wrapper contract 真正前移回 runtime owner：
  - `backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs`
    中新增 `BatchGenerationRetryProgressionPlan`
  - 这个 owner 现在统一负责：
    - retry backoff
    - `Retry(next_retry_count)` progression contract
  - `BatchGenerationGenericFailureRoutingPlan::persist_and_resolve(...)`
    与 `BatchGenerationQualityGateRoutingPlan::persist_and_resolve(...)`
    现在都直接 materialize 并执行这个 owner

  - 同文件中新增 `BatchGenerationPostWriteGuardPlan`
  - 这个 owner 现在统一负责：
    - post-write guard snapshot load
    - post-write guard resolve semantics
  - `BatchGenerationSuccessAttemptPlan::execute(...)`
    现在直接 materialize 并执行这个 owner，不再保留
    `load/resolve post-write guard`
    这条 free helper hop

  - step / generation-attempt 邻链也继续前移：
    - `BatchGenerationStepExecutionPlan::execute(...)`
      现在直接消费 `PreparedBatchGenerationStepExecution::prepare(...)`
    - `PreparedBatchGenerationStepExecution::execute(...)`
      现在直接 prepare 并 execute
      `PreparedBatchGenerationGenerationAttempt`
    - prerequisite error / prerequisite block / provider payload build error /
      generation error
      现在都直接 materialize
      `BatchGenerationGenericFailureRoutingPlan::from_step_error(...).persist_and_resolve(...)`
  - 因此旧 free helper：
    - `resolve_batch_generation_retry_attempt_progression(...)`
    - `resolve_batch_generation_generic_failure_outcome(...)`
    - `prepare_batch_generation_step_execution(...)`
    - `run_batch_generation_generation_attempt(...)`
    - `load_batch_generation_post_write_guard_outcome(...)`
    - `resolve_batch_generation_post_write_guard_outcome(...)`
    已从生产路径移除

  这条 seam 的意义不只是“又少了几个 helper”。它真正回答的是：
  batch runtime lane 在 retry progression、generic failure routing、
  post-write guard resolution，以及 prepared step / generation-attempt handoff
  已经有明确 Rust owner 之后，到底由哪个 owner 负责把这些 contract 连成连续的
  runtime chain。现在这类 duplicate 已被删掉，batch runtime lane 在 ownership
  上又向前收了一层。

  对 Phase 5 的价值同样直接：
  - cutover 审计时更容易回答“batch runtime helper-wrapper elimination
    之后，哪一段已经是连续的 Rust-owned chain”
  - retry progression / generic failure routing / post-write guard resolution /
    prepared step execute / generation-attempt execute
    现在共享更连续的 owner 链
  - fallback shrink / rollback clarity / stronger smoke 在 batch runtime lane
    上又少了一类隐藏在 outer runtime body 邻层的 local helper hop

  这条 seam 已通过：
  - `cargo fmt --manifest-path "backend-rs/Cargo.toml"`
  - `cargo test should_build_batch_generation_retry_progression_plan_owner_contract --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-runtime-helper-wrapper-owner" -- --nocapture`
  - `cargo test should_build_batch_generation_post_write_guard_plan_owner_contract --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-runtime-helper-wrapper-owner" -- --nocapture`
  - `cargo test should_keep_batch_generation_quality_gate_resolution_plan_owner_contract --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-runtime-helper-wrapper-owner" -- --nocapture`
  - `cargo test chapter_batch_generation_runtime_state_service --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-runtime-helper-wrapper-owner" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-runtime-helper-wrapper-owner"`

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
6. 基于上述治理资产推进的 `backend-rs` 模块级迁移包，并在包内保留小步验证
7. 面向 `settings` / `projects` 等第一批 shrink-readiness 模板的
   route-group readiness 摘要能力

- 2026-06-05 同一条 `chapter_generation` Phase 5 lane 又沿
  batch runtime analysis-attempt 邻链再收掉了一条
  “analysis workflow owner 已存在，但 inner attempt 仍通过 free helper 串
  started / execute / completion-or-retry”
  的 seam。当前：
  - `chapter_batch_generation` batch runtime analysis 邻域已经有：
    - analysis-started persistence owner
    - prepared analysis execute vs direct follow-up execute handoff
    - analysis completion persistence owner
    - analysis retry / stop routing owner
  - 但 `BatchGenerationFollowUpAnalysisPlan::execute(...)` 之前仍会经由两条
    free helper：
    - `execute_batch_generation_analysis_attempt(...)`
    - `persist_and_resolve_batch_generation_analysis_attempt(...)`
  - 这代表 batch runtime analysis lane 在 Phase 5 上仍保留一条
    Python-era 旧 hop：
    outer analysis workflow owner 明明已经齐备，但 inner attempt lifecycle
    仍在 owner 外部通过 free helper 重放
    `persist started -> execute analysis -> persist completed or route retry`
    这条 handoff 支路

  本轮已把这条 batch runtime analysis-attempt contract 真正前移回 runtime owner：
  - `backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs`
    中新增 `BatchGenerationAnalysisAttemptPlan`
  - 这个 owner 现在统一负责：
    - analysis-started persistence
    - prepared-analysis execute handoff
    - direct follow-up analysis fallback handoff
    - resolution owner handoff

  - 同文件中新增 `BatchGenerationAnalysisAttemptResolutionPlan`
  - 这个 owner 现在统一负责：
    - completion persistence
    - retry / stop routing
  - `BatchGenerationFollowUpAnalysisPlan::execute(...)`
    现在只保留：
    - `enable_analysis` gate
    - retry budget loop
    - execute analysis-attempt owner
    不再在 owner 外部保留 inner attempt free helper hop

  - 因此旧 free helper：
    - `execute_batch_generation_analysis_attempt(...)`
    - `persist_and_resolve_batch_generation_analysis_attempt(...)`
    已从生产路径移除

  这条 seam 的意义不只是“analysis helper 改成 struct”。它真正回答的是：
  batch runtime lane 在 follow-up analysis workflow 已经决定进入某一次 attempt
  之后，到底由哪个 Rust owner 负责把
  `started persistence / execute handoff / completion or retry`
  推进到最终 analysis attempt outcome。现在这条 duplicate 已被删掉，
  batch runtime analysis-attempt lane 在 ownership 上又向前收了一层。

  对 Phase 5 的价值同样直接：
  - cutover 审计时更容易回答“batch runtime analysis attempt 到底由哪个
    Rust owner 提供 contract”
  - analysis-started persistence / prepared execute handoff /
    completion persistence / retry-stop routing
    现在共享更连续的 owner 链
  - fallback shrink / rollback clarity / stronger smoke 在 batch runtime
    analysis lane 上又少了一条隐藏在 workflow 邻层的 local attempt helper hop

  这条 seam 已通过：
  - `cargo fmt --manifest-path "backend-rs/Cargo.toml"`
  - `cargo test chapter_batch_generation_runtime_state_service --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-runtime-analysis-attempt-owner" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-runtime-analysis-attempt-owner"`

- 2026-06-05 同一条 `chapter_generation` Phase 5 lane 又沿
  batch runtime generation-attempt input 邻链再收掉了一条
  “prepared generation-attempt owner 已存在，但 compat restore / prompt
  override / provider payload 仍通过 local/free helper 拼装”
  的 seam。当前：
  - `chapter_batch_generation` generation-attempt 邻域已经有：
    - persisted runtime snapshot restore
    - compat-option recovery
    - prompt-override projection
    - provider-payload preparation
    - prepared generation-attempt execute handoff
  - 但 `PreparedBatchGenerationGenerationAttempt::prepare(...)` 之前仍会经由：
    - `resolve_runtime_compat_options_for_batch_generation_step(...)`
    - `build_runtime_provider_payload_for_batch_generation_step(...)`
    这两条 free helper 来本地重组 attempt input
  - 这代表 batch runtime generation-attempt lane 在 Phase 5 上仍保留一条
    Python-era 旧 hop：
    owner-ready 的 runtime snapshot / provider input 语义明明已经齐备，
    但 prepare body 仍在 owner 外部重放
    `compat restore -> prompt overrides -> provider payload`
    这条 handoff 支路

  本轮已把这条 batch runtime attempt-input contract 真正前移回 runtime owner：
  - `backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs`
    中新增 `BatchGenerationAttemptInputPlan`
  - 这个 owner 现在统一负责：
    - persisted compat restore
    - prompt-override projection
    - provider payload preparation
  - `PreparedBatchGenerationGenerationAttempt::prepare(...)`
    现在直接 materialize 并消费这个 owner，不再在 prepare body 本地保留
    上述 input assembly hop

  - 因此旧 free helper：
    - `build_runtime_provider_payload_for_batch_generation_step(...)`
    已从生产路径移除

  这条 seam 的意义不只是“又少了一个 helper”。它真正回答的是：
  batch runtime lane 在某一次 chapter generation attempt 已经进入 prepared
  input 阶段之后，到底由哪个 Rust owner 负责把
  `compat restore / prompt overrides / provider payload`
  推进到最终 attempt input。现在这条 duplicate 已被删掉，batch runtime
  generation-attempt input lane 在 ownership 上又向前收了一层。

  对 Phase 5 的价值同样直接：
  - cutover 审计时更容易回答“batch runtime generation-attempt input 到底由
    哪个 Rust owner 提供 contract”
  - runtime snapshot restore / prompt override projection / provider payload
    preparation / prepared-attempt execute
    现在共享更连续的 owner 链
  - fallback shrink / rollback clarity / stronger smoke 在 batch runtime
    generation lane 上又少了一条隐藏在 prepare 邻层的 local input hop

  这条 seam 已通过：
  - `cargo fmt --manifest-path "backend-rs/Cargo.toml"`
  - `cargo test chapter_batch_generation_runtime_state_service --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-runtime-attempt-input-owner" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-runtime-attempt-input-owner"`

- 2026-06-05 同一条 `chapter_generation` Phase 5 lane 又沿
  batch runtime post-analysis terminal 邻链再收掉了一条
  “post-analysis resolution owner 已存在，但最后 success/failure terminal
  persistence 仍分散在 local methods / free helper”
  的 seam。当前：
  - `chapter_batch_generation` post-analysis 邻域已经有：
    - post-analysis resolution owner
    - quality-gate resolution owner
    - failed terminal persistence path
    - success progression persistence path
  - 但 `BatchGenerationPostAnalysisResolutionPlan::execute(...)` 之前仍会把
    最后终态 handoff 分散到：
    - `resolve_analysis_success_outcome(...)`
    - `fail_after_analysis(...)`
    - `fail_batch_generation_after_analysis(...)`
    - `persist_batch_generation_post_generation_success_outcome(...)`
  - 这代表 batch runtime post-analysis lane 在 Phase 5 上仍保留一条
    Python-era 旧 hop：
    resolved analysis outcome 已经清楚，但最后
    `quality gate or fail stop -> persist terminal outcome`
    仍在 owner 外部分叉收尾

  本轮已把这条 batch runtime post-analysis-terminal contract 真正前移回 runtime owner：
  - `backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs`
    中新增 `BatchGenerationPostAnalysisTerminalPlan`
  - 这个 owner 现在统一负责：
    - success terminal handoff
    - failure terminal handoff
    - final persistence routing
  - 同时新增 `BatchGenerationPostAnalysisTerminalOutcome`
    让 success vs failure 分支在 owner 边界显式化
  - `BatchGenerationPostAnalysisResolutionPlan::execute(...)`
    现在只保留：
    - resolved analysis outcome -> terminal owner handoff
    不再在 owner 外部保留最后的 success/failure terminal persistence 支路

  - 因此旧 free helper：
    - `fail_batch_generation_after_analysis(...)`
    - `persist_batch_generation_post_generation_success_outcome(...)`
    已从生产路径移除

  这条 seam 的意义不只是“最后两个 helper 被收掉”。它真正回答的是：
  batch runtime lane 在 follow-up analysis 已经得出结果之后，到底由哪个 Rust owner
  负责把
  `quality-gate route or fail stop -> persist terminal outcome`
  推进到最终 post-analysis result。现在这条 duplicate 已被删掉，batch runtime
  post-analysis terminal lane 在 ownership 上又向前收了一层。

  对 Phase 5 的价值同样直接：
  - cutover 审计时更容易回答“batch runtime post-analysis terminal 到底由哪个
    Rust owner 提供 contract”
  - quality-gate handoff / failed stop persistence / success progression
    persistence 现在共享更连续的 owner 链
  - fallback shrink / rollback clarity / stronger smoke 在 batch runtime
    post-analysis lane 上又少了一条隐藏在 resolution 邻层的 local terminal hop

  这条 seam 已通过：
  - `cargo fmt --manifest-path "backend-rs/Cargo.toml"`
  - `cargo test chapter_batch_generation_runtime_state_service --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-runtime-post-analysis-terminal-owner" -- --nocapture`
  - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-runtime-post-analysis-terminal-owner"`

### 2026-06-05 阶段补充：batch runtime active-path 已开始清理 owner 邻接 free helper

当前 `chapter_batch_generation` 的 Phase 5 Rust 收口，已经不只是在建立
新的 owner struct，也开始把仍残留在 active runtime path 上的 owner 邻接
free helper 一并消掉，避免生产链路继续保留 Python 时代的“owner 已存在、
caller 仍穿过薄 wrapper”形状。

1. **attempt-input active-path helper 已收回 owner**
   - `BatchGenerationAttemptInputPlan::prepare(...)` 现在直接完成：
     - persisted runtime snapshot restore
     - compat-option recovery
     - prompt override projection
     - provider payload preparation
   - active path 不再继续调用
     `resolve_runtime_compat_options_for_batch_generation_step(...)`

2. **post-analysis terminal success helper 已收回 owner**
   - `BatchGenerationPostAnalysisTerminalPlan::persist_post_generation_success(...)`
     现在直接完成：
     - persisted runtime snapshot load
     - recent story-repair quality summary refresh
     - refreshed runtime-state persistence
     - success progression persistence
   - active path 不再继续调用
     `refresh_batch_generation_runtime_story_repair_state(...)`

3. 这轮 seam 的价值不是“少了两个 helper”本身，而是：
   - batch runtime active path 进一步压成连续的 Rust-owned chain
   - cutover 审计时更容易判断哪一段仍在 owner 外部重放状态派生
   - fallback shrink / rollback reasoning 时，生产路径上的 owner hop 更少，
     证据更集中

4. 因此当前 `chapter_batch_generation` runtime lane 的补充 stop-rule 再加两条：
   - **不要**在 `BatchGenerationAttemptInputPlan` 已经拥有
     persisted compat restore + prompt/provider handoff 后，仍保留只被
     active path 单点调用的 compat wrapper helper
   - **不要**在 `BatchGenerationPostAnalysisTerminalPlan` 已经拥有
     post-analysis terminal success persistence 后，仍保留只被 active path
     单点调用的 runtime story-repair refresh wrapper helper

5. 这说明当前模块级迁移包已经开始进入下一层提速阶段：
   - 不只是新增 owner contract
   - 还要同步清理 owner 已就位但 active path 仍残留的薄 wrapper
   - 后续优先继续留在同一条 `chapter_batch_generation` runtime lane，
     直到 active path 的 owner 链连续性明显下降收益，再转向
     `chapter_single_generation` 或 route/fallback 收缩包

6. 这条 seam 已通过：
   - `cargo fmt --manifest-path "backend-rs/Cargo.toml"`
   - `cargo test chapter_batch_generation_runtime_state_service --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-runtime-next-owner" -- --nocapture`
   - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-runtime-next-owner"`

### 2026-06-05 阶段补充：analysis completion lane 也开始清理 owner 外 snapshot helper

当前 `chapter_batch_generation` 的 Phase 5 Rust 收口，已经继续往
follow-up analysis completion 这条子链推进，不再只停留在
attempt-input / post-analysis terminal 的 active-path helper 清理。

1. **analysis completion current-quality snapshot 已收回 owner**
   - `BatchGenerationAnalysisCompletionPersistencePlan` 现在直接完成：
     - latest plot analysis load
     - current chapter quality summary / latest metrics build
     - persisted runtime context reload
     - current-quality runtime snapshot materialization
     - analysis-completed snapshot persistence
   - production path 不再继续调用
     `build_batch_generation_current_chapter_quality_runtime_snapshot(...)`

2. 这条 seam 的价值在于：
   - follow-up analysis completion lane 又少了一条 owner 外状态派生 hop
   - current-quality snapshot 不再以单次 free helper 形式存在于 owner 外
   - cutover 审计时更容易回答“analysis completed 后的 runtime quality
     snapshot 到底由哪个 Rust owner 提供 contract”

3. 因此当前 `chapter_batch_generation` analysis lane 的补充 stop-rule 再加一条：
   - **不要**在 `BatchGenerationAnalysisCompletionPersistencePlan`
     已经拥有 analysis-completed persistence 和 current-quality snapshot
     handoff 后，仍保留只被同一条生产链单点调用的 current-quality
     snapshot rebuild helper

4. 这说明当前模块级迁移包的推进方式已经更清晰：
   - 先把连续 owner contract 建起来
   - 再持续收掉 owner 邻接、单点调用、只负责重建状态的 free helper
   - 后续仍优先留在 `chapter_batch_generation` runtime / analysis lane，
     继续把高收益 hop 压成连续的 Rust-owned chain

5. 这条 seam 已通过：
   - `cargo fmt --manifest-path "backend-rs/Cargo.toml"`
   - `cargo test chapter_batch_generation_runtime_state_service --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-runtime-analysis-completion-owner" -- --nocapture`
   - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-runtime-analysis-completion-owner"`

### 2026-06-05 阶段补充：analysis attempt lane 也开始把 prepared/fallback 编排收回 owner

当前 `chapter_batch_generation` 的 Phase 5 Rust 收口，已经继续沿同一条
runtime / analysis lane 推进到 follow-up analysis attempt 这一段，不再让
prepared analysis 与 fallback analysis 的分支继续裸露在外层 attempt body。

1. **analysis attempt prepared/fallback 编排已收回 owner**
   - `BatchGenerationAnalysisAttemptPlan` 现在直接完成：
     - initial analysis-started snapshot persistence
     - prepared chapter-analysis execution attempt
     - prepared-path started snapshot refresh with `analysis_task_id`
     - fallback follow-up analysis execution
     - resolution owner handoff
   - `execute()` 主体不再直接保留 prepared vs fallback 分支编排

2. 这条 seam 的价值在于：
   - follow-up analysis attempt lane 又少了一层 owner 外 orchestration
   - prepared analysis 与 fallback analysis 的切换不再散落在 caller body
   - cutover 审计时更容易回答“analysis attempt 到底由哪个 Rust owner
     负责串起 started snapshot、prepared task handoff、fallback 执行、
     resolution handoff”

3. 因此当前 `chapter_batch_generation` analysis lane 的补充 stop-rule 再加一条：
   - **不要**在 `BatchGenerationAnalysisAttemptPlan`
     已经拥有 started snapshot persistence、prepared analysis handoff 与
     resolution-owner handoff 后，仍保留 prepared vs fallback analysis 的
     caller-local orchestration 形状

4. 这说明当前模块级迁移包的推进策略进一步稳定：
   - 不是只删 free helper
   - 也要持续把 owner 已就位、但 caller 仍保留分支编排的生产链路收回
   - 后续仍优先继续留在 `chapter_batch_generation` runtime / analysis lane，
     直到 analysis 子链的连续 owner 收益明显下降，再切去下一包

5. 这条 seam 已通过：
   - `cargo fmt --manifest-path "backend-rs/Cargo.toml"`
   - `cargo test chapter_batch_generation_runtime_state_service --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-runtime-analysis-attempt-next-owner" -- --nocapture`
   - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-runtime-analysis-attempt-next-owner"`

### 2026-06-05 阶段补充：post-generation lane 也开始去掉中间 result-forwarding owner

当前 `chapter_batch_generation` 的 Phase 5 Rust 收口，已经继续沿同一条
runtime / analysis lane 推进到 post-generation 这一段，不再让
follow-up analysis 结果先经过一个只负责 success/failure 转发的中间 owner。

1. **post-generation analysis terminal handoff 已收回 owner**
   - `BatchGenerationPostGenerationPlan` 现在直接完成：
     - follow-up analysis execution
     - analysis outcome branch resolution
     - post-analysis terminal handoff
   - production path 不再继续经过
     `BatchGenerationPostAnalysisResolutionPlan`

2. 这条 seam 的价值在于：
   - post-generation lane 又少了一层 owner hop
   - analysis result 到 terminal persistence 的责任链更短
   - cutover 审计时更容易回答“post-generation 到底由哪个 Rust owner
     串起 analysis workflow、terminal branch 选择、quality-gate /
     success persistence handoff”

3. 因此当前 `chapter_batch_generation` post-generation lane 的补充 stop-rule 再加一条：
   - **不要**在 `BatchGenerationPostGenerationPlan`
     已经拥有 follow-up analysis boundary 与 terminal handoff 后，仍保留
     一个只负责转发 `Result<Option<Value>, String>` 到 terminal owner 的
     中间 resolution owner

4. 这说明当前模块级迁移包的推进已经不只是 analysis attempt 子链收口：
   - post-generation 主链也开始同步压缩 owner hop
   - 后续仍优先继续留在 `chapter_batch_generation` runtime / analysis lane，
     直到这一条 active chain 的连续 owner 收益明显下降，再转向下一包

5. 这条 seam 已通过：
   - `cargo fmt --manifest-path "backend-rs/Cargo.toml"`
   - `cargo test chapter_batch_generation_runtime_state_service --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-runtime-post-generation-next-owner" -- --nocapture`
   - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-runtime-post-generation-next-owner"`

### 2026-06-05 阶段补充：success lane 也开始去掉中间 post-generation owner

当前 `chapter_batch_generation` 的 Phase 5 Rust 收口，已经继续沿同一条
runtime / analysis lane 推进到 success-attempt 这一段，不再让
post-write guard 通过后又额外 materialize 一个单调用的 post-generation owner。

1. **success lane 的 post-generation handoff 已收回 owner**
   - `BatchGenerationSuccessAttemptPlan` 现在直接完成：
     - post-write guard resolution
     - follow-up analysis execution
     - analysis outcome branch resolution
     - post-analysis terminal handoff
   - production path 不再继续经过
     `BatchGenerationPostGenerationPlan`

2. 这条 seam 的价值在于：
   - success lane 又少了一层 owner hop
   - post-write guard 之后到 terminal persistence 的责任链更短
   - cutover 审计时更容易回答“success-attempt 到底由哪个 Rust owner
     串起 post-write guard、analysis workflow、terminal branch 选择、
     quality-gate / success persistence handoff”

3. 因此当前 `chapter_batch_generation` success lane 的补充 stop-rule 再加一条：
   - **不要**在 `BatchGenerationSuccessAttemptPlan`
     已经拥有 post-write guard、follow-up analysis 与 terminal handoff 后，
     仍保留一个只负责单次 success-chain 转发的
     `BatchGenerationPostGenerationPlan`

4. 这说明当前模块级迁移包的推进仍然保持高收益：
   - 不只是 analysis 子链继续压缩
   - success 主链也继续同步压缩 owner hop
   - 后续仍优先继续留在 `chapter_batch_generation` runtime / analysis lane，
     直到 active path 的连续 owner 收益明显下降，再转向下一包

5. 这条 seam 已通过：
   - `cargo fmt --manifest-path "backend-rs/Cargo.toml"`
   - `cargo test chapter_batch_generation_runtime_state_service --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-runtime-success-lane-next-owner" -- --nocapture`
   - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-runtime-success-lane-next-owner"`

### 2026-06-05 阶段补充：quality-gate lane 也开始收回 retry-budget context load

当前 `chapter_batch_generation` 的 Phase 5 Rust 收口，已经继续沿同一条
runtime / analysis lane 推进到 quality-gate 这一段，不再让
post-analysis terminal owner 先代替 quality-gate owner 读取 retry-budget context。

1. **quality-gate retry-budget context 已收回 owner**
   - `BatchGenerationQualityGateResolutionPlan` 现在直接完成：
     - persisted snapshot load
     - retry-budget context load
     - terminal semantics parsing
     - retry/manual-review routing
   - `BatchGenerationPostAnalysisTerminalPlan::resolve_analysis_success_outcome(...)`
     不再自己先读取 task retry counters

2. 这条 seam 的价值在于：
   - quality-gate lane 又少了一段 owner 外状态加载
   - terminal owner 到 quality-gate routing 的责任边界更清晰
   - cutover 审计时更容易回答“quality-gate 到底由哪个 Rust owner
     负责读取 retry budget、解析 terminal semantics、并推进 routing”

3. 因此当前 `chapter_batch_generation` quality-gate lane 的补充 stop-rule 再加一条：
   - **不要**在 `BatchGenerationQualityGateResolutionPlan`
     已经拥有 terminal semantics parsing 与 routing persistence 后，仍保留
     由 terminal owner 预先加载 retry-budget context 的 caller-local 形状

4. 这说明当前模块级迁移包仍在沿 active path 连续压缩：
   - success lane 已经压缩 owner hop
   - quality-gate lane 现在开始继续收回 owner 所需上下文
   - 后续仍优先继续留在 `chapter_batch_generation` runtime / analysis lane，
     直到 active path 的连续 owner 收益明显下降，再转向下一包

5. 这条 seam 已通过：
   - `cargo fmt --manifest-path "backend-rs/Cargo.toml"`
   - `cargo test chapter_batch_generation_runtime_state_service --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-runtime-quality-gate-next-owner" -- --nocapture`
   - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-runtime-quality-gate-next-owner"`

### 2026-06-05 阶段补充：generation-attempt lane 也开始去掉中间 success owner

当前 `chapter_batch_generation` 的 Phase 5 Rust 收口，已经继续沿同一条
runtime / analysis lane 推进到 generation-attempt success 这一段，不再让
生成成功后又额外 materialize 一个单调用的 success-attempt owner。

1. **generation-attempt 的 success chain 已收回 owner**
   - `PreparedBatchGenerationGenerationAttempt` 现在直接完成：
     - generated result success branch handoff
     - post-write guard resolution
     - follow-up analysis execution
     - analysis outcome branch resolution
     - post-analysis terminal handoff
   - production path 不再继续经过
     `BatchGenerationSuccessAttemptPlan`

2. 这条 seam 的价值在于：
   - generation-attempt lane 又少了一层 owner hop
   - generated result 之后到 terminal persistence 的责任链更短
   - cutover 审计时更容易回答“generation-attempt 到底由哪个 Rust owner
     串起 generated result、post-write guard、analysis workflow、
     terminal branch 选择、quality-gate / success persistence handoff”

3. 因此当前 `chapter_batch_generation` generation-attempt lane 的补充 stop-rule 再加一条：
   - **不要**在 `PreparedBatchGenerationGenerationAttempt`
     已经拥有 generated result success branch 与 downstream analysis /
     terminal handoff 后，仍保留一个只负责单次 success-chain 转发的
     `BatchGenerationSuccessAttemptPlan`

4. 这说明当前模块级迁移包仍然保持 active path 高收益压缩：
   - success lane 已经压缩了 post-generation owner hop
   - generation-attempt lane 现在继续压缩 success owner hop
   - 后续仍优先继续留在 `chapter_batch_generation` runtime / analysis lane，
     直到 active path 的连续 owner 收益明显下降，再转向下一包

5. 这条 seam 已通过：
   - `cargo fmt --manifest-path "backend-rs/Cargo.toml"`
   - `cargo test chapter_batch_generation_runtime_state_service --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-runtime-generation-success-next-owner" -- --nocapture`
   - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-runtime-generation-success-next-owner"`

### 2026-06-05 阶段补充：step lane 也开始去掉中间 prepared generation-attempt owner

当前 `chapter_batch_generation` 的 Phase 5 Rust 收口，已经继续沿同一条
runtime / analysis lane 推进到 step-execution 这一段，不再让章节 step
在已经拥有 loaded chapter 与 retry state 之后，又额外 materialize 一个
单调用的 prepared generation-attempt owner。

1. **step lane 的 generation-attempt lifecycle 已收回 owner**
   - `PreparedBatchGenerationStepExecution` 现在直接完成：
     - chapter-started persistence
     - prerequisite gate
     - attempt-input prepare
     - generation call
     - success / failure routing
     - post-success analysis / terminal handoff
   - production path 不再继续经过
     `PreparedBatchGenerationGenerationAttempt`

2. 这条 seam 的价值在于：
   - step lane 又少了一层 owner hop
   - loaded chapter 之后到 terminal persistence 的责任链更短
   - cutover 审计时更容易回答“step-execution 到底由哪个 Rust owner
     串起 chapter-started、prerequisite、attempt-input、generation、
     success/failure branch 与 terminal handoff”

3. 因此当前 `chapter_batch_generation` step lane 的补充 stop-rule 再加一条：
   - **不要**在 `PreparedBatchGenerationStepExecution`
     已经拥有 loaded chapter、retry carry 与 downstream generation /
     analysis / terminal handoff 后，仍保留一个只负责单次
     generation-attempt 转发的 `PreparedBatchGenerationGenerationAttempt`

4. 这说明当前模块级迁移包仍然保持 active path 高收益压缩：
   - generation-attempt lane 已经压缩了 success owner hop
   - step lane 现在继续压缩 prepared generation-attempt owner hop
   - 后续仍优先继续留在 `chapter_batch_generation` runtime / analysis lane，
     直到 active path 的连续 owner 收益明显下降，再转向下一包

5. 这条 seam 已通过：
   - `cargo fmt --manifest-path "backend-rs/Cargo.toml"`
   - `cargo test chapter_batch_generation_runtime_state_service --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-runtime-step-generation-next-owner" -- --nocapture`
   - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-runtime-step-generation-next-owner"`

### 2026-06-05 阶段补充：runtime lifecycle lane 也开始去掉中间 step owner

当前 `chapter_batch_generation` 的 Phase 5 Rust 收口，已经继续沿同一条
runtime / analysis lane 推进到 runtime lifecycle 这一段，不再让 runtime
lifecycle 在已经拥有 chapter iteration 与 downstream prepared-step lifecycle
之后，又额外 materialize 一个单调用的 step-execution owner。

1. **runtime lifecycle 的 step chain 已收回 owner**
   - `BatchGenerationRuntimeLifecyclePlan` 现在直接完成：
     - chapter iteration
     - step preparation retry carry
     - prepared-step execution
     - continue / stop progression handoff
   - production path 不再继续经过
     `BatchGenerationStepExecutionPlan`

2. 这条 seam 的价值在于：
   - runtime lifecycle lane 又少了一层 owner hop
   - chapter iteration 之后到 stop/continue handoff 的责任链更短
   - cutover 审计时更容易回答“runtime lifecycle 到底由哪个 Rust owner
     串起 chapter iteration、step prepare、prepared-step execute、
     progression handoff”

3. 因此当前 `chapter_batch_generation` lifecycle lane 的补充 stop-rule 再加一条：
   - **不要**在 `BatchGenerationRuntimeLifecyclePlan`
     已经拥有 chapter iteration 与 downstream step lifecycle 后，仍保留一个
     只负责单次 step prepare / execute 转发的
     `BatchGenerationStepExecutionPlan`

4. 这说明当前模块级迁移包仍然保持 active path 高收益压缩：
   - step lane 已经压缩了 prepared generation-attempt owner hop
   - lifecycle lane 现在继续压缩 step owner hop
   - 后续仍优先继续留在 `chapter_batch_generation` runtime / analysis lane，
     直到 active path 的连续 owner 收益明显下降，再转向下一包

5. 这条 seam 已通过：
   - `cargo fmt --manifest-path "backend-rs/Cargo.toml"`
   - `cargo test chapter_batch_generation_runtime_state_service --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-runtime-lifecycle-step-next-owner" -- --nocapture`
   - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-batch-runtime-lifecycle-step-next-owner"`

### 2026-06-05 阶段补充：切到 single-generation background write lane，去掉中间 workflow-start / launch wrapper

当前 `chapter_batch_generation` active path 的高收益 owner-hop 压缩已经基本连续收完，
继续停留在同一文件里做低收益参数搬运的收益开始下降。因此 Phase 5 这一步明确切到
相邻的 `chapter_single_generation` background write lane，继续按整块模块迁移的方式
推进 Rust owner 收口，而不是退回微切片。

1. **single-generation background workflow entry 已收回 launch persistence / dispatch 链**
   - `SingleGenerationBackgroundWorkflowEntry` 现在直接完成：
     - chapter target load
     - existing background-task payload short-circuit
     - launch persistence plan prepare
     - persist new task + startup snapshot
     - runtime dispatch handoff
   - production path 不再继续经过：
     - `PreparedSingleGenerationBackgroundLaunch`
     - `PreparedSingleGenerationBackgroundWorkflowStart`

2. 这条 seam 的价值在于：
   - single background lane 一次去掉了两层只做单次转发的 owner wrapper
   - target lookup 之后到 persistence / dispatch 的责任链明显更短
   - cutover 审计时更容易回答“single-generation background write
     到底由哪个 Rust owner 串起 existing-task short-circuit、launch persistence、
     startup snapshot persistence 与 runtime dispatch”

3. 因此当前 `chapter_single_generation` background write lane 的补充 stop-rule 加一条：
   - **不要**在 `SingleGenerationBackgroundWorkflowEntry`
     已经拥有 existing payload branch 与 downstream launch persistence /
     dispatch chain 后，仍保留只负责单次 handoff 的
     `PreparedSingleGenerationBackgroundWorkflowStart`
     或 `PreparedSingleGenerationBackgroundLaunch`

4. 这说明 Phase 5 的推进策略已经开始按“相邻高收益模块包”切换，而不是困在单一路径：
   - `chapter_batch_generation` active path 已经完成一轮连续 owner-hop 压缩
   - 当前开始转向 `chapter_single_generation` background write lane
   - 后续优先继续在 `chapter_single_generation` 或相邻 `chapters` / workflow
     模块里寻找同等级的整段 owner 收口，再决定下一块完整迁移包

5. 这条 seam 已通过：
   - `cargo fmt --manifest-path "backend-rs/Cargo.toml"`
   - `cargo test chapter_single_generation_write_workflow_service --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-single-generation-write-workflow-next-owner" -- --nocapture`
   - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-single-generation-write-workflow-next-owner"`

### 2026-06-05 阶段补充：继续沿 single-generation runtime lane 去掉中间 runtime driver / terminal wrapper

当前已经切到 `chapter_single_generation` 相邻模块包后，下一条高收益 seam 没有再回到
`chapter_batch_generation` 做低收益搬运，而是继续沿 single-generation runtime lane
收回真正还在 production path 上的中间 owner hop。

1. **single-generation runtime lifecycle 已收回 terminal persistence 链**
   - `SingleGenerationRuntimeLifecyclePlan` 现在直接完成：
     - preparation persistence
     - owned runtime generation execute
     - follow-up analysis routing
     - completed / manual-review / failed persistence
   - production path 不再继续经过：
     - `SingleGenerationRuntimeDriver`
     - `SingleGenerationTerminalPersistencePlan`

2. 这条 seam 的价值在于：
   - single runtime lane 又少了两层只做单次转发的 owner wrapper
   - runtime execute 之后到 terminal persistence 的责任链明显更短
   - cutover 审计时更容易回答“single-generation runtime
     到底由哪个 Rust owner 串起 preparing persistence、runtime execute、
     analysis routing 与 terminal persistence”

3. 因此当前 `chapter_single_generation` runtime lane 的补充 stop-rule 再加一条：
   - **不要**在 `SingleGenerationRuntimeLifecyclePlan`
     已经拥有 runtime input、preparing persistence、analysis routing 与
     downstream terminal persistence 后，仍保留只负责单次 handoff 的
     `SingleGenerationRuntimeDriver`
     或 `SingleGenerationTerminalPersistencePlan`

4. 这说明当前模块级迁移包已经继续在 single-generation 邻域形成连续 owner 收口：
   - background write lane 已经压缩了 workflow-start / launch wrapper
   - runtime lane 现在继续压缩 driver / terminal wrapper
   - 后续优先继续在 `chapter_single_generation` stream / prepare /
     runtime 邻域里寻找下一个同等级的整段 owner 收口，再决定是否切换模块包

5. 这条 seam 已通过：
   - `cargo fmt --manifest-path "backend-rs/Cargo.toml"`
   - `cargo test chapter_single_generation_runtime_state_service --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-single-generation-runtime-lifecycle-next-owner" -- --nocapture`
   - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-single-generation-runtime-lifecycle-next-owner"`

### 2026-06-05 阶段补充：继续沿 single-generation stream success lane 去掉中间 success-follow-up wrapper

当前 single-generation 模块包已经连续收回了 background write lane 和 runtime lane，
因此下一步没有切走模块，而是继续沿 stream success lane 把真正仍在 production path
上的中间 success wrapper 收回 owner。

1. **single-generation stream completion owner 已收回 success follow-up / emission 链**
   - `SingleGenerationStreamCompletionProjection` 现在直接完成：
     - generated result success follow-up analysis
     - latest-quality history update
     - quality metrics / quality-gate / analysis-started event projection
     - response payload assembly
     - SSE emission handoff
   - production path 不再继续经过：
     - `SingleGenerationStreamSuccessFollowUpProjection`

2. 这条 seam 的价值在于：
   - single stream success lane 又少了一层只做单次转发的 owner wrapper
   - generated result 之后到 SSE emission 的责任链明显更短
   - cutover 审计时更容易回答“single-generation stream success
     到底由哪个 Rust owner 串起 follow-up analysis、quality projection、
     result payload 与 event emission”

3. 因此当前 `chapter_single_generation` stream success lane 的补充 stop-rule 再加一条：
   - **不要**在 `SingleGenerationStreamCompletionProjection`
     已经拥有 generated result、follow-up analysis 与 downstream event /
     response projection后，仍保留只负责单次 handoff 的
     `SingleGenerationStreamSuccessFollowUpProjection`

4. 这说明当前模块级迁移包已经继续在 single-generation 邻域形成第三条连续 owner 收口：
   - background write lane 已经压缩了 workflow-start / launch wrapper
   - runtime lane 已经压缩了 driver / terminal wrapper
   - stream success lane 现在继续压缩 success-follow-up wrapper
   - 后续优先继续在 `chapter_single_generation` stream / prepare 邻域里找
     下一条同等级的整段 owner 收口，再决定是否切换模块包

5. 这条 seam 已通过：
   - `cargo fmt --manifest-path "backend-rs/Cargo.toml"`
   - `cargo test chapter_single_generation_stream_workflow_service --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-single-generation-stream-success-next-owner" -- --nocapture`
   - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-single-generation-stream-success-next-owner"`

### 2026-06-05 阶段补充：继续沿 single-generation prepare lane 去掉中间 prepared-execution wrapper

当前 single-generation 模块包已经连续收回了 background write lane、runtime lane
和 stream success lane，因此下一步没有切走模块，而是继续沿 prepare lane
把真正仍在 production path 上的中间 prepared-execution wrapper 收回 owner。

1. **single-generation prepare owner 已收回 restored launch 前的 prepared execution 链**
   - `prepare_single_chapter_generation_request(...)` 现在直接产出：
     `PreparedSingleChapterGenerationRestoredRuntimeLaunch`
   - `prepare_single_chapter_generation_request_from_target(...)` 现在也直接产出：
     `PreparedSingleChapterGenerationRestoredRuntimeLaunch`
   - prepare lane 现在同一个 owner 直接完成：
     - request validation
     - chapter target load / from-target ownership
     - request runtime-state assembly
     - execution-config preparation
     - restored runtime-state load
     - startup snapshot + runtime launch input materialization
   - production path 不再继续经过：
     - `PreparedSingleChapterGenerationExecution`

2. 这条 seam 的价值在于：
   - single prepare lane 又少了一层只做单次转发的 owner wrapper
   - request/target 进入 runtime restore 与 launch materialization 的责任链更短
   - cutover 审计时更容易回答“single-generation prepare
     到底由哪个 Rust owner 串起 request validation、runtime restore、
     startup snapshot 与 runtime launch input”

3. 因此当前 `chapter_single_generation` prepare lane 的补充 stop-rule 再加一条：
   - **不要**在 prepare boundary 已经拥有 request validation、target ownership、
     request runtime-state、restore runtime-state 与 launch materialization 后，
     仍保留只负责把
     `chapter_target + execution_input + request_runtime_state`
     再 handoff 一次的 `PreparedSingleChapterGenerationExecution`

4. 这说明当前模块级迁移包已经继续在 single-generation 邻域形成第四条连续 owner 收口：
   - background write lane 已经压缩了 workflow-start / launch wrapper
   - runtime lane 已经压缩了 driver / terminal wrapper
   - stream success lane 已经压缩了 success-follow-up wrapper
   - prepare lane 现在继续压缩 prepared-execution wrapper
   - 后续优先继续在 `chapter_single_generation` prepare / stream / runtime
     邻域里找下一个同等级的整段 owner 收口，再决定是否切换模块包

5. 这条 seam 已通过：
   - `cargo fmt --manifest-path "backend-rs/Cargo.toml"`
   - `cargo test chapter_single_generation_prepare_service --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-single-generation-prepare-next-owner" -- --nocapture`
   - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-single-generation-prepare-next-owner"`

### 2026-06-05 阶段补充：继续沿 single-generation runtime lane 去掉内部 persistence-plan wrapper 集群

当前 single-generation 模块包已经连续收回了 background write lane、runtime
driver/terminal lane、stream success lane 和 prepare lane，因此下一步没有切走
模块，而是继续沿 runtime lane 深挖，把仍留在 production path 内部的
persistence-plan wrapper 集群继续收回 owner。

1. **single-generation runtime lifecycle 已收回内部 terminal persistence plan 集群**
   - `SingleGenerationRuntimeLifecyclePlan` 现在直接通过更窄 helper 完成：
     - preparing task/snapshot persistence
     - runtime generation execute
     - follow-up analysis routing
     - completed / failed / manual-review terminal persistence
   - production path 不再继续经过：
     - `SingleGenerationPreparationPersistencePlan`
     - `SingleGenerationCompletionPersistencePlan`
     - `SingleGenerationFailurePersistencePlan`
     - `SingleGenerationManualReviewPersistencePlan`

2. 这条 seam 的价值在于：
   - single runtime lane 又少了一组只做单次终态写入转发的内部 owner wrapper
   - runtime execute 之后到 checkpoint/runtime-state persistence 的责任链更短
   - cutover 审计时更容易回答“single-generation runtime
     到底由哪个 Rust owner 串起 analysis routing、checkpoint projection、
     runtime-state patch 与 terminal persistence”

3. 因此当前 `chapter_single_generation` runtime lane 的补充 stop-rule 再加一条：
   - **不要**在 `SingleGenerationRuntimeLifecyclePlan`
     已经拥有 runtime execute、analysis routing 和 terminal persistence contract 后，
     仍保留只负责单次 preparing/completed/failed/manual-review 写入 handoff 的
     `SingleGenerationPreparationPersistencePlan`
     `SingleGenerationCompletionPersistencePlan`
     `SingleGenerationFailurePersistencePlan`
     或 `SingleGenerationManualReviewPersistencePlan`

4. 这说明当前模块级迁移包已经继续在 single-generation 邻域形成第五条连续 owner 收口：
   - background write lane 已经压缩了 workflow-start / launch wrapper
   - runtime lane 已经压缩了 driver / terminal wrapper
   - stream success lane 已经压缩了 success-follow-up wrapper
   - prepare lane 已经压缩了 prepared-execution wrapper
   - runtime lane 现在继续压缩内部 persistence-plan wrapper 集群
   - 后续优先继续在 `chapter_single_generation` runtime / prepare / stream /
     write 邻域里找下一个同等级的整段 owner 收口，再决定是否切换模块包

5. 这条 seam 已通过：
   - `cargo fmt --manifest-path "backend-rs/Cargo.toml"`
   - `cargo test chapter_single_generation_runtime_state_service --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-single-generation-runtime-persistence-owner" -- --nocapture`
   - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-single-generation-runtime-persistence-owner"`

### 2026-06-05 阶段补充：继续沿 single-generation write lane 去掉中间 launch-persistence wrapper

当前 single-generation 模块包已经连续收回了 background workflow-start lane、
runtime lane、stream success lane、prepare lane 和 runtime 内部 persistence-plan
lane，因此下一步没有切回 `chapter_batch_generation` 做低收益搬运，而是继续沿
同一个 single-generation background write lane 把仍留在 production path 上的
launch-persistence wrapper 收回 owner。

1. **single-generation background workflow entry 已直接消费 launch parts**
   - `SingleGenerationBackgroundWorkflowEntry::Launch(...)` 现在直接持有：
     `PreparedSingleGenerationBackgroundLaunchParts`
   - workflow entry 现在同一个 owner 直接完成：
     - chapter target lookup
     - existing-task payload short-circuit
     - restored runtime launch preparation
     - background task-seed / startup snapshot / response payload /
       runtime-input materialization
     - persist task
     - persist startup snapshot
     - dispatch runtime
   - production path 不再继续经过：
     - `SingleGenerationBackgroundLaunchPersistencePlan`

2. 这条 seam 的价值在于：
   - single background write lane 又少了一层只做单次 handoff 的 owner wrapper
   - restored launch owner 进入 task/snapshot persistence 与 runtime dispatch
     的责任链更短
   - cutover 审计时更容易回答“single-generation background write
     到底由哪个 Rust owner 串起 restored launch、task seed、snapshot write
     与 runtime dispatch”

3. 因此当前 `chapter_single_generation` background write lane 的补充 stop-rule
   再加一条：
   - **不要**在 workflow-entry boundary 已经拥有 target lookup、
     existing-task payload short-circuit、restored launch prepare 和
     background launch-parts contract 后，仍保留只负责把
     `PreparedSingleGenerationBackgroundLaunchParts`
     再 handoff 一次的
     `SingleGenerationBackgroundLaunchPersistencePlan`

4. 这说明当前模块级迁移包已经继续在 single-generation 邻域形成第六条连续
   owner 收口：
   - background write lane 已经压缩了 workflow-start / launch wrapper
   - runtime lane 已经压缩了 driver / terminal wrapper
   - stream success lane 已经压缩了 success-follow-up wrapper
   - prepare lane 已经压缩了 prepared-execution wrapper
   - runtime lane 已经压缩了内部 persistence-plan wrapper 集群
   - background write lane 现在继续压缩 launch-persistence wrapper
   - 后续优先继续在 `chapter_single_generation` stream / write / runtime
     邻域里找下一个同等级的整段 owner 收口，再决定是否切换模块包

5. 这条 seam 已通过：
   - `cargo fmt --manifest-path "backend-rs/Cargo.toml"`
   - `cargo test chapter_single_generation_write_workflow_service --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-single-generation-write-launch-parts-owner" -- --nocapture`
   - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-single-generation-write-launch-parts-owner"`

### 2026-06-05 阶段补充：继续沿 single-generation stream lane 去掉中间 emission-plan wrapper

### 2026-06-05 阶段补充：继续沿 single-generation background write lane 去掉最后的 launch-parts persistence helper

当前 `chapter_single_generation` 模块包没有切回全仓统计或低收益 lane 清理，而是
继续沿同一个 single-generation background write production path，把在 launch-parts
owner 已经明确之后仍然留在邻居自由函数里的最后一段持久化/分发 handoff 收掉。

1. **single-generation background launch-parts owner 已直接承担 persistence / dispatch**
   - `PreparedSingleGenerationBackgroundLaunchParts` 现在直接拥有：
     `persist_and_dispatch(...)`
   - 同一个 owner 现在直接完成：
     - task insert active-model materialization
     - startup snapshot persist
     - runtime dispatch
     - response payload return
   - production path 不再继续经过：
     - `persist_and_dispatch_background_launch_parts(...)`

2. 这条 seam 的价值在于：
   - single-generation background write lane 又少了一层只做单次 handoff 的自由函数
   - launch-parts owner 进入 task/snapshot persistence 与 runtime dispatch
     的责任链更短
   - cutover 审计时更容易回答“single-generation background write 到底由哪个 Rust owner
     串起 task seed、snapshot write 与 runtime dispatch”

3. 因此当前 `chapter_single_generation` background write lane 的 stop-rule 再补一条：
   - **不要**在 launch-parts owner 已经同时持有 task-seed、startup snapshot、
     response payload 与 runtime input 后，仍然在邻居 write-workflow 中保留只负责把
     这些 parts 再 handoff 一次的自由 helper

4. 这说明当前模块级迁移包继续沿 single-generation 邻域形成又一条连续 owner 收口：
   - background workflow public-start 已收口
   - restored-launch materialization 已收口
   - runtime lifecycle / direct-generation-analysis 已收口
   - stream workflow public-start 已收口
   - background launch-parts persistence 现在也已收口
   - 后续优先继续在 `chapter_single_generation` stream / write / runtime 邻域里找
     下一条同等级 production seam，再决定是否切换模块包

5. 这条 seam 的验证命令：
   - `cargo test chapter_single_generation_prepare_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-background-launch-parts-persistence-owner" -- --nocapture`
   - `cargo test chapter_single_generation_write_workflow_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-background-launch-parts-persistence-owner" -- --nocapture`
   - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-background-launch-parts-persistence-owner"`

### 2026-06-05 阶段补充：继续沿 single-generation stream success lane 去掉 analysis-projection free helpers

当前 `chapter_single_generation` 模块包继续沿真实 production path 推进，没有切回
统计或 route 外围，而是把 single stream 成功链里仍残留的
`analysis -> quality event -> response payload` 自由 helper 链收回 owner。

1. **single-generation stream analysis owner 已直接承担 success projection**
   - `SingleGenerationStreamAnalysisOutcome` 现在直接拥有：
     - `from_generated_result(...)`
     - `run_follow_up_analysis(...)`
     - `quality_metrics_event(...)`
     - `quality_gate_event(...)`
     - `analysis_started_event(...)`
     - `response_payload(...)`
   - 同一个 owner 现在直接完成：
     - follow-up analysis execution
     - latest quality history sync trigger
     - quality SSE event projection
     - analysis-started event projection
     - terminal response payload projection
   - production path 不再继续经过：
     - `run_single_generation_stream_follow_up_analysis(...)`
     - `build_single_generation_stream_quality_metrics_event(...)`
     - `build_single_generation_stream_quality_gate_event(...)`
     - `build_single_generation_stream_analysis_started_event(...)`
     - `build_single_generation_stream_result_payload(...)`

2. 这条 seam 的价值在于：
   - single stream success lane 又少了一组只做单次 handoff 的 free helpers
   - analysis owner 到 completion owner 的责任链更短
   - cutover 审计时更容易回答“single-generation stream 成功后到底由哪个 Rust owner
     串起 analysis、quality event 和 terminal payload”

3. 因此当前 `chapter_single_generation` stream success lane 的 stop-rule 再补一条：
   - **不要**在 completion owner 已经只消费一个 analysis outcome 时，仍保留一组
     仅负责把同一份 analysis outcome 再投影成 quality event、analysis-started event
     和 response payload 的邻居 free helpers

4. 这说明当前模块级迁移包继续在 single-generation 邻域形成连续 owner 收口：
   - background workflow public-start 已收口
   - restored-launch materialization 已收口
   - background launch-parts persistence 已收口
   - runtime lifecycle / direct-generation-analysis 已收口
   - stream success emission 已收口
   - stream success analysis-projection 现在也已收口

5. 这条 seam 的验证命令：
   - `cargo test chapter_single_generation_stream_workflow_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-stream-success-analysis-projection-owner" -- --nocapture`
   - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-stream-success-analysis-projection-owner"`

### 2026-06-05 阶段补充：继续沿 single-generation route edge 去掉本地 request-builder handoff

当前 `chapter_single_generation` 模块包继续沿真实 transport -> workflow 边界推进，
没有回到 service 内部做低收益整理，而是把单章 background/stream route 里仍然残留的
本地 request-builder handoff 收回到 workflow public-start owner。

1. **single-generation route 现在直接把 raw payload 交给 workflow owner**
   - `chapter_generation_routes.rs` 不再本地执行：
     - `build_single_chapter_generation_request_from_route_payload(...)`
   - background lane 现在直接通过：
     - `start_owned_single_generation_background_write_workflow_from_route_payload(...)`
   - stream lane 现在直接通过：
     - `create_single_generation_stream_workflow_from_route_payload(...)`
   - 两条 workflow 现在分别由：
     - `SingleGenerationBackgroundWorkflowRouteStart`
     - `SingleGenerationStreamWorkflowRouteStart`
     负责 route-payload -> workflow-request 的 owner handoff

2. 这条 seam 的价值在于：
   - route 进一步变薄，transport-only 边界更清晰
   - background/stream workflow owner 责任链更完整
   - cutover 审计时更容易回答“single-generation route 到底是 route 自己拼请求，
     还是已经直接交给 Rust workflow owner 统一接管”

3. 因此当前 `chapter_single_generation` route edge 的 stop-rule 再补一条：
   - **不要**在 background/stream workflow public-start owner 已经稳定存在时，
     仍在 route handler 本地保留一份重复的
     `route payload -> workflow request -> workflow start`
     handoff 链

4. 这说明当前 single-generation 模块包又补上一条更外层的 owner 收口：
   - route edge 已收口
   - background workflow public-start 已收口
   - background launch-parts persistence 已收口
   - restored-launch materialization 已收口
   - runtime lifecycle / direct-generation-analysis 已收口
   - stream success emission / analysis-projection 已收口

5. 这条 seam 的验证命令：
   - `cargo test chapter_single_generation_write_workflow_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-route-workflow-start-owner" -- --nocapture`
   - `cargo test chapter_single_generation_stream_workflow_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-route-workflow-start-owner" -- --nocapture`
   - `cargo test chapter_generation_routes --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-route-workflow-start-owner" -- --nocapture`
   - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-route-workflow-start-owner"`

当前 single-generation 模块包已经连续收回了 background write lane、runtime lane、
stream success lane、prepare lane、runtime 内部 persistence-plan lane 和
background launch-persistence lane，因此下一步没有切走模块，而是继续沿同一个
single-generation stream success lane 把仍留在 production path 上的
emission-plan wrapper 收回 owner。

1. **single-generation completion owner 已直接发射 ordered success events**
   - `SingleGenerationStreamCompletionProjection` 现在直接拥有：
     - completion message
     - ordered success event payloads
     - SSE success emission sequence
   - completion owner 现在同一个 owner 直接完成：
     - quality-metrics event projection
     - quality-gate event projection
     - result payload projection
     - analysis-started event projection
     - ordered SSE emit
   - production path 不再继续经过：
     - `SingleGenerationStreamSuccessEmissionPlan`

2. 这条 seam 的价值在于：
   - single stream success lane 又少了一层只做单次 handoff 的 owner wrapper
   - completion projection 进入 SSE emit 的责任链更短
   - cutover 审计时更容易回答“single-generation stream success
     到底由哪个 Rust owner 串起 completion semantics、ordered payload
     projection 与 terminal SSE emit”

3. 因此当前 `chapter_single_generation` stream lane 的补充 stop-rule 再加一条：
   - **不要**在 completion owner 已经拥有 completion message、
     quality/result/analysis-started payload contract 与 ordered emit
     sequence 后，仍保留只负责把这些 payload 再 handoff 一次的
     `SingleGenerationStreamSuccessEmissionPlan`

4. 这说明当前模块级迁移包已经继续在 single-generation 邻域形成第七条连续
   owner 收口：
   - background write lane 已经压缩了 workflow-start / launch wrapper
   - runtime lane 已经压缩了 driver / terminal wrapper
   - stream success lane 已经压缩了 success-follow-up wrapper
   - prepare lane 已经压缩了 prepared-execution wrapper
   - runtime lane 已经压缩了内部 persistence-plan wrapper 集群
   - background write lane 已经压缩了 launch-persistence wrapper
   - stream lane 现在继续压缩 emission-plan wrapper
   - 后续优先继续在 `chapter_single_generation` stream / runtime / write
     邻域里找下一个同等级的整段 owner 收口，再决定是否切换模块包

5. 这条 seam 已通过：
   - `cargo fmt --manifest-path "backend-rs/Cargo.toml"`
   - `cargo test chapter_single_generation_stream_workflow_service --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-single-generation-stream-emission-next-owner" -- --nocapture`
   - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-single-generation-stream-emission-next-owner"`

### 2026-06-05 阶段补充：继续沿 single-generation stream lane 去掉外层 lifecycle spawn wrapper

当前 single-generation 模块包已经连续收回了 background write lane、runtime lane、
stream success lane、prepare lane、runtime 内部 persistence-plan lane、
background launch-persistence lane 和 stream emission-plan lane，因此下一步
没有切走模块，而是继续沿同一个 single-generation stream lane 把仍留在
production path 外层的 lifecycle spawn wrapper 收回 owner。

1. **single-generation stream lifecycle owner 已直接串起 spawn -> runtime -> emit**
   - `SingleGenerationStreamLifecyclePlan` 现在直接拥有：
     - stream runtime launch input
     - progress tracker transport sequence
     - runtime execute
     - completion projection handoff
     - terminal success/error emit
   - stream lifecycle owner 现在同一个 owner 直接完成：
     - create stream channel
     - spawn background stream runtime
     - emit start/preparing/generating
     - execute runtime
     - emit completion or error
   - production path 不再继续经过：
     - `launch_owned_single_chapter_generation_stream(...)`

2. 这条 seam 的价值在于：
   - single stream lane 又少了一层只做单次 handoff 的 outer wrapper
   - prepare runtime input 进入 runtime execute 与 terminal emit 的责任链更短
   - cutover 审计时更容易回答“single-generation stream
     到底由哪个 Rust owner 串起 spawn、progress transport、runtime execute
     与 success/error emit”

3. 因此当前 `chapter_single_generation` stream lane 的补充 stop-rule 再加一条：
   - **不要**在 stream lane 已经拥有 runtime launch input、progress tracker、
     runtime execute、completion projection 与 terminal emit contract 后，
     仍保留只负责把这些步骤再 handoff 一次的
     `launch_owned_single_chapter_generation_stream(...)`

4. 这说明当前模块级迁移包已经继续在 single-generation 邻域形成第八条连续
   owner 收口：
   - background write lane 已经压缩了 workflow-start / launch wrapper
   - runtime lane 已经压缩了 driver / terminal wrapper
   - stream success lane 已经压缩了 success-follow-up wrapper
   - prepare lane 已经压缩了 prepared-execution wrapper
   - runtime lane 已经压缩了内部 persistence-plan wrapper 集群
   - background write lane 已经压缩了 launch-persistence wrapper
   - stream lane 已经压缩了 emission-plan wrapper
   - stream lane 现在继续压缩 outer lifecycle spawn wrapper
   - 后续优先继续在 `chapter_single_generation` stream / runtime / write
     邻域里找下一个同等级的整段 owner 收口，再决定是否切换模块包

5. 这条 seam 已通过：
   - `cargo fmt --manifest-path "backend-rs/Cargo.toml"`
   - `cargo test chapter_single_generation_stream_workflow_service --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-single-generation-stream-lifecycle-next-owner" -- --nocapture`
   - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-single-generation-stream-lifecycle-next-owner"`

### 2026-06-05 阶段补充：继续沿 single-generation prepare lane 去掉外层 prepare-entry wrapper

当前 single-generation 模块包已经连续收回了 background write lane、runtime lane、
stream success lane、prepare lane、runtime 内部 persistence-plan lane、
background launch-persistence lane、stream emission-plan lane和 stream lifecycle
lane，因此下一步没有切走模块，而是继续沿同一个 single-generation prepare lane
把仍留在 production path 外层的 prepare-entry wrapper 收回 owner。

1. **single-generation restored-launch owner 已直接承担 prepare entry**
   - `PreparedSingleChapterGenerationRestoredRuntimeLaunch` 现在直接拥有：
     - request validation
     - chapter-target lookup
     - from-target ownership
     - runtime-state restore
     - restored launch materialization
   - prepare owner 现在同一个 owner 直接完成：
     - `prepare(...)`
     - `prepare_from_target(...)`
     - stream lane 的 `into_runtime_launch_input()` handoff
   - production path 不再继续经过：
     - `prepare_single_chapter_generation_request(...)`
     - `prepare_single_chapter_generation_request_from_target(...)`
     - `prepare_owned_single_generation_runtime_launch_input(...)`

2. 这条 seam 的价值在于：
   - single prepare lane 又少了一层只做单次 handoff 的 outer wrapper
   - public prepare entry 进入 runtime restore 与 launch materialization 的责任链更短
   - cutover 审计时更容易回答“single-generation prepare
     到底由哪个 Rust owner 串起 request entry、target ownership、runtime restore
     与 restored launch materialization”

3. 因此当前 `chapter_single_generation` prepare lane 的补充 stop-rule 再加一条：
   - **不要**在 restored-launch owner 已经拥有 request validation、
     target ownership、runtime restore 与 launch materialization contract 后，
     仍保留只负责把这些步骤再 handoff 一次的
     `prepare_single_chapter_generation_request(...)`
     `prepare_single_chapter_generation_request_from_target(...)`
     或 `prepare_owned_single_generation_runtime_launch_input(...)`

4. 这说明当前模块级迁移包已经继续在 single-generation 邻域形成第九条连续
   owner 收口：
   - background write lane 已经压缩了 workflow-start / launch wrapper
   - runtime lane 已经压缩了 driver / terminal wrapper
   - stream success lane 已经压缩了 success-follow-up wrapper
   - prepare lane 已经压缩了 prepared-execution wrapper
   - runtime lane 已经压缩了内部 persistence-plan wrapper 集群
   - background write lane 已经压缩了 launch-persistence wrapper
   - stream lane 已经压缩了 emission-plan wrapper
   - stream lane 已经压缩了 outer lifecycle spawn wrapper
   - prepare lane 现在继续压缩 outer prepare-entry wrapper
   - 后续优先继续在 `chapter_single_generation` prepare / runtime / write
     邻域里找下一个同等级的整段 owner 收口，再决定是否切换模块包

5. 这条 seam 已通过：
   - `cargo fmt --manifest-path "backend-rs/Cargo.toml"`
   - `cargo test chapter_single_generation_prepare_service --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-single-generation-prepare-entry-next-owner" -- --nocapture`
   - `cargo test chapter_single_generation_stream_workflow_service --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-single-generation-prepare-entry-next-owner" -- --nocapture`
   - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "backend-rs/target-codex-single-generation-prepare-entry-next-owner"`

### 2026-06-05 阶段补充：batch write workflow public start 直接收口到 workflow-start owner

在 single-generation 邻域继续推进之外，本轮又回到同一条
`chapter_generation` / `chapter_batch_generation` Phase 5 主线，把 batch create /
resume write-lane 的公共入口再收短一层。

此前 create/resume 两条 lane 虽然都已经有显式的 workflow-start owner，但最外层
public entry 仍然保留了一层：

- `PreparedBatchGenerationCreateWorkflowStart::prepare(...).persist_and_dispatch(...)`
- `PreparedBatchGenerationResumeWorkflowStart::prepare(...).persist_and_dispatch(...)`

这层已经不再承载新的兼容语义，只是在 owner 已经存在之后，把同一条 handoff
链在 free function entry 再重放一遍。于是本轮把它正式收回 owner：

1. **create lane**
   - `PreparedBatchGenerationCreateWorkflowStart` 新增直接入口
     `start(...)`
   - 同一个 owner 现在直接顺序承担：
     `prepare workflow entry -> persist and dispatch`
   - `start_owned_batch_generation_write_workflow(...)`
     不再在外层重开 `prepare(...).persist_and_dispatch(...)`

2. **resume lane**
   - `PreparedBatchGenerationResumeWorkflowStart` 新增直接入口
     `start(...)`
   - 同一个 owner 现在直接顺序承担：
     `prepare workflow launch -> persist and dispatch`
   - `resume_owned_batch_generation_write_workflow(...)`
     不再在外层重开 `prepare(...).persist_and_dispatch(...)`

3. 这条 seam 的价值在于：
   - batch create / batch resume 两条 write-lane 的 public entry
     现在都更接近真正的 workflow-start owner
   - create/resume 在 write-workflow 起点的 owner 图进一步对齐
   - cutover 审计时更容易回答“公共 write-workflow 入口到底是谁真正发起了
     prepare -> persist -> dispatch”

4. 因此当前 `chapter_batch_generation` write lane 的补充 stop-rule 再加一条：
   - **不要**在 create/resume lane 已经拥有显式 workflow-start owner 后，
     仍然让 outer public entry 用
     `prepare(...).persist_and_dispatch(...)` 的形式重复 owner-local handoff

5. 这说明当前 `chapter_generation` Phase 5 主线又继续形成一条真实 owner 收口：
   - 不是只在 restore/prepare/runtime 内部缩 seam
   - 也开始继续压缩公共 write-workflow entry 与 workflow-start owner
     之间最后一层无独立语义的 handoff

6. 这条 seam 已通过：
   - `cargo fmt --manifest-path "backend-rs/Cargo.toml"`
   - `cargo test should_keep_batch_generation_create_workflow_start_owner_contract --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-workflow-start-owner" -- --nocapture`
   - `cargo test should_keep_batch_generation_resume_workflow_start_owner_contract --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-workflow-start-owner" -- --nocapture`
   - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-workflow-start-owner"`

### 2026-06-05 阶段补充：single background workflow public start 直接收口到 workflow-start owner

在 batch write workflow public start 收口之后，本轮继续沿同一个
`chapter_generation` Phase 5 主线回到 `chapter_single_generation` 邻域，把
single background write-lane 的公共入口也再压短一层。

此前 single background write lane 虽然已经有显式的 workflow-entry owner，
但最外层 public entry 仍然保留了一层：

- `SingleGenerationBackgroundWorkflowEntry::prepare(...).persist_and_dispatch(...)`

这层已经不再承载新的兼容语义，只是在 workflow-entry owner 已经存在之后，把
同一条 handoff 链在 free function entry 再重放一遍。于是本轮把它正式收回到
显式 workflow-start owner：

1. **single background workflow-start owner**
   - 新增 `SingleGenerationBackgroundWorkflowStart`
   - 同一个 owner 现在直接顺序承担：
     `prepare workflow entry -> persist and dispatch`
   - `start_owned_single_generation_background_write_workflow(...)`
     不再在外层重开 `prepare(...).persist_and_dispatch(...)`

2. 这条 seam 的价值在于：
   - single background write-lane 的 public entry 现在更接近真正的 owner 边界
   - `existing payload` vs `prepared launch` 的 workflow-entry branch
     继续保留在相邻 owner 中，但 public entry 不再重复重放 handoff
   - cutover 审计时更容易回答“single background write-workflow
     公共入口到底是谁真正串起 prepare -> persist -> dispatch”

3. 因此当前 `chapter_single_generation` background write lane 的补充 stop-rule
   再加一条：
   - **不要**在 single background write lane 已经拥有显式 workflow-start owner
     后，仍然让 outer public entry 用
     `prepare(...).persist_and_dispatch(...)` 的形式重复 owner-local handoff

4. 这说明当前 `chapter_single_generation` 模块包又继续形成一条真实 owner 收口：
   - 不只是继续压 prepare / runtime / stream 邻域里的 owner 图
   - 也继续压缩 public background write entry 与 workflow-start owner
     之间最后一层无独立语义的 handoff

5. 这条 seam 已通过：
   - `cargo fmt --manifest-path "backend-rs/Cargo.toml"`
   - `cargo test chapter_single_generation_write_workflow_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-background-workflow-start-owner" -- --nocapture`
   - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-background-workflow-start-owner"`

### 2026-06-05 阶段补充：single runtime dispatch 直接收口到 lifecycle owner

在 single background workflow public start 收口之后，本轮继续沿同一条
`chapter_single_generation` Phase 5 主线推进到 runtime lane，把单章运行时的
公共 dispatch 入口也再压短一层。

此前 single runtime lane 虽然已经拥有：

- preparing 持久化
- 生成执行 owner
- follow-up analysis manual-review gate
- terminal checkpoint/task persistence

但最外层仍然保留了一层 runtime driver 壳：

- `dispatch_single_chapter_generation_runtime(...)`
  `-> execute_single_generation_runtime(...)`

这层已经不再承载新的兼容语义，只是在 runtime lifecycle 已经明确之后，把同一条
`prepare -> execute -> persist` handoff 链在外层 free function 再重放一遍。
于是本轮把它正式收回到显式 lifecycle owner：

1. **single runtime lifecycle owner**
   - 新增 `SingleGenerationRuntimeLifecyclePlan`
   - 同一个 owner 现在直接顺序承担：
     `persist preparing -> execute generation -> run follow-up analysis -> persist completed/manual-review/failed`
   - `dispatch_single_chapter_generation_runtime(...)`
     现在直接把 launch input 交给 lifecycle owner 执行

2. **外层 runtime driver 壳被压缩**
   - 原先重复重放 lifecycle handoff 的
     `execute_single_generation_runtime(...)`
     已不再保留为独立公共编排层
   - completed 路径里原本按 `enable_analysis` 分开的两段终态持久化分支，
     现在也并入同一个 lifecycle owner 的 completed persistence path

3. 这条 seam 的价值在于：
   - single runtime lane 的 dispatch/public start 现在更接近真正的 owner 边界
   - background / resume 邻域把 runtime launch handoff 交给了同一个 lifecycle owner
   - cutover 审计时更容易回答“single runtime 到底是谁真正串起
     prepare -> execute -> analysis -> terminal persist”

4. 因此当前 `chapter_single_generation` runtime lane 的补充 stop-rule 再加一条：
   - **不要**在 single runtime lane 已经拥有显式 lifecycle owner 后，
     仍然保留单独的
     `dispatch -> execute_single_generation_runtime(...)`
     wrapper chain 作为外层重复 handoff

5. 这说明当前 `chapter_single_generation` 模块包又继续形成一条真实 owner 收口：
   - 不只是继续压 write / prepare / stream 邻域里的 owner 图
   - 也继续压缩 runtime dispatch entry 与 lifecycle owner 之间最后一层
     无独立语义的 handoff

6. 这条 seam 已通过：
   - `cargo test chapter_single_generation_runtime_state_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-runtime-public-start-owner" -- --nocapture`
   - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-runtime-public-start-owner"`

### 2026-06-05 阶段补充：single runtime direct generation-analysis 收口到 lifecycle owner

在 single runtime dispatch 收口到 lifecycle owner 之后，本轮继续沿同一条
`chapter_single_generation` Phase 5 主线推进到 runtime lane 内部，把仍留在
production path 上的 generation / analysis / manual-review free-helper handoff
再收掉一层。

1. `chapter_single_generation` 的 single runtime lane 之前已经有：
   - `SingleGenerationRuntimeLifecyclePlan`
   - preparing persistence
   - completed / failed / manual-review terminal persistence
2. 但 lifecycle 邻层之前仍会在外部重放同一组边界：
   - `execute_owned_single_chapter_generation(...)`
   - `run_single_generation_follow_up_analysis(...)`
   - `maybe_fail_single_generation_for_quality_gate_manual_review(...)`
3. 这代表 single runtime lane 在 Phase 5 上仍保留一条 Python-era 的旧 hop：
   lifecycle owner 已经存在，但真正的 generation execute、analysis routing、
   manual-review terminal persist 仍回到 free helper replay，而不是留在
   lifecycle owner 自己的生产链里。
4. 本轮已把这条 single runtime direct generation-analysis contract
   真正前移回 lifecycle owner 本身：
   - `backend-rs/src/services/chapter_single_generation_runtime_state_service.rs`
     里的 `SingleGenerationRuntimeLifecyclePlan` 现在直接拥有：
     - `execute_generation(...)`
     - `run_follow_up_analysis(...)`
     - `persist_manual_review_generation(...)`
   - 同一个 owner 现在直接串起：
     `execute generation -> run follow-up analysis -> persist manual-review/completed/failed`
5. 因此 single runtime lane 不再需要在 owner 外部重放：
   - generation execute
   - follow-up analysis
   - manual-review terminal persistence

   现在 lifecycle owner 本身终于成为 single runtime
   `generation -> analysis -> terminal persistence` 的更连续 Rust
   materialization 边界。
6. 对 Phase 5 的价值同样直接：
   - cutover 审计时更容易回答“single runtime generation/analysis 到底由哪个
     Rust owner 接手”
   - runtime launch input / generation execute / analysis routing /
     manual-review-completed-failed terminal persistence 现在共享更连续的
     owner 链
   - fallback shrink / rollback / stronger smoke 在 single runtime lane 上
     又少了一条藏在 free helper 里的 active-path replay 支路

### 2026-06-05 阶段补充：single stream public start 直接收口到 workflow-start owner

在 single runtime dispatch 收口到 lifecycle owner 之后，本轮继续沿同一条
`chapter_single_generation` Phase 5 主线推进到 stream lane，把单章流式生成的
公共入口也再压短一层。

此前 single stream lane 虽然已经拥有：

- restored runtime launch prepare
- stream lifecycle owner
- success/error SSE emission owner
- follow-up analysis / completion projection owner

但最外层 public stream entry 仍然保留了一条 handoff 链：

- `prepare(...)`
- `into_runtime_launch_input()`
- `SingleGenerationStreamLifecyclePlan::from_runtime_launch(...).spawn(...)`

这层已经不再承载新的兼容语义，只是在 stream lifecycle 已经明确之后，把同一条
`prepare -> launch -> lifecycle.spawn` handoff 链在外层 free function 再重放一遍。
于是本轮把它正式收回到显式 workflow-start owner：

1. **single stream workflow-start owner**
   - 新增 `SingleGenerationStreamWorkflowStart`
   - 同一个 owner 现在直接顺序承担：
     `prepare restored runtime launch -> hand off to lifecycle.spawn`
   - `create_single_generation_stream_workflow(...)`
     不再在外层重开
     `prepare -> into_runtime_launch_input -> from_runtime_launch(...).spawn(...)`

2. 这条 seam 的价值在于：
   - single stream lane 的 public entry 现在更接近真正的 owner 边界
   - prepare 邻域与 stream lifecycle 邻域之间的 handoff 现在通过一个
     显式 workflow-start owner 串起来
   - cutover 审计时更容易回答“single stream 公共入口到底是谁真正串起
     prepare -> launch -> lifecycle.spawn”

3. 因此当前 `chapter_single_generation` stream lane 的补充 stop-rule 再加一条：
   - **不要**在 single stream lane 已经拥有显式 workflow-start owner 后，
     仍然让 outer public stream entry 用
     `prepare -> into_runtime_launch_input -> lifecycle.spawn`
     的形式重复 owner-local handoff

4. 这说明当前 `chapter_single_generation` 模块包又继续形成一条真实 owner 收口：
   - 不只是继续压 background write / runtime 邻域里的 owner 图
   - 也继续压缩 public stream entry 与 workflow-start owner 之间最后一层
     无独立语义的 handoff

5. 这条 seam 已通过：
   - `cargo test chapter_single_generation_stream_workflow_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-stream-workflow-start-owner" -- --nocapture`
   - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-stream-workflow-start-owner"`

### 2026-06-06 阶段补充：single-generation background/stream workflow wrapper 空壳层收口

在 single-generation route edge 已经收口、background/stream 公共入口也已经逐步变薄之后，
本轮继续沿同一个 `chapter_single_generation` Phase 5 模块包推进，把仍残留在
single background / single stream 生产入口上的几层 wrapper 空壳一起收掉。

1. 当前 single-generation 邻域此前已经有：
   - route payload 直接交给 workflow owner 的入口
   - background workflow-entry owner
   - stream workflow-start / lifecycle owner
2. 但生产链上仍保留四个没有独立兼容意义的 hop：
   - `SingleGenerationBackgroundWorkflowRouteStart`
   - `SingleGenerationBackgroundWorkflowStart`
   - `SingleGenerationStreamWorkflowRouteStart`
   - `SingleGenerationStreamWorkflowStart::start(...)`
3. 这些 wrapper 在当前 owner 结构里已经不再承担新的职责：
   - 不增加 access control
   - 不增加 request normalization
   - 不增加 error translation
   - 不增加 branch selection

   它们只是在重复转手同一条：
   - background:
     `route payload -> request -> prepare -> persist_and_dispatch`
   - stream:
     `route payload -> request -> prepare -> lifecycle.spawn`
4. 本轮已把这条 single-generation workflow wrapper contract 真正压回相邻 owner：
   - `chapter_single_generation_write_workflow_service.rs`
     现在直接保留：
     - `start_owned_single_generation_background_write_workflow_from_route_payload(...)`
     - `start_owned_single_generation_background_write_workflow(...)`
     - `SingleGenerationBackgroundWorkflowEntry::start(...)`
   - `chapter_single_generation_stream_workflow_service.rs`
     现在直接保留：
     - `create_single_generation_stream_workflow_from_route_payload(...)`
     - `create_single_generation_stream_workflow(...)`
     - `SingleGenerationStreamWorkflowStart::prepare(...).spawn(...)`
5. 这条 seam 的意义不在于“少了几个 struct”。它真正回答的是：
   single-generation background / stream 的生产入口在 Rust owner 已经明确之后，
   到底是不是还要回到邻层再转一遍 wrapper。现在这条 duplicate 已被删掉，
   背景写入链和流式链终于都更接近真实的 Rust materialization boundary。
6. 对 Phase 5 的价值同样直接：
   - cutover 审计时更容易回答“single-generation 生产入口到底由哪个 Rust owner 串起来”
   - background/stream 两条 lane 共享了更连续的 owner 边界
   - fallback shrink / rollback / stronger smoke 在 single-generation 邻域上
     又少了几条藏在 wrapper 里的 forwarding 支路
7. 补充 stop-rule：
   - **不要**在 single-generation 的 background / stream owner 已经稳定后，
     继续保留只负责 forwarding 的 `RouteStart` / `WorkflowStart` 空壳层
8. 验证：
   - `cargo test chapter_single_generation_write_workflow_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-workflow-wrapper-collapse" -- --nocapture`
   - `cargo test chapter_single_generation_stream_workflow_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-workflow-wrapper-collapse" -- --nocapture`
   - `cargo test chapter_single_generation --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-workflow-wrapper-collapse" -- --nocapture`
   - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-workflow-wrapper-collapse"`

### 2026-06-06 阶段补充：single-generation prepare/runtime owner 再收口一层

在 single-generation 的 route/workflow wrapper 已经收口之后，本轮没有切换模块，
而是继续沿同一条 `chapter_single_generation` Phase 5 主线，向 prepare/runtime
更内层的 owner 链再压一层。

1. 当前 single-generation 邻域此前已经有：
   - restored-launch owner
   - background / stream workflow owner
   - runtime lifecycle owner
2. 但同一条生产链上仍保留两条没有独立兼容意义的 helper seam：
   - `prepare_validated_single_chapter_generation_request_from_target(...)`
     / `prepare_validated_from_target(...)`
   - `execute_single_generation_runtime_generation(...)`
3. 这些 helper 在当前 owner 结构里已经不再承担新的职责：
   - 不增加 validation 语义
   - 不增加 error translation
   - 不增加 branch selection
   - 不增加新的 transport boundary

   它们只是在重复转手同一条：
   - `validated request/target -> restored-launch materialization`
   - `runtime launch input -> generate/persist chapter`
4. 本轮已把这条 single-generation prepare/runtime contract 真正压回相邻 owner：
   - `PreparedSingleChapterGenerationRestoredRuntimeLaunch::prepare_from_target(...)`
     现在直接承担 validated prepare 全链，不再保留额外的
     `prepare_validated_*` wrapper
   - `SingleGenerationRuntimeLaunchInput` 现在直接暴露
     `execute_generation(...)`
   - `SingleGenerationRuntimeLifecyclePlan` 与 single stream lifecycle
     现在都直接消费这个 owner 方法
5. 这条 seam 的意义不在于“少了两个 helper”。它真正回答的是：
   single-generation 的 request/target 一旦已经 validated，runtime launch input
   一旦已经 materialize，后续到底是不是还要回到邻层再转一遍 helper。
   现在这条 duplicate 已被删掉，prepare/runtime 链终于更接近真实的 Rust
   materialization boundary。
6. 对 Phase 5 的价值同样直接：
   - cutover 审计时更容易回答“single-generation 的 validated prepare 和 execute generation 到底由哪个 Rust owner 接手”
   - background / stream / runtime 三条 lane 共享了更连续的 owner 链
   - fallback shrink / rollback / stronger smoke 在 single-generation 邻域上
     又少了两条藏在 helper 里的 forwarding 支路
7. 补充 stop-rule：
   - **不要**在 single-generation 的 restored-launch owner 和 runtime launch owner
     已经稳定后，继续保留只负责 forwarding 的 `prepare_validated_*` 或
     `execute_*` free helper
8. 验证：
   - `cargo test chapter_single_generation_prepare_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-prepare-runtime-owner-collapse" -- --nocapture`
   - `cargo test chapter_single_generation_runtime_state_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-prepare-runtime-owner-collapse" -- --nocapture`
   - `cargo test chapter_single_generation --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-prepare-runtime-owner-collapse" -- --nocapture`
   - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-prepare-runtime-owner-collapse"`

### 2026-06-06 阶段补充：single-generation stream success owner 再收口一层

在 single-generation 的 prepare/runtime owner 已经继续收口之后，本轮仍然没有切换模块，
而是继续沿同一条 `chapter_single_generation` Phase 5 主线，向 stream success terminal
owner 链再压一层。

1. 当前 single-generation stream 邻域此前已经有：
   - stream workflow start owner
   - stream lifecycle owner
   - stream analysis outcome owner
2. 但同一条成功收尾链上仍保留一个没有独立兼容意义的 projection seam：
   - `SingleGenerationStreamCompletionProjection`
3. 这个 projection owner 在当前 owner 结构里已经不再承担新的职责：
   - 不增加 analysis 语义
   - 不增加 payload translation
   - 不增加 branch selection
   - 不增加新的 transport boundary

   它只是在重复转手同一条：
   - `analysis outcome -> completion message`
   - `analysis outcome -> ordered success event payloads`
   - `analysis outcome -> success SSE emission`
4. 本轮已把这条 single-generation stream success contract 真正压回相邻 owner：
   - `SingleGenerationStreamAnalysisOutcome` 现在直接拥有：
     - `completion_message()`
     - `ordered_success_event_payloads(...)`
     - `emit_success(...)`
   - `SingleGenerationStreamLifecyclePlan` 成功分支现在直接调用
     `analysis.emit_success(&result, &tx, &mut tracker).await`
   - `SingleGenerationStreamCompletionProjection` 已从文件内删除
5. 这条 seam 的意义不在于“少了一个 struct”。它真正回答的是：
   single-generation 的 stream success 一旦已经拿到 generated result 和
   follow-up analysis，后续到底是不是还要回到邻层再转一遍 completion owner。
   现在这条 duplicate 已被删掉，success projection/emission 链终于更接近真实的
   Rust analysis owner boundary。
6. 对 Phase 5 的价值同样直接：
   - cutover 审计时更容易回答“single-generation stream success 的终态 SSE 到底由哪个 Rust owner 串起来”
   - stream lifecycle 与 success terminal lane 共享了更连续的 owner 链
   - fallback shrink / rollback / stronger smoke 在 single-generation 邻域上
     又少了一条藏在 completion owner 里的 forwarding 支路
7. 补充 stop-rule：
   - **不要**在 single-generation 的 stream analysis owner 已经稳定后，
     继续保留只负责 forwarding 的 `SingleGenerationStreamCompletionProjection`
     这一类 completion projection 空壳层
8. 验证：
   - `cargo test chapter_single_generation_stream_workflow_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-stream-analysis-owner-collapse" -- --nocapture`
   - `cargo test chapter_single_generation --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-stream-analysis-owner-collapse" -- --nocapture`
   - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-stream-analysis-owner-collapse"`

### 2026-06-06 阶段补充：single-generation runtime checkpoint 整文件收口

在 single-generation 的 stream success owner 已经继续收口之后，本轮仍然没有切换模块，
而是继续沿同一条 `chapter_single_generation` Phase 5 主线，向 runtime checkpoint
module boundary 再压一层，并完成一次整文件级收口。

1. 当前 single-generation runtime 邻域此前已经有：
   - runtime lifecycle owner
   - task-stage mutation owner
   - runtime snapshot persistence owner
2. 但同一条 runtime snapshot 链上仍保留一个没有独立兼容意义的邻接文件：
   - `chapter_single_generation_runtime_checkpoint_service.rs`
3. 这个 checkpoint 文件在当前 owner 结构里已经不再承担新的职责：
   - 不增加 transport translation
   - 不增加 semantic branching
   - 不增加新的 persistence boundary
   - 不增加独立 error contract

   它只是在重复承载同一条：
   - `SingleGenerationSnapshotStage -> checkpoint payload`
4. 本轮已把这条 single-generation runtime checkpoint contract 真正压回相邻 owner：
   - `chapter_single_generation_runtime_state_service.rs` 现在直接拥有：
     - `SingleGenerationSnapshotStage`
     - `build_single_generation_runtime_checkpoint_for_stage(...)`
   - `chapter_single_generation_prepare_service.rs` 现在直接消费这个 runtime
     owner 进行 pending checkpoint materialization
   - `chapter_single_generation_runtime_checkpoint_service.rs` 已整文件删除
5. 这条 seam 的意义不在于“少了一个小文件”。它真正回答的是：
   single-generation 的 task-stage mutation、runtime snapshot persistence、
   checkpoint payload projection 一旦已经全部落在 runtime 邻域里，后续到底是不是
   还要保留一个单独 checkpoint 文件作为中转。现在这条 duplicate 已被删掉，
   runtime snapshot 链终于更接近真实的 Rust runtime owner boundary。
6. 对 Phase 5 的价值同样直接：
   - cutover 审计时更容易回答“single-generation runtime checkpoint 到底由哪个 Rust owner 持有”
   - prepare/runtime 两条 lane 共享了更连续的 owner 边界
   - fallback shrink / rollback / stronger smoke 在 single-generation 邻域上
     又少了一个藏在独立模块里的 forwarding 节点
7. 补充 stop-rule：
   - **不要**在 single-generation runtime owner 已经稳定后，继续保留只负责
     `snapshot stage -> checkpoint payload` 投影的独立 checkpoint service 文件
8. 验证：
   - `cargo test chapter_single_generation_runtime_state_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-runtime-checkpoint-file-collapse" -- --nocapture`
   - `cargo test chapter_single_generation_prepare_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-runtime-checkpoint-file-collapse" -- --nocapture`
   - `cargo test chapter_single_generation --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-runtime-checkpoint-file-collapse" -- --nocapture`
   - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-runtime-checkpoint-file-collapse"`

### 2026-06-06 阶段补充：single-generation existing-background query owner 再收口一层

在 single-generation 的 runtime checkpoint 已经完成整文件收口之后，本轮仍然没有切换模块，
而是继续沿同一条 `chapter_single_generation` Phase 5 主线，向 existing-background
query lane 再压一层。

1. 当前 single-generation background write 邻域此前已经有：
   - target loading owner
   - existing-task short-circuit branch selection
   - prepared background launch / persist-and-dispatch owner
2. 但同一条 existing-task short-circuit 链上仍保留一个没有独立兼容意义的 batch 邻层入口：
   - `chapter_batch_generation_task_view_query_service::load_existing_single_generation_background_task_payload(...)`
3. 这个 batch query entry 在当前 owner 结构里已经不再承担新的职责：
   - 不增加 transport translation
   - 不增加 request validation
   - 不增加 semantic branching
   - 不增加独立 error contract

   它只是在重复承载同一条：
   - `load active project tasks -> existing background payload projection`
4. 本轮已把这条 single-generation existing-background query contract 真正压回相邻 owner：
   - `chapter_single_generation_write_workflow_service.rs` 现在直接拥有：
     - `load_active_single_generation_background_tasks(...)`
     - `load_owned_single_generation_existing_background_task_payload(...)`
   - `SingleGenerationBackgroundWorkflowEntry::prepare(...)` 现在直接消费这条
     single-generation write owner query 链
   - `chapter_batch_generation_task_view_query_service.rs` 已移除
     single-background existing-task 公开入口，只保留 batch active-task-list /
     active-project query lane
5. 这条 seam 的意义不在于“少了一个调用”。它真正回答的是：
   single-generation 的 background write lane 一旦已经自己拥有 target load、
   existing-task short-circuit 和 final compat payload consumer，后续到底是不是还要
   回到 batch task-view query 邻层再转一遍入口。现在这条 duplicate 已被删掉，
   existing-background query lane 更接近真实的 Rust single-generation owner boundary。
6. 对 Phase 5 的价值同样直接：
   - cutover 审计时更容易回答“single-generation existing background payload 到底由哪个 Rust owner 查询”
   - single-generation background write lane 共享了更连续的 owner 边界
   - fallback shrink / rollback / stronger smoke 在 single-generation 邻域上
     又少了一条藏在 batch 邻层里的 forwarding 节点
7. 补充 stop-rule：
   - **不要**在 single-generation background write owner 已经稳定后，继续保留只为
     single-background branch 服务的 batch task-view query 入口
8. 验证：
   - `cargo test chapter_single_generation_write_workflow_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-existing-background-query-owner-collapse" -- --nocapture`
   - `cargo test chapter_batch_generation_task_view_query_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-existing-background-query-owner-collapse" -- --nocapture`
   - `cargo test chapter_single_generation --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-existing-background-query-owner-collapse" -- --nocapture`
   - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-existing-background-query-owner-collapse"`

### 2026-06-06 阶段补充：single-generation existing-background payload owner 再收口一层

在 existing-background query 入口已经从 batch task-view query 邻层拉回之后，本轮仍然没有切换模块，
而是继续沿同一条 `chapter_single_generation` Phase 5 主线，把 single-generation 专属的
existing-background payload projection 也压回 write owner。

1. 当前 single-generation background write 邻域此前已经有：
   - target loading owner
   - active-task query selection owner
   - existing-task short-circuit branch selection owner
2. 但同一条 existing-task short-circuit 链上仍保留一个没有独立 batch 兼容意义的 read-context 投影 seam：
   - `BatchGenerationReadContext::into_single_generation_existing_background_task_payload()`
   - `load_existing_single_generation_background_task_payload_for_tasks(...)`
3. 这组 helper 在当前 owner 结构里已经不再承担新的职责：
   - 不增加 transport translation
   - 不增加 request validation
   - 不增加 semantic branching
   - 不增加独立 error contract

   它只是在重复承载同一条：
   - `active read contexts -> find chapter task -> existing background payload`
4. 本轮已把这条 single-generation existing-background payload contract 真正压回相邻 owner：
   - `chapter_single_generation_write_workflow_service.rs` 现在直接拥有：
     - `into_single_generation_existing_background_task_payload(...)`
     - `load_owned_single_generation_existing_background_task_payload(...)`
   - `SingleGenerationBackgroundWorkflowEntry::prepare(...)` 现在直接消费这条
     single-generation write owner payload 链
   - `chapter_batch_generation_read_context_service.rs` 已移除
     single-generation existing-background payload projection，只保留 batch shared
     read-context payload owner
5. 这条 seam 的意义不在于“又少了一个 helper”。它真正回答的是：
   single-generation 的 background write lane 一旦已经自己拥有 active-task query、
   chapter match filtering 和 final compat payload consumer，后续到底是不是还要回到
   batch read-context 邻层再做一次单章专属 payload 投影。现在这条 duplicate 已被删掉，
   existing-background payload lane 更接近真实的 Rust single-generation owner boundary。
6. 对 Phase 5 的价值同样直接：
   - cutover 审计时更容易回答“single-generation existing background payload 到底由哪个 Rust owner 投影”
   - single-generation background write lane 共享了更连续的 owner 边界
   - fallback shrink / rollback / stronger smoke 在 single-generation 邻域上
     又少了一条藏在 batch read-context 邻层里的 forwarding 节点
7. 补充 stop-rule：
   - **不要**在 single-generation background write owner 已经稳定后，继续保留只为
     single-background branch 服务的 batch read-context payload projection helper
8. 验证：
   - `cargo test chapter_single_generation_write_workflow_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-existing-background-payload-owner-collapse" -- --nocapture`
   - `cargo test chapter_batch_generation_read_context_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-existing-background-payload-owner-collapse" -- --nocapture`
   - `cargo test chapter_single_generation --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-existing-background-payload-owner-collapse" -- --nocapture`
   - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-existing-background-payload-owner-collapse"`

### 2026-06-06 阶段补充：single-generation existing-background payload variant 再收口一层

在 single-generation existing-background payload projection 已经从 batch read-context 邻层拉回之后，
本轮仍然没有切换模块，而是继续沿同一条 `chapter_single_generation` Phase 5 主线，把
single-generation 专属的 existing-background payload variant 也从 batch payload base 中拆出去。

1. 当前 single-generation background write 邻域此前已经有：
   - active-task query selection owner
   - chapter match filtering owner
   - existing-task short-circuit payload consumer owner
2. 但同一条 existing-task payload 链上仍保留一个没有独立 batch 共享意义的 payload-base variant：
   - `BatchGenerationTaskViewPayloadVariant::SingleGenerationExistingBackgroundTask`
3. 这个 variant 在当前 owner 结构里已经不再承担新的职责：
   - 不增加 transport translation
   - 不增加 request validation
   - 不增加 semantic branching
   - 不增加独立 error contract

   它只是在重复承载同一条：
   - `shared task-view payload base -> single-generation existing payload fields`
4. 本轮已把这条 single-generation existing-background payload variant contract 真正压回相邻 owner：
   - `chapter_single_generation_write_workflow_service.rs` 现在直接拥有
     single-generation existing-background payload 的字段装配：
     - `task_id`
     - `chapter_id`
     - `message`
     - `estimated_time_minutes`
   - `chapter_batch_generation_task_payload_base_service.rs` 已移除
     `BatchGenerationTaskViewPayloadVariant::SingleGenerationExistingBackgroundTask`
     这个单章专属 variant，只保留 batch shared task-view payload variants
5. 这条 seam 的意义不在于“少了一个枚举分支”。它真正回答的是：
   single-generation 的 existing-background payload 一旦已经由 Rust write owner
   自己掌握 active-task query、chapter match 和 final compat payload consumer，
   后续到底是不是还要回到 batch payload base 邻层保留一个只给单章分支使用的 variant。
   现在这条 duplicate 已被删掉，single-generation existing-background payload
   更接近真实的 Rust owner boundary。
6. 对 Phase 5 的价值同样直接：
   - cutover 审计时更容易回答“single-generation existing background payload 到底由哪个 Rust owner 最终装配”
   - batch payload base 的 shared boundary 更干净，只保留真正 batch shared 的 task-view variants
   - fallback shrink / rollback / stronger smoke 在 single-generation 邻域上
     又少了一条藏在 batch payload 基座里的 forwarding 节点
7. 补充 stop-rule：
   - **不要**在 single-generation background write owner 已经稳定后，继续保留只为
     single-background branch 服务的 batch task-view payload variant
8. 验证：
   - `cargo test chapter_single_generation_write_workflow_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-existing-background-payload-variant-collapse" -- --nocapture`
   - `cargo test chapter_batch_generation_task_payload_base_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-existing-background-payload-variant-collapse" -- --nocapture`
   - `cargo test chapter_single_generation --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-existing-background-payload-variant-collapse" -- --nocapture`
   - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-existing-background-payload-variant-collapse"`

### 2026-06-06 阶段补充：single-generation existing-background read-context owner 再收口一层

在 single-generation existing-background payload variant 已经从 batch payload base 拆出去之后，
本轮仍然没有切换模块，而是继续沿同一条 `chapter_single_generation` Phase 5 主线，把
single-generation 专属的 existing-background read-context owner 也从 batch read-context 邻层拉出来。

1. 当前 single-generation background write 邻域此前已经有：
   - active-task query selection owner
   - chapter match filtering owner
   - existing-task short-circuit payload projection owner
2. 但同一条 existing-task read-state 链上仍保留一个没有独立 single-generation 兼容意义的 batch read-context owner 链：
   - `BatchGenerationReadContext`
   - `load_active_batch_generation_read_contexts_for_tasks(...)`
   - `batch_generation_task_contains_chapter(...)`
3. 这组 helper 在当前 owner 结构里已经不再承担新的职责：
   - 不增加 transport translation
   - 不增加 request validation
   - 不增加 semantic branching
   - 不增加独立 error contract

   它只是在重复承载同一条：
   - `recover active tasks -> load snapshots -> build read context -> match chapter`
4. 本轮已把这条 single-generation existing-background read-context contract 真正压回相邻 owner：
   - `chapter_single_generation_write_workflow_service.rs` 现在直接拥有：
     - `SingleGenerationExistingBackgroundTaskContext`
     - `load_active_single_generation_existing_background_task_contexts(...)`
     - `single_generation_existing_background_task_contains_chapter(...)`
   - `chapter_batch_generation_read_context_service.rs` 已不再参与这条
     single-generation existing-background read-state/context owner 链，只保留 batch shared
     read-context lanes 与更底层可复用 recovery primitive
5. 这条 seam 的意义不在于“又少了一个 struct”。它真正回答的是：
   single-generation 的 existing-background write lane 一旦已经自己拥有 active-task query、
   chapter match 和 final compat payload projection，后续到底是不是还要回到
   batch read-context 邻层再做一次单章专属 read-state/context 组织。现在这条 duplicate 已被删掉，
   single-generation existing-background read-state 更接近真实的 Rust owner boundary。
6. 对 Phase 5 的价值同样直接：
   - cutover 审计时更容易回答“single-generation existing background read-state 到底由哪个 Rust owner 组织”
   - single-generation background write lane 的 local owner chain 更连续
   - fallback shrink / rollback / stronger smoke 在 single-generation 邻域上
     又少了一条藏在 batch read-context 邻层里的 forwarding 节点
7. 补充 stop-rule：
   - **不要**在 single-generation background write owner 已经稳定后，继续保留只为
     single-background branch 服务的 batch read-context owner 链
8. 验证：
   - `cargo test chapter_single_generation_write_workflow_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-existing-background-read-context-owner-collapse" -- --nocapture`
   - `cargo test chapter_batch_generation_read_context_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-existing-background-read-context-owner-collapse" -- --nocapture`
   - `cargo test chapter_single_generation --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-existing-background-read-context-owner-collapse" -- --nocapture`
   - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-existing-background-read-context-owner-collapse"`

### 2026-06-05 阶段补充：resume restored-state launch 直接收口到 restored owner

在 batch resume 邻域持续向 restored runtime-state owner 收口之后，本轮继续沿同一条
`chapter_batch_generation` Phase 5 主线推进到 resume launch materialization 邻域，
又收掉了一条“restored owner 已经 materialize，但 command 邻层仍本地重放
request-runtime / runtime-seed handoff”的旧 hop。

1. `chapter_batch_generation` 的 batch/single resume 邻域已经有：
   - `RestoredResumeRuntimeStateProjection`
   - restored `request_runtime_state`
   - restored `runtime_state_seed`
2. 但 `chapter_batch_generation_resume_task_command_service.rs` 之前仍会在
   `ResumeExecutionDispatchPlan::from_validated_execution(...)` 里本地重放同一组边界：
   - 先 `into_launch_parts()`
   - 再分别重建 batch/single runtime launch
3. 这代表 batch resume / single resume dispatch 邻链在 Phase 5 上仍保留一条
   Python-era 的旧 hop：owner 已 materialize，但 command 邻层仍保留一条
   平行 launch-assembly 支路。
4. 本轮已把这条 restored-state launch contract 真正前移回 restored owner：
   - `backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs`
     里的 `RestoredResumeRuntimeStateProjection` 现在直接拥有：
     - `prepare_batch_runtime_launch(...)`
     - `prepare_single_chapter_runtime_launch(...)`
   - `backend-rs/src/services/chapter_batch_generation_resume_task_command_service.rs`
     现在直接消费该 owner，不再本地 reopen `into_launch_parts()`。
5. 这条 seam 的意义不在于“又少一个 helper 调用”。它真正回答的是：
   batch/single resume 的 restored runtime-state 一旦已经被 Rust owner
   materialize，后续 launch 到底是不是还要回到 command 邻层再拼一遍。
   现在这条 duplicate 已被删掉，restored owner 本身终于成为
   batch/single resume launch 的显式 Rust materialization 边界。
6. 对 Phase 5 的价值同样直接：
   - cutover 审计时更容易回答“resume launch 到底由哪个 Rust owner 接手”
   - batch resume / single resume / reset persistence / dispatch plan
     现在共享更连续的 owner 链
   - fallback shrink / rollback / stronger smoke 在 resume lane 上又少了一条
     藏在 command 邻层里的 launch rebuild 支路
7. 验证：
   - `cargo test chapter_batch_generation_resume_task_command_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/resume-restored-state-owner" -- --nocapture`
   - `cargo test chapter_batch_generation_runtime_state_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/resume-restored-state-owner" -- --nocapture`
   - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/resume-restored-state-owner"`

### 2026-06-06 阶段补充：single-generation background payload base 再收口一层

在 single-generation existing-background read-context owner 已经拉回相邻
`chapter_single_generation` write lane 之后，本轮没有切换模块，而是继续沿同一条
`chapter_single_generation` Phase 5 主线，把 background create payload 和
existing-background payload 之间仍残留的 batch payload/status base hop 再收掉一层。

1. `chapter_single_generation` 的 background 邻域已经有：
   - target loading
   - existing-task query / read-state / payload short-circuit
   - background create response payload projection
2. 但 create / existing 两条 payload 邻链之前仍会在最后一层重放同一组 batch 语义：
   - `build_batch_generation_task_view_payload_from_task_state(...)`
   - `estimated_task_minutes(...)`
   - `active_batch_generation_statuses()`
3. 这代表 single-generation background payload lane 在 Phase 5 上仍保留一条
   Python-era 的旧 hop：single-generation owner 已经 materialize runtime/task state，
   但 payload base / active-status / single-task estimate 仍回到 batch 邻层再拼一次。
4. 本轮已把这条 single-generation background payload base contract 真正压回相邻 owner：
   - `backend-rs/src/services/chapter_single_generation_prepare_service.rs`
     现在直接拥有：
     - `estimated_single_generation_task_minutes(...)`
     - `single_generation_pending_stage_code()`
     - `single_generation_active_task_statuses()`
     - `build_single_generation_runtime_payload_base(...)`
     - `build_single_generation_task_view_payload_from_task_state(...)`
   - `backend-rs/src/services/chapter_single_generation_write_workflow_service.rs`
     现在直接消费这条 single-generation 本地 payload base owner，不再重开
     batch task-view/status semantics helper。
5. 这条 seam 的意义不在于“又少一个 helper 调用”。它真正回答的是：
   single-generation background create payload 和 existing-background payload
   在 Rust owner 已稳定之后，到底是不是还要回到 batch 邻层保留一条只服务于
   单章分支的 payload base / active-status 语义链。现在这条 duplicate 已被删掉，
   create / existing 两条 payload 终于共享同一条 single-generation-local base owner。
6. 对 Phase 5 的价值同样直接：
   - cutover 审计时更容易回答“single background payload base 到底由哪个 Rust owner 接手”
   - background create / existing short-circuit / runtime checkpoint payload
     现在共享更连续的 single-generation owner 链
   - 后续继续整块迁移时，不必再回到 batch payload/status 邻层保留只为单章背景任务
     服务的兼容壳
7. 验证：
   - `cargo test chapter_single_generation_prepare_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-background-payload-base-owner-collapse" -- --nocapture`
   - `cargo test chapter_single_generation_write_workflow_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-background-payload-base-owner-collapse" -- --nocapture`
   - `cargo test chapter_single_generation --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-background-payload-base-owner-collapse" -- --nocapture`
   - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-background-payload-base-owner-collapse"`

### 2026-06-06 阶段补充：single-generation quality-status owner 再收口一层

在 single-generation background payload base owner 已经拉回相邻
`chapter_single_generation` owner 之后，本轮继续沿同一条
`chapter_single_generation` Phase 5 主线，把仍残留在单章背景任务和单章 runtime
手工复核链上的 batch quality-status semantic shell 再收掉一层。

1. `chapter_single_generation` 的单章 quality 邻域已经有：
   - chapter-scoped quality runtime context reconstruction
   - existing-background payload quality field insertion
   - runtime follow-up analysis + manual-review persistence
2. 但 existing-background payload / runtime manual-review 两条链之前仍会在最后一层
   重放同一组 batch 语义：
   - `BatchGenerationQualityStatusContext`
   - `manual_review_label_from_quality_context(...)`
3. 这代表 single-generation quality lane 在 Phase 5 上仍保留一条
   Python-era 的旧 hop：chapter-scoped quality source 已经由单章 owner materialize，
   但 quality payload projection 与 manual-review label 解析仍回到 batch 邻层再拼一次。
4. 本轮已把这条 single-generation quality-status contract 真正压回相邻 owner：
   - 新增
     `backend-rs/src/services/chapter_single_generation_quality_status_service.rs`
   - 现在直接拥有：
     - `SingleGenerationQualityStatusContext`
     - `SingleGenerationQualityStatusContext::from_snapshot_and_runtime_state(...)`
     - `SingleGenerationQualityStatusContext::insert_into_payload(...)`
     - `manual_review_label_from_single_generation_quality_context(...)`
   - `chapter_single_generation_write_workflow_service.rs`
     现在直接消费这条 single-generation 本地 quality-status owner
   - `chapter_single_generation_runtime_state_service.rs`
     现在直接消费这条 single-generation 本地 manual-review label owner
5. 这条 seam 的意义不在于“又少一个 helper 调用”。它真正回答的是：
   单章背景任务 quality payload 和单章 runtime manual-review label 在 Rust owner
   已稳定之后，到底是不是还要回到 batch 邻层保留一条只服务于 chapter scope 的
   quality-status semantic shell。现在这条 duplicate 已被删掉，single-generation
   quality-status 终于成为显式的本地 Rust owner。
6. 对 Phase 5 的价值同样直接：
   - cutover 审计时更容易回答“single-generation quality-status 到底由哪个 Rust owner 接手”
   - existing-background payload / runtime manual-review / chapter quality history
     现在共享更连续的 single-generation owner 链
   - 后续继续整块迁移时，不必再回到 batch quality-status 邻层保留只为单章分支
     服务的兼容壳
7. 验证：
   - `cargo test chapter_single_generation_quality_status_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-quality-status-owner-collapse" -- --nocapture`
   - `cargo test chapter_single_generation_write_workflow_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-quality-status-owner-collapse" -- --nocapture`
   - `cargo test chapter_single_generation_runtime_state_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-quality-status-owner-collapse" -- --nocapture`
   - `cargo test chapter_single_generation --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-quality-status-owner-collapse" -- --nocapture`
   - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-quality-status-owner-collapse"`

### 2026-06-05 阶段补充：single restored-launch 直接收口到生产 materialization owner

在 single-generation 邻域已经连续把 prepare entry、stream public start 和
background launch-parts owner 收回来之后，本轮没有切换模块，而是继续沿同一条
`chapter_single_generation` Phase 5 主线，把仍残留在生产入口上的
`prepare(...).into_*()` handoff 链再收掉一层。

1. `chapter_single_generation` 的 restored-launch 邻域已经有：
   - `PreparedSingleChapterGenerationRestoredRuntimeLaunch`
   - startup snapshot owner
   - runtime launch input
   - background launch-parts projection
2. 但 stream/background 两条 production 邻链之前仍会在外层重放同一组边界：
   - `prepare(...).into_runtime_launch_input()`
   - `prepare_from_target(...).into_background_launch_parts(task_id)`
3. 这代表 single-generation restored-launch lane 在 Phase 5 上仍保留一条
   Python-era 的旧 hop：owner 已 materialize，但生产 workflow 邻层仍保留
   一条平行的 final materialization 支路。
4. 本轮已把这条 restored-launch production materialization contract 真正前移回
   owner 本身：
   - `backend-rs/src/services/chapter_single_generation_prepare_service.rs`
     里的 `PreparedSingleChapterGenerationRestoredRuntimeLaunch` 现在直接拥有：
     - `prepare_runtime_launch_input(...)`
     - `prepare_background_launch_parts_from_target(...)`
   - `chapter_single_generation_stream_workflow_service.rs`
     现在直接消费 owner 提供的 runtime-launch materialization
   - `chapter_single_generation_write_workflow_service.rs`
     现在直接消费 owner 提供的 background-launch materialization
5. 这条 seam 的意义不在于“又少一行 `.into_*()`”。它真正回答的是：
   single-generation 的 restored-launch 一旦已经被 Rust owner materialize，
   后续到底是不是还要回到邻层 workflow 再拆一遍。现在这条 duplicate 已被删掉，
   restored-launch owner 本身终于成为 stream/background 两条生产邻链的显式
   Rust materialization 边界。
6. 对 Phase 5 的价值同样直接：
   - cutover 审计时更容易回答“single-generation 生产 launch 产物到底由哪个 Rust owner 接手”
   - prepare / stream / background 三条相邻 owner 链现在共享更连续的 owner 边界
   - fallback shrink / rollback / stronger smoke 在 single-generation 邻域上又少了
     一条藏在 workflow 邻层里的 local rebuild 支路
7. 验证：
   - `cargo test chapter_single_generation_prepare_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-restored-launch-owner" -- --nocapture`
   - `cargo test chapter_single_generation_write_workflow_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-restored-launch-owner" -- --nocapture`
   - `cargo test chapter_single_generation_stream_workflow_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-restored-launch-owner" -- --nocapture`
   - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/single-restored-launch-owner"`

### 2026-06-05 阶段补充：batch create workflow-launch 直接收口到 persistence-plan owner

在 single-generation 邻域连续收口之外，本轮继续回到同一条
`chapter_batch_generation` Phase 5 主线，把 batch create write lane 上仍残留的
`prepare(...).into_persistence_plan(...)` 生产 handoff 链再收掉一层。

1. `chapter_batch_generation` 的 batch create 邻域已经有：
   - `PreparedBatchGenerationCreateWorkflowLaunch`
   - startup snapshot owner
   - runtime launch input
   - create response payload / task-seed projection
2. 但 create workflow-entry 邻层之前仍会在外层重放同一组边界：
   - 先 `PreparedBatchGenerationCreateWorkflowLaunch::prepare(...)`
   - 再本地 `.into_persistence_plan(...)`
3. 这代表 batch create write lane 在 Phase 5 上仍保留一条 Python-era 的旧 hop：
   workflow-launch owner 已 materialize，但 workflow-entry 邻层仍保留一条
   平行的 persistence-plan rebuild 支路。
4. 本轮已把这条 create persistence-plan materialization contract 真正前移回
   workflow-launch owner 本身：
   - `backend-rs/src/services/chapter_batch_generation_write_workflow_service.rs`
     里的 `PreparedBatchGenerationCreateWorkflowLaunch` 现在直接拥有：
     - `prepare_persistence_plan(...)`
   - `PreparedBatchGenerationCreateWorkflowEntry::prepare(...)`
     现在直接消费该 owner，不再本地 reopen
     `prepare(...).into_persistence_plan(...)`
5. 这条 seam 的意义不在于“少了一次函数串接”。它真正回答的是：
   batch create 的 workflow-launch 一旦已经被 Rust owner materialize，
   后续 persistence-plan 到底是不是还要回到 workflow-entry 邻层再拼一遍。
   现在这条 duplicate 已被删掉，workflow-launch owner 本身终于成为
   batch create persistence-plan 的显式 Rust materialization 边界。
6. 对 Phase 5 的价值同样直接：
   - cutover 审计时更容易回答“batch create persistence-plan 到底由哪个 Rust owner 接手”
   - create launch / startup snapshot / persistence-plan 现在共享更连续的 owner 链
   - fallback shrink / rollback / stronger smoke 在 batch create 邻域上又少了一条
     藏在 workflow-entry 邻层里的 local rebuild 支路
7. 验证：
   - `cargo test chapter_batch_generation_write_workflow_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-create-persistence-owner" -- --nocapture`
   - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-create-persistence-owner"`

### 2026-06-05 阶段补充：batch runtime public start 直接收口到 lifecycle owner

在 batch create write lane 收口之外，本轮继续沿同一条
`chapter_batch_generation` Phase 5 主线，把 batch runtime outer entry 上仍残留的
`dispatch -> execute -> driver -> lifecycle` 生产 handoff 链再收掉一层。

1. `chapter_batch_generation` 的 batch runtime 邻域已经有：
   - `BatchGenerationRuntimeLifecyclePlan`
   - preparing persistence owner
   - chapter step / attempt / post-analysis progression owner
2. 但 outer runtime entry 之前仍会在外层重放同一组边界：
   - `dispatch_batch_generation_runtime(...)`
   - `execute_batch_generation_runtime(...)`
   - `BatchGenerationRuntimeDriver::new(...).execute(...)`
3. 这代表 batch runtime lane 在 Phase 5 上仍保留一条 Python-era 的旧 hop：
   lifecycle owner 已经存在，但 runtime public start 邻层仍保留一条
   平行的 wrapper/forwarding 支路。
4. 本轮已把这条 batch runtime public-start contract 真正前移回
   lifecycle owner 本身：
   - `backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs`
     里的 `BatchGenerationRuntimeLifecyclePlan` 现在直接拥有：
     - `start(...)`
   - `dispatch_batch_generation_runtime(...)` 现在直接把 execution input
     交给该 owner，不再本地 reopen
     `execute_batch_generation_runtime(...) -> BatchGenerationRuntimeDriver`
   - 已确认 `execute_batch_generation_runtime(...)` 不再有生产引用，并在验证后
     直接删除，避免留下新的 dead wrapper
5. 这条 seam 的意义不在于“少了一层函数”。它真正回答的是：
   batch runtime 一旦已经进入 lifecycle owner，外层 runtime public start
   到底是不是还要回到邻层再转一遍 driver/wrapper。现在这条 duplicate 已被删掉，
   lifecycle owner 本身终于成为 batch runtime public-start 的显式 Rust
   materialization 边界。
6. 对 Phase 5 的价值同样直接：
   - cutover 审计时更容易回答“batch runtime public-start 到底由哪个 Rust owner 接手”
   - batch runtime dispatch / lifecycle / step progression 现在共享更连续的 owner 链
   - fallback shrink / rollback / stronger smoke 在 batch runtime 邻域上又少了一条
     藏在 outer runtime entry 里的 local wrapper 支路
7. 验证：
   - `cargo test chapter_batch_generation_runtime_state_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-runtime-public-start-owner" -- --nocapture`
   - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-runtime-public-start-owner"`

### 2026-06-05 阶段补充：batch runtime post-analysis 直接收口到 success owner 邻链

在 batch runtime public-start 收口之后，本轮继续沿同一条
`chapter_batch_generation` Phase 5 主线，把 batch runtime success lane 上仍残留的
`analysis owner -> local wrapper -> terminal owner` 生产 handoff 链再收掉一层。

1. `chapter_batch_generation` 的 batch runtime success 邻域已经有：
   - `BatchGenerationFollowUpAnalysisPlan`
   - `BatchGenerationPostAnalysisTerminalPlan`
   - post-write guard owner
2. 但 success 邻层之前仍会在外层重放同一组边界：
   - 本地 `run_follow_up_analysis(...)`
   - 本地 `resolve_analysis_outcome(...)`
3. 这代表 batch runtime success lane 在 Phase 5 上仍保留一条 Python-era 的旧 hop：
   analysis owner 与 terminal owner 都已经存在，但 success 邻层仍保留一条
   平行的 forwarding 支路。
4. 本轮已把这条 batch runtime post-analysis direct-owner contract 真正前移回
   success owner 本身：
   - `backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs`
     里的 `PreparedBatchGenerationStepExecution::execute_success_chain(...)`
     现在直接：
     - 调用 `BatchGenerationFollowUpAnalysisPlan`
     - 把结果直接交给 `BatchGenerationPostAnalysisTerminalPlan`
   - 不再本地 reopen
     `run_follow_up_analysis(...) -> resolve_analysis_outcome(...)`
5. 这条 seam 的意义不在于“少了两个 helper”。它真正回答的是：
   batch runtime success lane 在 follow-up analysis owner 已经存在后，
   analysis result 到底是不是还要回到 success 邻层再转一遍 local wrapper。
   现在这条 duplicate 已被删掉，success owner 邻链终于成为 batch runtime
   post-analysis handoff 的更连续 Rust materialization 边界。
6. 对 Phase 5 的价值同样直接：
   - cutover 审计时更容易回答“batch runtime post-analysis 到底由哪个 Rust owner 接手”
   - success / analysis / terminal routing 现在共享更连续的 owner 链
   - fallback shrink / rollback / stronger smoke 在 batch runtime success 邻域上
     又少了一条藏在 local wrapper 里的 forwarding 支路
7. 验证：
   - `cargo test chapter_batch_generation_runtime_state_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-runtime-post-analysis-owner" -- --nocapture`
   - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-runtime-post-analysis-owner"`

### 2026-06-05 阶段补充：batch runtime analysis-attempt 直接收口到 attempt owner

在 batch runtime post-analysis 邻链收口之后，本轮继续沿同一条
`chapter_batch_generation` Phase 5 主线，把 batch runtime follow-up analysis
attempt 上仍残留的 `attempt owner -> local wrapper -> resolution owner`
生产 handoff 链再收掉一层。

1. `chapter_batch_generation` 的 batch runtime analysis-attempt 邻域已经有：
   - `BatchGenerationAnalysisAttemptPlan`
   - `BatchGenerationAnalysisAttemptResolutionPlan`
   - analysis-started snapshot persistence
2. 但 analysis-attempt 邻层之前仍会在外层重放同一组边界：
   - 一条外层 `persist_started(...)` 分支
   - 一条本地 `execute_prepared_or_fallback(...)` wrapper
3. 这代表 batch runtime analysis-attempt lane 在 Phase 5 上仍保留一条
   Python-era 的旧 hop：attempt owner 与 resolution owner 都已经存在，
   但 attempt 邻层仍保留一条平行的 prepared/fallback forwarding 支路。
4. 本轮已把这条 batch runtime analysis-attempt direct-preparation contract
   真正前移回 attempt owner 本身：
   - `backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs`
     里的 `BatchGenerationAnalysisAttemptPlan::execute(...)` 现在直接：
     - 选择 prepared analysis 或 fallback
     - 持久化 started snapshot
     - 执行 prepared/fallback analysis
     - 把结果交给 `BatchGenerationAnalysisAttemptResolutionPlan`
   - 不再本地 reopen
     `execute_prepared_or_fallback(...)`
5. 这条 seam 的意义不在于“少了一个 helper”。它真正回答的是：
   batch runtime analysis-attempt lane 在 attempt owner 已经存在后，
   prepared/fallback execution 到底是不是还要回到邻层再转一遍 local wrapper。
   现在这条 duplicate 已被删掉，attempt owner 自身终于成为 batch runtime
   analysis-attempt 的更连续 Rust materialization 边界。
6. 对 Phase 5 的价值同样直接：
   - cutover 审计时更容易回答“batch runtime analysis-attempt 到底由哪个 Rust owner 接手”
   - attempt preparation / started persistence / execution / resolution
     现在共享更连续的 owner 链
   - fallback shrink / rollback / stronger smoke 在 batch runtime analysis 邻域上
     又少了一条藏在 local wrapper 里的 prepared/fallback 支路
7. 验证：
   - `cargo test chapter_batch_generation_runtime_state_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-runtime-analysis-attempt-next-owner" -- --nocapture`
   - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-runtime-analysis-attempt-next-owner"`

### 2026-06-05 阶段补充：batch runtime terminal quality-gate 直接收口到 terminal owner

在 batch runtime analysis-attempt 收口之后，本轮继续沿同一条
`chapter_batch_generation` Phase 5 主线，把 batch runtime post-analysis
terminal 上仍残留的 `terminal owner -> quality-gate-resolution owner ->
routing plan` 生产 handoff 链再收掉一层。

1. `chapter_batch_generation` 的 batch runtime terminal 邻域已经有：
   - `BatchGenerationPostAnalysisTerminalPlan`
   - `BatchGenerationQualityGateRoutingPlan`
   - post-analysis success / failure routing
2. 但 terminal success 邻层之前仍会在外层重放同一组边界：
   - materialize `BatchGenerationQualityGateResolutionPlan`
   - 再本地把 quality-runtime state 和 retry budget 交给该邻层
3. 这代表 batch runtime terminal lane 在 Phase 5 上仍保留一条
   Python-era 的旧 hop：terminal owner 和 routing owner 都已经存在，但
   success 邻层仍保留一条平行的 resolution 支路。
4. 本轮已把这条 batch runtime terminal quality-gate contract 真正前移回
   terminal owner 本身：
   - `backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs`
     里的 `BatchGenerationPostAnalysisTerminalPlan` 现在直接：
     - 加载 retry budget context
     - resolve quality-gate terminal semantics
     - hand off to `BatchGenerationQualityGateRoutingPlan`
   - 不再单独 materialize
     `BatchGenerationQualityGateResolutionPlan`
5. 这条 seam 的意义不在于“少了一个 struct”。它真正回答的是：
   batch runtime terminal lane 在 post-analysis terminal owner 已经存在后，
   quality-gate resolution 到底是不是还要回到邻层再转一遍单用途 owner。
   现在这条 duplicate 已被删掉，terminal owner 自身终于成为 batch runtime
   quality-gate resolution 的更连续 Rust materialization 边界。
6. 对 Phase 5 的价值同样直接：
   - cutover 审计时更容易回答“batch runtime quality-gate terminal 到底由哪个 Rust owner 接手”
   - terminal success / retry-budget load / quality-gate routing 现在共享更连续的 owner 链
   - fallback shrink / rollback / stronger smoke 在 batch runtime terminal 邻域上
     又少了一条藏在 local resolution owner 里的 forwarding 支路
7. 验证：
   - `cargo test chapter_batch_generation_runtime_state_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-runtime-quality-gate-terminal-owner" -- --nocapture`
   - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-runtime-quality-gate-terminal-owner"`

### 2026-06-05 阶段补充：batch runtime lifecycle-step 直接收口到 step owner

在 batch runtime terminal quality-gate 收口之后，本轮继续沿同一条
`chapter_batch_generation` Phase 5 主线，把 batch runtime lifecycle body 上
仍残留的 `lifecycle owner -> local step wrapper -> prepared-step owner`
生产 handoff 链再收掉一层。

1. `chapter_batch_generation` 的 batch runtime lifecycle-step 邻域已经有：
   - `PreparedBatchGenerationStepExecution::prepare(...)`
   - `PreparedBatchGenerationStepExecution::execute(...)`
   - step retry carry / generation-attempt continuation
2. 但 lifecycle 邻层之前仍会在外层重放同一组边界：
   - 本地 `preparation_retry_count` loop
   - 本地 retry-carry rebuild
   - 再把结果交回 prepared-step owner
3. 这代表 batch runtime lifecycle-step lane 在 Phase 5 上仍保留一条
   Python-era 的旧 hop：prepared-step owner 已经存在，但 lifecycle 邻层仍保留一条
   平行的 step orchestration forwarding 支路。
4. 本轮已把这条 batch runtime lifecycle-step direct-owner contract 真正前移回
   prepared-step owner 本身：
   - `backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs`
     里的 `PreparedBatchGenerationStepExecution` 现在直接拥有：
     - `start(...)`
   - 该 owner 现在直接：
     - prepare step
     - 复用 preparation-level retry carry
     - hand off to prepared-step execution
   - `BatchGenerationRuntimeLifecyclePlan::execute(...)` 不再本地 reopen
     `prepare -> carry retry -> execute`
   - 已确认 `execute_step(...)` 不再有生产引用，并在验证后直接删除，
     避免留下新的 dead wrapper
5. 这条 seam 的意义不在于“少了一层循环”。它真正回答的是：
   batch runtime lifecycle 一旦已经进入 step lane，retry-aware step entry
   到底是不是还要回到邻层再转一遍 local wrapper。现在这条 duplicate 已被删掉，
   prepared-step owner 本身终于成为 batch runtime lifecycle-step 的显式 Rust
   materialization 边界。
6. 对 Phase 5 的价值同样直接：
   - cutover 审计时更容易回答“batch runtime lifecycle-step 到底由哪个 Rust owner 接手”
   - step prepare / retry carry / prepared-step execution 现在共享更连续的 owner 链
   - fallback shrink / rollback / stronger smoke 在 batch runtime lifecycle 邻域上
     又少了一条藏在 local step wrapper 里的 forwarding 支路
7. 验证：
   - `rustfmt --edition 2021 backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs`
   - `cargo test chapter_batch_generation_runtime_state_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-runtime-lifecycle-step-owner" -- --nocapture`
   - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-runtime-lifecycle-step-owner"`

### 2026-06-05 阶段补充：batch runtime analysis-attempt 直接收口到 attempt owner resolution

在 batch runtime lifecycle-step 收口之后，本轮继续沿同一条
`chapter_batch_generation` Phase 5 主线，把 batch runtime follow-up analysis
attempt 上仍残留的 `attempt owner -> resolution owner`
生产 handoff 链再收掉一层。

1. `chapter_batch_generation` 的 batch runtime analysis-attempt 邻域已经有：
   - `BatchGenerationAnalysisAttemptPlan`
   - analysis-started snapshot persistence
   - prepared/fallback analysis execution
2. 但 analysis-attempt 邻层之前仍会在外层重放同一组边界：
   - materialize `BatchGenerationAnalysisAttemptResolutionPlan`
   - 再本地把 prepared/fallback 的结果交给该邻层
3. 这代表 batch runtime analysis-attempt lane 在 Phase 5 上仍保留一条
   Python-era 的旧 hop：attempt owner 已经存在，但 completion/retry resolution
   仍保留一条平行的 neighbor forwarding 支路。
4. 本轮已把这条 batch runtime analysis-attempt direct-resolution contract
   真正前移回 attempt owner 本身：
   - `backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs`
     里的 `BatchGenerationAnalysisAttemptPlan` 现在直接拥有：
     - `resolve_result(...)`
   - 该 owner 现在直接：
     - 持久化 started snapshot
     - 执行 prepared 或 fallback analysis
     - resolve completed snapshot 或 retry routing
   - 不再单独 materialize
     `BatchGenerationAnalysisAttemptResolutionPlan`
5. 这条 seam 的意义不在于“少了一个 struct”。它真正回答的是：
   batch runtime analysis-attempt 一旦已经进入 attempt owner，completion/retry
   resolution 到底是不是还要回到邻层再转一遍单用途 owner。现在这条 duplicate
   已被删掉，attempt owner 本身终于成为 batch runtime analysis-attempt
   resolution 的显式 Rust materialization 边界。
6. 对 Phase 5 的价值同样直接：
   - cutover 审计时更容易回答“batch runtime analysis-attempt resolution 到底由哪个 Rust owner 接手”
   - analysis-started / prepared-fallback execution / completion-retry routing
     现在共享更连续的 owner 链
   - fallback shrink / rollback / stronger smoke 在 batch runtime analysis 邻域上
     又少了一条藏在 local resolution owner 里的 forwarding 支路
7. 验证：
   - `rustfmt --edition 2021 backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs`
   - `cargo test chapter_batch_generation_runtime_state_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-runtime-analysis-attempt-direct-resolution-owner" -- --nocapture`
   - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-runtime-analysis-attempt-direct-resolution-owner"`

### 2026-06-05 阶段补充：batch runtime step-generation-attempt 直接收口到 prepared-step owner

在 batch runtime analysis-attempt direct-resolution 收口之后，本轮继续沿同一条
`chapter_batch_generation` Phase 5 主线，把 prepared-step lane 上仍残留的
`prepared-step owner -> generation-attempt wrapper -> success-chain wrapper`
生产 handoff 链再收掉一层。

1. `chapter_batch_generation` 的 batch runtime prepared-step 邻域已经有：
   - `PreparedBatchGenerationStepExecution::start(...)`
   - `PreparedBatchGenerationStepExecution::execute(...)`
   - retry-aware step entry
2. 但 prepared-step 邻层之前仍会在外层重放同一组边界：
   - materialize `execute_generation_attempt(...)`
   - materialize `execute_success_chain(...)`
   - 再本地把 generation 与 post-analysis 链转给这些邻层
3. 这代表 batch runtime prepared-step lane 在 Phase 5 上仍保留一条
   Python-era 的旧 hop：prepared-step owner 已经存在，但 generation-attempt
   主链仍保留两条平行的 local wrapper forwarding 支路。
4. 本轮已把这条 batch runtime step-generation-attempt direct-owner contract
   真正前移回 prepared-step owner 本身：
   - `backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs`
     里的 `PreparedBatchGenerationStepExecution::execute(...)` 现在直接：
     - persist chapter-started
     - run prerequisite gate
     - prepare attempt input
     - execute generation
     - run post-write guard
     - run follow-up analysis
     - route terminal outcome
   - 不再单独 materialize
     `execute_generation_attempt(...)`
   - 不再单独 materialize
     `execute_success_chain(...)`
5. 这条 seam 的意义不在于“少了两个 helper”。它真正回答的是：
   batch runtime 一旦已经进入 prepared-step owner，generation-attempt 到
   post-analysis terminal 这整条生产链到底是不是还要回到邻层再转一遍。
   现在这条 duplicate 已被删掉，prepared-step owner 本身终于成为
   batch runtime step-generation-attempt 的显式 Rust materialization 边界。
6. 对 Phase 5 的价值同样直接：
   - cutover 审计时更容易回答“batch runtime step-generation-attempt 到底由哪个 Rust owner 接手”
   - step retry / prerequisite / attempt-input / generation / post-analysis
     现在共享更连续的 owner 链
   - fallback shrink / rollback / stronger smoke 在 batch runtime step 邻域上
     又少了两条藏在 local wrapper 里的 forwarding 支路
7. 验证：
   - `rustfmt --edition 2021 backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs`
   - `cargo test chapter_batch_generation_runtime_state_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-runtime-step-generation-attempt-direct-owner" -- --nocapture`
   - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-runtime-step-generation-attempt-direct-owner"`

### 2026-06-05 阶段补充：batch runtime attempt-input 直接收口到 generation owner

在 batch runtime step-generation-attempt direct-owner 收口之后，本轮继续沿
同一条 `chapter_batch_generation` Phase 5 主线，把 generation-attempt input
lane 上仍残留的
`attempt-input owner -> outer generate_and_persist... replay`
生产 handoff 链再收掉一层。

1. `chapter_batch_generation` 的 batch runtime generation-attempt input
   邻域已经有：
   - `BatchGenerationAttemptInputPlan::prepare(...)`
   - compat restore
   - prompt override materialization
   - provider payload preparation
2. 但 prepared-step 邻层之前仍会在外层重放同一组边界：
   - materialize `BatchGenerationAttemptInputPlan`
   - 再本地调用一次
     `generate_and_persist_chapter_content_with_provider_payload(...)`
3. 这代表 batch runtime attempt-input lane 在 Phase 5 上仍保留一条
   Python-era 的旧 hop：attempt-input owner 已经存在，但真正的 generation
   runtime call 仍回到邻层 replay，而不是留在这个 owner 自己的生产链里。
4. 本轮已把这条 batch runtime attempt-input direct-generation contract
   真正前移回 attempt-input owner 本身：
   - `backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs`
     里的 `BatchGenerationAttemptInputPlan::execute(...)` 现在直接拥有：
     - compat restore
     - prompt override materialization
     - provider payload preparation
     - generation execution call
5. 因此 prepared-step lane 不再需要在 owner 外部重放：
   - `prepare attempt input`
   - 再手动拼接 generation runtime call

   现在 attempt-input owner 本身终于成为 batch runtime
   `attempt-input -> generation execution` 的显式 Rust materialization
   边界。
6. 对 Phase 5 的价值同样直接：
   - cutover 审计时更容易回答“batch runtime generation attempt input 到底
     由哪个 Rust owner 接手”
   - compat restore / prompt override / provider payload / generation call
     现在共享更连续的 owner 链
   - fallback shrink / rollback / stronger smoke 在 batch runtime step
     邻域上又少了一条藏在 outer local replay 里的 generation 支路

### 2026-06-05 阶段补充：batch create route workflow-start 直接收口到 write-workflow owner

在 batch runtime attempt-input 收口之外，本轮继续沿同一条
`chapter_generation` / `chapter_batch_generation` Phase 5 主线，把 batch create
route edge 上仍残留的
`route payload -> local request rebuild -> write workflow start`
生产 handoff 链再收掉一层。

1. `chapter_batch_generation` 的 batch create route 邻域已经有：
   - `BatchGenerationCreateRouteRequest`
   - `BatchGenerationCreateWorkflowRequest`
   - batch create write-workflow owner
2. 但 create route 邻层之前仍会在外层重放同一组边界：
   - 本地
     `build_batch_generation_create_workflow_request_from_route_payload(...)`
   - 再本地调用
     `start_owned_batch_generation_write_workflow(...)`
3. 这代表 batch create route lane 在 Phase 5 上仍保留一条 Python-era 的旧 hop：
   write-workflow owner 已经存在，但 route transport 邻层仍保留一条平行的
   request normalization / workflow-start forwarding 支路。
4. 本轮已把这条 batch create route workflow-start contract 真正前移回
   write-workflow owner 本身：
   - `backend-rs/src/services/chapter_batch_generation_write_workflow_service.rs`
     现在直接拥有：
     - `build_batch_generation_create_workflow_request_from_route_payload(...)`
     - `start_owned_batch_generation_write_workflow_from_route_payload(...)`
   - `backend-rs/src/api/chapter_batch_generation.rs`
     里的 `create_batch_generate(...)`
     现在直接把 route payload 交给该 owner，不再本地 reopen
     `build_batch_generation_create_workflow_request_from_route_payload(...) ->
     start_owned_batch_generation_write_workflow(...)`
5. 这条 seam 的意义不在于“少了一次 request builder 调用”。它真正回答的是：
   batch create route 一旦已经把 transport payload 交给 Rust owner，
   route 边界到底是不是还要保留一条本地 workflow-start rebuild 支路。
   现在这条 duplicate 已被删掉，write-workflow owner 本身终于成为
   batch create route workflow-start 的显式 Rust materialization 边界。
6. 对 Phase 5 的价值同样直接：
   - cutover 审计时更容易回答“batch create route workflow-start 到底由哪个 Rust owner 接手”
   - route payload normalization / create workflow-start / write-lane entry
     现在共享更连续的 owner 链
   - fallback shrink / rollback / stronger smoke 在 batch create route 邻域上
     又少了一条藏在 transport 邻层里的 local rebuild 支路
7. 验证：
   - `rustfmt --edition 2021 "backend-rs/src/api/chapter_batch_generation.rs" "backend-rs/src/services/chapter_batch_generation_write_workflow_service.rs"`
   - `cargo test chapter_batch_generation_write_workflow_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-route-workflow-start-owner" -- --nocapture`
   - `cargo test chapter_batch_generation --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-route-workflow-start-owner" -- --nocapture`

### 2026-06-05 阶段补充：batch create route-start 空壳层收口

在 batch create route workflow-start 已经直接收口到 write-workflow owner
之后，本轮继续沿同一条 `chapter_generation` / `chapter_batch_generation`
Phase 5 主线，把 create route lane 上仍残留的
`RouteStart wrapper -> write-workflow owner` 空壳 hop 再收掉一层。

1. `chapter_batch_generation` 的 batch create route 邻域已经有：
   - route payload transport contract
   - `build_batch_generation_create_workflow_request_from_route_payload(...)`
   - batch create write-workflow public-start owner
2. 但 create route 邻层之前仍会在外层保留一个额外 wrapper：
   - `BatchGenerationCreateWorkflowRouteStart`
3. 这代表 batch create route lane 在 Phase 5 上仍保留一条
   Python-era 的旧 hop：
   write-workflow owner 已经存在，但其邻层仍平行保留一条只负责把
   route payload 转成 request 后再转手交给同一 owner 的 forwarding shell。
4. 本轮已把这条 batch create route-start contract 真正前移回
   write-workflow owner 本身：
   - `backend-rs/src/services/chapter_batch_generation_write_workflow_service.rs`
     不再保留：
     - `BatchGenerationCreateWorkflowRouteStart`
   - create lane 现在直接停在：
     - `build_batch_generation_create_workflow_request_from_route_payload(...)`
     - `start_owned_batch_generation_write_workflow_from_route_payload(...)`
     - `start_owned_batch_generation_write_workflow(...)`
5. 这一步的迁移意义在于：
   batch create route edge 形状进一步收紧，后续再做 create lane cutover
   readiness 审计时，不会继续把一个不拥有 validation、error translation、
   branch selection 的空壳 route-start wrapper 误判为真实 owner。
6. 因此这一步的 stop-rule 也更明确：
   - 不要在 route-payload builder 和 write-workflow owner 都已经存在之后，
     继续保留只负责 forwarding 的 `RouteStart` shell；
   - 对这类壳，若它不再新增 access / validation / branch / error 语义，
     就应该直接收口回相邻 owner，而不是为了“层次对称”继续保留。
7. 这一轮 focused validation 已通过：
   - `cargo test chapter_batch_generation_write_workflow_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-create-route-start-collapse" -- --nocapture`
   - `cargo test chapter_batch_generation --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-create-route-start-collapse" -- --nocapture`
   - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-create-route-start-collapse"`

### 2026-06-05 阶段补充：batch active-task-list route-query 直接收口到 query owner

在 batch create route workflow-start 收口之后，本轮继续沿同一条
`chapter_generation` / `chapter_batch_generation` Phase 5 主线，把 batch
active-task-list route edge 上仍残留的
`route query -> local request rebuild -> query owner start`
生产 handoff 链再收掉一层。

1. `chapter_batch_generation` 的 active-task-list route 邻域已经有：
   - route query transport contract
   - `ActiveBatchGenerationTaskListQueryRequest`
   - active-task query/view owner
2. 但 active-task-list route 邻层之前仍会在外层重放同一组边界：
   - 本地
     `build_active_batch_generation_task_list_query_request(...)`
   - 再本地调用
     `load_active_batch_generation_task_list(...)`
3. 这代表 batch active-query lane 在 Phase 5 上仍保留一条 Python-era 的旧 hop：
   query owner 已经存在，但 route transport 邻层仍保留一条平行的
   request normalization / query-start forwarding 支路。
4. 本轮已把这条 batch active-task-list route-query contract 真正前移回
   query owner 本身：
   - `backend-rs/src/services/chapter_batch_generation_task_view_query_service.rs`
     现在直接拥有：
    - `ActiveBatchGenerationTaskListRouteQuery`
    - `build_active_batch_generation_task_list_query_request_from_route_query(...)`
    - `ActiveBatchGenerationTaskListRouteQueryError`
    - `load_active_user_batch_generation_task_list_view_from_route_query(...)`
   - `backend-rs/src/api/chapter_batch_generation_error_mapper.rs`
     现在直接拥有：
     - `map_active_batch_generation_task_list_route_error(...)`
   - `backend-rs/src/api/chapter_batch_generation.rs`
     里的 `list_active_batch_generation_tasks(...)`
     现在直接把 route query 交给该 owner，不再本地 reopen
     `build_active_batch_generation_task_list_query_request(...) ->
     load_active_batch_generation_task_list(...)`
5. 这条 seam 的意义也不在于“少了一次 query builder 调用”。它真正回答的是：
   batch active-task-list route 一旦已经把 transport query 交给 Rust owner，
   route 边界到底是不是还要保留一条本地 query-start rebuild 支路。
   现在这条 duplicate 已被删掉，query owner 本身终于成为
   batch active-task-list route-query 的显式 Rust materialization 边界。
6. 对 Phase 5 的价值同样直接：
   - cutover 审计时更容易回答“batch active-task-list route-query 到底由哪个 Rust owner 接手”
   - route-query normalization / active-task query start / error mapping
     现在共享更连续的 owner 链
   - fallback shrink / rollback / stronger smoke 在 batch active-query 邻域上
     又少了一条藏在 transport 邻层里的 local rebuild 支路
7. 验证：
   - `rustfmt --edition 2021 "backend-rs/src/services/chapter_batch_generation_task_view_query_service.rs" "backend-rs/src/api/chapter_batch_generation_error_mapper.rs" "backend-rs/src/api/chapter_batch_generation.rs"`
   - `cargo test chapter_batch_generation_task_view_query_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-active-task-list-route-owner" -- --nocapture`
   - `cargo test chapter_batch_generation_error_mapper --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-active-task-list-route-owner" -- --nocapture`
   - `cargo test chapter_batch_generation --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-active-task-list-route-owner" -- --nocapture`
   - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-active-task-list-route-owner"`

### 2026-06-05 阶段补充：batch active-project route-query 直接收口到 query owner

在 batch active-task-list route-query 收口之后，本轮继续沿同一条
`chapter_generation` / `chapter_batch_generation` Phase 5 主线，把 batch
active-project route edge 上仍残留的
`route path -> local query start -> query owner`
生产 handoff 链再收掉一层。

1. `chapter_batch_generation` 的 active-project route 邻域已经有：
   - route path transport contract
   - project-access query owner
   - active-project payload view owner
2. 但 active-project route 邻层之前仍会在外层重放同一组边界：
   - 本地 route path `project_id` handoff
   - 再本地调用 `load_active_batch_generation_query(...)`
3. 这代表 batch active-project lane 在 Phase 5 上仍保留一条 Python-era 的旧 hop：
   query owner 已经存在，但 route transport 邻层仍保留一条平行的
   query-start forwarding / error mapping 支路。
4. 本轮已把这条 batch active-project route-query contract 真正前移回
   query owner 本身：
   - `backend-rs/src/services/chapter_batch_generation_task_view_query_service.rs`
     现在直接拥有：
    - `ActiveProjectBatchGenerationRouteError`
    - `load_active_batch_generation_view_from_route_project(...)`
   - `backend-rs/src/api/chapter_batch_generation_error_mapper.rs`
     现在直接拥有：
     - `map_active_project_batch_generation_route_error(...)`
   - `backend-rs/src/api/chapter_batch_generation.rs`
     里的 `get_active_batch_generation(...)`
     现在直接把 route path 交给该 owner，不再本地 reopen
     `project_id -> load_active_batch_generation_query(...)`
5. 这条 seam 的意义也不在于“少了一次 query 调用包装”。它真正回答的是：
   batch active-project route 一旦已经把 transport path 交给 Rust owner，
   route 边界到底是不是还要保留一条本地 query-start forwarding 支路。
   现在这条 duplicate 已被删掉，query owner 本身终于成为
   batch active-project route-query 的显式 Rust materialization 边界。
6. 对 Phase 5 的价值同样直接：
   - cutover 审计时更容易回答“batch active-project route-query 到底由哪个 Rust owner 接手”
   - route-project handoff / active-project query start / error mapping
     现在共享更连续的 owner 链
   - fallback shrink / rollback / stronger smoke 在 batch active-project 邻域上
     又少了一条藏在 transport 邻层里的 local forwarding 支路
7. 验证：
   - `rustfmt --edition 2021 "backend-rs/src/services/chapter_batch_generation_task_view_query_service.rs" "backend-rs/src/api/chapter_batch_generation_error_mapper.rs" "backend-rs/src/api/chapter_batch_generation.rs"`
   - `cargo test chapter_batch_generation_task_view_query_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-active-project-route-owner" -- --nocapture`
   - `cargo test chapter_batch_generation_error_mapper --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-active-project-route-owner" -- --nocapture`
   - `cargo test chapter_batch_generation --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-active-project-route-owner" -- --nocapture`
   - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-active-project-route-owner"`

### 2026-06-05 阶段补充：batch query route-start 空壳层收口

在 batch active-task-list / active-project route-query 已经直接收口到 query owner
之后，本轮继续沿同一条 `chapter_generation` / `chapter_batch_generation`
Phase 5 主线，把 task-view query lane 上仍残留的
`route-start wrapper -> route-query owner` 空壳 hop 再收掉一层。

1. `chapter_batch_generation` 的 task-view query 邻域已经有：
   - active-task-list route-query transport contract
   - active-project route path transport contract
   - active-task / active-project query owner
   - shared route-query error mapping
2. 但这两条 lane 之前仍会在 query owner 邻层各自保留一个额外 wrapper：
   - `ActiveBatchGenerationTaskListRouteStart`
   - `ActiveProjectBatchGenerationRouteStart`
3. 这代表 batch task-view query lane 在 Phase 5 上仍保留一条
   Python-era 的旧 hop：
   route-query owner 已经存在，但其邻层仍平行保留一条只负责把
   route-normalized query/path 值再转手交给同一 owner 的 forwarding shell。
4. 本轮已把这条 batch query route-start contract 真正前移回
   route-query owner 本身：
   - `backend-rs/src/services/chapter_batch_generation_task_view_query_service.rs`
     不再保留：
     - `ActiveBatchGenerationTaskListRouteStart`
     - `ActiveProjectBatchGenerationRouteStart`
   - active-task-list lane 现在直接停在：
     - `ActiveBatchGenerationTaskListRouteQuery`
     - `build_active_batch_generation_task_list_query_request_from_route_query(...)`
     - `load_active_user_batch_generation_task_list_view_from_route_query(...)`
   - active-project lane 现在直接停在：
     - `load_active_batch_generation_view_from_route_project(...)`
     - `ActiveProjectBatchGenerationRouteError`
5. 这一步的迁移意义在于：
   batch task-view query owner 形状进一步收紧，后续再做 active 查询 cutover
   readiness 审计时，不会继续把一个不拥有 validation、branch selection、
   error translation 的空壳 route-start wrapper 误判为真实 owner。
6. 因此这一步的 stop-rule 也更明确：
   - 不要在 route-query owner 已经直接接收 transport-normalized 输入之后，
     继续保留只负责 forwarding 的 `RouteStart` shell；
   - 对这类壳，若它不再新增 validation / access / error / branch 语义，
     就应该直接收口回相邻 owner，而不是为了“层次对称”继续保留。
7. 这一轮 focused validation 已通过：
   - `cargo test chapter_batch_generation_task_view_query_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-query-route-start-collapse" -- --nocapture`
   - `cargo test chapter_batch_generation --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-query-route-start-collapse" -- --nocapture`
   - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-query-route-start-collapse"`

### 2026-06-05 阶段补充：batch status payload / status stream 共享 owned read-state owner

在 batch active-project route-query 收口之后，本轮继续沿同一条
`chapter_generation` / `chapter_batch_generation` Phase 5 主线，把 batch
status payload 与 status stream 两条邻接 read lane 之间仍残留的平行
`task -> recover -> snapshot`
生产链再收掉一层。

1. `chapter_batch_generation` 的 status-query / status-stream 邻域已经各自有：
   - owned status payload owner
   - owned stream-state owner
   - 下游 payload / event / stream transport owner
2. 但这两条 lane 之前仍会各自在外层重放同一组读侧边界：
   - `load_owned_task(...)`
   - `recover_batch_generation_task_if_needed(...)`
   - `load_batch_generation_snapshot(...)`
3. 这代表 batch status-query / status-stream 邻域在 Phase 5 上仍保留一条
   Python-era 的旧 hop：
   最终 payload/event owner 虽然已经分开明确，但它们上游仍平行保留一条
   duplicate 的 owned read-state materialization 支路。
4. 本轮已把这条 shared batch owned read-state contract 真正前移回
   shared owner 本身：
   - `backend-rs/src/services/chapter_batch_generation_owned_task_query_service.rs`
     现在直接拥有：
     - `OwnedBatchGenerationTaskReadState`
     - `load_owned_batch_generation_task_read_state(...)`
   - `backend-rs/src/services/chapter_batch_generation_status_task_query_service.rs`
     现在直接从该 owner 消费 shared read-state，再投影 status payload
   - `backend-rs/src/services/chapter_batch_generation_stream_state_query_service.rs`
     现在也直接从该 owner 消费 shared read-state，再投影 stream state
5. 这条 seam 的意义不在于“少写了几次 load/recover/snapshot 调用”。它真正回答的是：
   batch status payload 和 status stream 一旦已经都依赖同一组 owned 读侧来源，
   这些来源到底是不是还要在两个邻近 helper 里平行重放两遍。
   现在这条 duplicate 已被删掉，shared owner 本身终于成为
   batch status-query / status-stream 的显式 Rust read-state materialization 边界。
6. 对 Phase 5 的价值同样直接：
   - cutover 审计时更容易回答“batch status payload / status stream 的 owned read-state 到底由哪个 Rust owner 接手”
   - owned task load / active-timeout recovery / snapshot materialization
     现在共享更连续的 owner 链
   - fallback shrink / rollback / stronger smoke 在 batch status-query /
     status-stream 邻域上又少了一条藏在 parallel helper 里的 local rebuild 支路
7. 验证：
   - `rustfmt --edition 2021 "backend-rs/src/services/chapter_batch_generation_owned_task_query_service.rs" "backend-rs/src/services/chapter_batch_generation_status_task_query_service.rs" "backend-rs/src/services/chapter_batch_generation_stream_state_query_service.rs"`
   - `cargo test chapter_batch_generation_owned_task_query_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-owned-read-state-owner" -- --nocapture`
   - `cargo test chapter_batch_generation_status_task_query_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-owned-read-state-owner" -- --nocapture`
   - `cargo test chapter_batch_generation_stream_state_query_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-owned-read-state-owner" -- --nocapture`
   - `cargo test chapter_batch_generation --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-owned-read-state-owner" -- --nocapture`
   - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-owned-read-state-owner"`
   - `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-route-workflow-start-owner"`

### 2026-06-05 阶段补充：batch status/stream read-state projection 空壳层收口

在 batch owned read-state 已经先收口到 shared owner 之后，本轮继续沿同一条
`chapter_batch_generation` Phase 5 主线，把 status payload lane /
status stream lane 上仍残留的
`shared read-state -> local projection wrapper -> final owner`
空壳 hop 再收掉一层。

这一步的前提不是“status 和 stream 还没有共享读侧来源”，而恰恰相反：
shared owned read-state boundary 已经存在，`task -> recover -> snapshot`
也已经由
`backend-rs/src/services/chapter_batch_generation_owned_task_query_service.rs`
统一物化。问题变成了：

1. status lane 明明已经拿到同一个 `OwnedBatchGenerationTaskReadState`，
   为什么还要再包一层 `PreparedOwnedBatchGenerationStatusPayloadQuery`
   才去投影最终 payload。
2. stream lane 明明也已经拿到同一个 shared read-state，
   为什么还要再经过
   `build_batch_generation_stream_state_from_read_state(...)`
   这一层空壳 helper，才交回最终 stream-state owner。

这两个 wrapper 在当前 owner 结构里已经不再承担新的职责：
- 不增加 access control
- 不增加 recovery
- 不增加 error translation
- 不增加新的 semantic branching

它们只是把已经 materialized 的 shared read-state 再转手一次。

因此本轮继续把 owner boundary 收紧：

1. `backend-rs/src/services/chapter_batch_generation_status_task_query_service.rs`
   现在直接从 shared read-state owner 投影最终 status payload：
   - `load_owned_batch_generation_status_payload(...)`
   - `build_owned_batch_generation_status_payload_from_read_state(...)`
2. `backend-rs/src/services/chapter_batch_generation_stream_state_query_service.rs`
   现在也直接从同一个 shared read-state owner 投影最终 stream state：
   - `load_owned_batch_generation_stream_state(...)`
   - `OwnedBatchGenerationTaskReadState::into_parts()`
   - `build_batch_generation_stream_state_for_task_and_snapshot(...)`

这条 seam 的价值依然是 cutover readiness，而不是局部“代码变短了”：

1. batch status payload / status stream 现在都从同一个显式 Rust
   read-state owner 出发，并直接落到最终 projection owner，
   中间不再夹一层 Python-era 兼容空壳 hop。
2. batch status-query / status-stream 邻域的 owner map 更容易审计：
   - shared read-state owner 负责来源物化
   - status lane 负责 payload projection
   - stream lane 负责 stream-state projection
3. fallback shrink / rollback / stronger smoke 在这组相邻读侧 lane 上
   又少了一层“看似有 owner、其实只是在 forwarding”的局部重放边界。

验证：
- `cargo test chapter_batch_generation_status_task_query_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-status-stream-read-state-collapse" -- --nocapture`
- `cargo test chapter_batch_generation_stream_state_query_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-status-stream-read-state-collapse" -- --nocapture`
- `cargo test chapter_batch_generation --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-status-stream-read-state-collapse" -- --nocapture`
- `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-status-stream-read-state-collapse"`

### 2026-06-05 阶段补充：batch task-view prepared-query 空壳层收口

在 batch task-view route-query owner 和 shared read-side owner 都已经比较清晰后，
本轮继续沿同一条 `chapter_batch_generation` Phase 5 主线，把
`task-view query owner -> Prepared* wrapper -> final payload`
这一层残留空壳 hop 整块收掉。

这里的关键不再是“route 有没有交到 Rust owner 手里”，而是：
既然 `chapter_batch_generation_task_view_query_service.rs` 自己已经拥有
active task 加载、project access gating、existing-background task 搜索、
以及最终 payload 投影，为什么中间还要保留一层
`Prepared* -> into_payload` 的局部兼容壳。

本轮收掉的不是单个 helper，而是一组相邻 query/view wrapper：

1. `PreparedActiveBatchGenerationTaskListView`
2. `PreparedActiveProjectBatchGenerationQuery`
3. `PreparedExistingSingleGenerationBackgroundTaskPayloadQuery`

它们在当前 owner 结构里已经不再承担新的职责：
- 不增加 access control
- 不增加 request normalization
- 不增加 error translation
- 不增加新的 branch selection

它们只是把同一文件里已经 load 完的 task / payload 再包成
`prepare -> into_payload` 一次。

因此本轮继续把 task-view owner boundary 收紧：

1. active-task-list lane 现在直接停在：
   - `load_active_user_batch_generation_task_list_view(...)`
   - `build_active_batch_generation_task_list_view_payload(...)`
2. active-project lane 现在直接停在：
   - `load_active_batch_generation_query(...)`
   - `build_active_project_batch_generation_view_payload(...)`
3. existing single-background lane 现在直接停在：
   - `load_existing_single_generation_background_task_payload(...)`
   - `load_existing_single_generation_background_task_payload_for_tasks(...)`

这条 seam 的价值仍然是 cutover readiness，而不是局部“函数变少了”：

1. batch task-view query 邻域现在对 active-task-list / active-project /
   existing-background 三条分支都保留了更直接的
   `query owner -> final payload projection` 链。
2. 这让 task-view owner map 更容易审计：
   - transport route-query owner
   - task-view query owner
   - final payload projection
   三层职责更连续，不再夹一层没有真实边界意义的 Prepared hop。
3. fallback shrink / rollback / stronger smoke 在 batch task-view 邻域上
   又少了一条 Python-era 兼容壳层，后续继续整块迁移会更顺。

验证：
- `cargo test chapter_batch_generation_task_view_query_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-task-view-prepared-collapse" -- --nocapture`
- `cargo test chapter_batch_generation --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-task-view-prepared-collapse" -- --nocapture`
- `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-task-view-prepared-collapse"`

### 2026-06-05 阶段补充：batch resume launch-sources 空壳层收口

在 batch resume restored-state launch owner 已经先收口之后，本轮继续沿同一条
`chapter_batch_generation` Phase 5 主线，把
`resume command -> PreparedLaunchSources -> launch persistence owner`
这一层残留空壳 hop 再收掉一层。

这里的关键不是 resume lane 还缺少恢复态 owner，而是：
既然 `chapter_batch_generation_resume_task_command_service.rs` 自己已经拥有

1. status gating
2. restored runtime-state recovery
3. manual-review blocker 检查
4. validated execution selection
5. reset persistence / dispatch-ready payload materialization

为什么中间还要保留一层
`PreparedBatchGenerationResumeLaunchSources`
只负责把前半段结果再转手一次。

这层 wrapper 在当前 owner 结构里已经不再承担新的职责：
- 不增加 access control
- 不增加 request validation
- 不增加 error translation
- 不增加 dispatch branch selection

它只是把已经 materialized 的 restored-state sources 再做一次
`prepare -> into launch persistence plan` 的 forwarding。

因此本轮继续把 batch resume owner boundary 收紧：

1. `chapter_batch_generation_resume_task_command_service.rs`
   现在先直接通过：
   - `prepare_resume_launch_restored_state(...)`
   产出 restored runtime-state 与 existing workflow runtime-state
2. 之后直接由：
   - `BatchGenerationResumeLaunchPersistencePlan::prepare(...)`
   - `BatchGenerationResumeLaunchPersistencePlan::prepare_from_validated_execution(...)`
   接手 validated execution / reset persistence / response payload /
   dispatch plan 的最终 materialization

这条 seam 的价值依然是 cutover readiness，而不是局部“少了一个 struct”：

1. batch resume command lane 现在对恢复态来源和最终 launch persistence
   materialization 保持了更直接的 Rust owner chain。
2. 这让 resume owner map 更容易审计：
   - restored-state owner
   - validated execution owner
   - launch-persistence owner
   三层职责连续，不再夹一层没有真实边界意义的 Prepared hop。
3. fallback shrink / rollback / stronger smoke 在 batch resume 邻域上
   又少了一层 Python-era forwarding 壳层，后续继续做整块迁移更顺。

验证：
- `cargo test chapter_batch_generation_resume_task_command_service --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-resume-launch-sources-collapse" -- --nocapture`
- `cargo test chapter_batch_generation --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-resume-launch-sources-collapse" -- --nocapture`
- `cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "D:/codex-targets/batch-resume-launch-sources-collapse"`

 ---

 ## 8. 当前成功标准

 当前阶段满足以下条件，才可视为进入“可持续推进 Rust 迁移”的状态：

 - 部署脚本具备结构化 gateway smoke 步骤
 - Rust 与 Python fallback 至少各有一个 through-gateway 探针
 - smoke 结果落盘到 `tmp/smoke/`
 - 探针失败时部署脚本会终止并保留诊断
 - 关键 route group 已具备 owner / fallback / asymmetric 的最小治理资产
 - `backend-rs` 的新切片可以在不改变外部行为的前提下持续落地并验证
