# PRD：后台任务恢复策略注册表

## 目标

为 Rust 后台任务按 `task_type` 建立唯一的恢复策略注册表，替换当前“所有
pending/running 任务在服务启动时统一标记 failed”的无差别处理。恢复结果必须保留诊断
信息，并通过现有 `TaskRecord`/后台任务 API 向前端提供准确、可操作的终态语义。

## 用户价值

- 服务重启后，用户能够区分“可重新发起”“可从 checkpoint 恢复”“需要人工确认”和
  “无法恢复”。
- 可能已经产生部分数据库写入的任务不会被静默自动重放，降低重复写入和覆盖风险。
- 已有章节 checkpoint/resume 能力继续由原 owner 承担，不会产生第二套恢复系统。
- 未知任务类型使用安全默认策略，不会因为漏登记而错误声明可恢复。

## 已确认事实

1. 启动顺序为 snapshot load → `recover_orphan_tasks()` → periodic save。
2. `backend-rs/src/tasks/recovery.rs` 当前将所有 pending/running 任务统一改为 failed，并写入
   相同错误和 checkpoint。
3. JSON `TaskRecord` 不保存原始请求 payload，generic 任务无法在启动阶段安全自动重放。
4. `chapters_batch_generate` 和 `chapter_single_generate` 已有独立数据库 runtime-state、
   checkpoint 和 resume owner。
5. 前端 `BackgroundTaskStatus` 已支持 `terminal_reason`、`terminal_label`、
   `review_required` 和 `can_resume`，但 Rust `TaskRecord` 尚未提供这些可选字段。
6. 前端只把章节批量和单章生成视为可执行 resume 的任务，与现有后端 resume owner 一致。
7. 通用 `/background-tasks` 列表和详情直接序列化 `TaskRecord`，增加可选字段即可复用现有 API，
   不需要新增 endpoint。
8. snapshot version 1 使用 serde JSON；新增缺省为 `None` 的可选字段可以兼容旧快照。

## 恢复策略

### `restartable`

表示该任务可由用户从原业务入口安全重新发起，但 R2 不在启动时自动重放，因为 snapshot
没有保存原始 payload。

登记类型：

- `chapter_analysis`
- `inspiration_generate_options`
- `inspiration_refine_options`
- `inspiration_quick_generate`
- `polish_text`

恢复投影：failed、`terminal_reason=restart_required`、`review_required=false`、
`can_resume=false`，错误和消息明确提示重新发起。

### `checkpoint_resumable`

表示已有业务 owner 能从 checkpoint 创建恢复命令；R2 只投影恢复能力，不执行 resume。

登记类型：

- `chapters_batch_generate`
- `chapter_single_generate`

存在非空 checkpoint 时投影：failed、`terminal_reason=resume_available`、
`review_required=false`、`can_resume=true`。checkpoint 缺失或不是对象时必须降级为
`terminal_reason=checkpoint_missing`、`can_resume=false`。

### `manual_confirmation`

表示任务可能已经产生部分持久化副作用，自动重放可能重复写入或覆盖用户后续修改。

登记类型：

- `chapter_regenerate`
- `chapter_partial_regenerate`
- `book_import_apply`
- `book_import_retry_failed_steps`
- `polish_batch`
- `careers_generate_system`
- `character_generate`
- `organization_generate`
- `world_regenerate`
- `outline_generate`
- `outline_expand`
- `outline_batch_expand`
- `wizard_world_building`
- `wizard_career_system`
- `wizard_characters`
- `wizard_outline`

恢复投影：failed、`terminal_reason=manual_review`、`review_required=true`、
`can_resume=false`，消息要求用户检查已生成内容后决定是否重新执行。

### `non_resumable`

所有未登记类型使用该安全默认策略，包括 `unknown`。恢复投影为 failed、
`terminal_reason=non_resumable`、`review_required=false`、`can_resume=false`。

## 功能要求

1. 新增集中式 `TaskRecoveryPolicy` 和静态策略注册表，禁止在多个调用点复制 match 表。
2. 注册表必须为每个已知后台任务类型返回确定策略，并对未知类型安全降级。
3. `TaskRecord` 新增四个可选恢复语义字段，旧 version-1 快照缺失字段时必须正常加载。
4. 新建任务必须将恢复语义字段初始化为 `None`，active 状态不得携带陈旧终态语义。
5. 启动恢复只处理 pending/running；completed/failed/cancelled 必须保持不变。
6. 恢复必须保留既有 result、progress 和 checkpoint 业务字段，只追加/更新恢复诊断键。
7. checkpoint 诊断至少记录 `event=orphan_recovery`、`recovery_policy`、
   `terminal_reason`、`can_resume`、`review_required` 和是否已有 result。
8. 每条恢复日志只记录 task id/type/policy/status，不得输出 payload、result 或 checkpoint 内容。
9. 不新增数据库迁移、API endpoint、任务状态枚举或自动执行线程。
10. 前端继续复用现有字段和 resume owner；R2 不创建第二套 task store。
11. 所有新增文本和代码保持 UTF-8 无 BOM。

## 验收标准

- [x] 四种策略均有独立单元测试。
- [x] 策略注册表不存在重复 task type，并覆盖 24 个当前已知生产类型（当前 registry 基线）。
- [x] 未知 task type 明确降级为 non_resumable。
- [x] restartable 孤儿任务提示重新发起且 `can_resume=false`。
- [x] 有 checkpoint 的章节任务投影 `resume_available` 且 `can_resume=true`。
- [x] checkpoint 缺失的章节任务不会错误声明可恢复。
- [x] manual_confirmation 任务投影 `manual_review` 且 `review_required=true`。
- [x] non_resumable 任务保留诊断并明确不可恢复。
- [x] pending 和 running 均被处理，三个既有终态均不被修改。
- [x] 既有 checkpoint 自定义字段、result 和 progress 不丢失。
- [x] 旧 version-1 snapshot 在缺少新增字段时可以反序列化。
- [x] `/background-tasks` payload 自动包含非空恢复语义字段且保持 success/data 兼容壳。
- [x] targeted tests、完整 Rust tests、fmt、check、Clippy 增量门禁和前端 build 通过。

## 非目标

- 不在 R2 中持久化原始请求 payload 或自动重放 generic 任务。
- 不实现新的章节 resume endpoint；继续使用已有章节 runtime-state owner。
- 不实现业务 checkpoint schema 标准化，该能力属于 R6。
- 不新增 `paused`、`recovering` 等 TaskStatus。
- 不修改 PostgreSQL 章节批量任务表或 migration。
- 不在本任务中清理历史 Clippy warning。

## 兼容性约束

- `TaskStatus` 字符串和现有 API 路径保持不变。
- `TaskRecord::new()` 签名保持不变。
- 新增字段必须为 optional，并在 active/new record 中为空。
- snapshot version 继续保持 `1`；旧 JSON 必须向前兼容加载。
- R1 的 primary/backup/temp 文件协议不得改变。

## 开放问题

无阻塞问题。路线文档已经确定四级策略；代码证据明确 R2 不具备安全自动重放 generic
payload 的条件，因此采用“分类并投影可操作终态”的保守 MVP。
