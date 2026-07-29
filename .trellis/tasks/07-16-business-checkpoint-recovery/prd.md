# Business Checkpoint 标准与恢复验证

## Goal

实现优化路线 R6：在现有批量章节生成 runtime snapshot 内建立版本化、可校验的业务
checkpoint 标准，并以“章节草稿已保存”为首个业务边界验证 create → 持久化 → resume
恢复链路。checkpoint 必须复用 R4 `input_digest`、现有 task/workflow state 和 snapshot
持久化 owner，不创建第二套任务系统或项目状态事实。

## User Value

- 长流程中断后可以从已完成的业务结果继续，而不是依赖进程内状态或任意 Token 位置。
- 恢复动作能够判断旧 checkpoint 是否属于当前输入、是否仍指向有效输出。
- 重试和重复写入具备稳定幂等身份，不会让 checkpoint revision 倒退。
- 旧任务、旧 runtime state、旧页面和旧 API 在缺少 R6 字段时继续工作。

## Confirmed Facts

- R4 已在 batch `workflow_runtime_state.generation_contract_snapshot.input_digest` 保存 canonical
  输入摘要，并能在 resume context 中恢复 typed `GenerationContractSnapshotV1`。
- 批量章节生成成功边界统一经过
  `BatchGenerationRuntimePersistencePlan::chapter_succeeded`，此时章节正文已经持久化。
- `batch_generation_snapshots.workflow_runtime_state` 是现有持久化载体，且 snapshot 以
  `batch_task_id` 唯一；runtime state merge 会保留未被 incoming payload 覆盖的 additive 字段。
- resume 入口复用 `BatchGenerationPersistedRuntimeContext` 和现有 task reset/launch 语义。
- 当前路线禁止为 R6 新增生产 migration；checkpoint 必须 additive 写入现有 JSON state。

## Requirements

### R1. Versioned typed schema

- 定义唯一 `business-checkpoint/v1` owner，首个 boundary 为 `chapter_draft_saved`。
- 每个 checkpoint 必须包含：
  - `schema_version`
  - `boundary`
  - 正整数 `revision`
  - `idempotency_key`
  - R4 `input_digest`
  - typed `output_reference`
  - `recorded_at`
- 首个 output reference 仅允许 `{ "kind": "chapter", "id": "<chapter-id>" }`。
- checkpoint 存放在 `workflow_runtime_state.business_checkpoint`，不得占用或替换现有
  runtime `checkpoint` 投影。

### R2. Revision and idempotency

- 同一 batch task 内 revision 必须单调不减；首个实现使用已成功章节数作为业务 revision
  基础，并与已有 checkpoint revision 取最大值，禁止因 retry/resume 倒退。
- 幂等键必须由 typed allowlist 字段 canonical 计算，至少覆盖 task id、boundary、revision、
  R4 input digest 和 output reference。
- 相同 typed 输入重复构建必须得到相同幂等键；任一身份字段变化必须改变幂等键。
- 幂等键不得包含 Prompt、正文、API Key、Authorization、完整 URL、动态 diagnostics 或时间戳。

### R3. Batch success persistence

- `chapter_succeeded` persistence plan 负责构建并携带 business checkpoint；route、AI client
  或 runtime driver 不得旁路二次写 snapshot。
- 构建必须使用当前 snapshot 中已验证的 R4 generation contract input digest。
- 若 legacy task 缺少 generation contract，则保留旧成功持久化行为，不伪造 digest，也不 panic。
- business checkpoint 与 task stage/runtime checkpoint 通过同一 persistence owner 写入现有
  snapshot merge 链路。

### R4. Resume validation

- resume context 读取 business checkpoint 时必须区分：缺失、合法、未知 schema、格式非法。
- 旧 runtime state 缺字段时按 legacy 路径恢复；未知 schema 或格式非法不得 panic，也不得误认
  为可验证业务边界。
- 合法 checkpoint 只有在以下条件全部满足时才可作为已验证恢复证据：
  - checkpoint `input_digest` 与当前恢复的 R4 contract digest 一致；
  - output reference 指向当前项目/任务范围内存在的章节；
  - 章节正文 trim 后非空。
- digest mismatch 或 dangling/empty output 必须返回 typed 恢复错误并阻止 runtime launch，不能
  静默从失效 checkpoint 继续。

### R5. Compatibility and security

- 不新增数据库 migration、任务表、checkpoint 表、SSE event kind 或公开 API 必填字段。
- 不删除/重命名现有 `workflow_runtime_state.checkpoint`、task status 或 resume payload 字段。
- persisted checkpoint 必须由 typed schema 序列化，读取只接受字段白名单；不得透传任意 JSON。
- checkpoint、错误和日志不得保存或输出 Prompt、正文、API Key、Authorization 或完整 URL。
- 文件保持 UTF-8 无 BOM、LF-only、无 trailing whitespace。

### R6. Validation scope

- 至少覆盖一个真实 DB-backed 长流程：创建 batch snapshot/contract → 成功保存章节 → 写入
  business checkpoint → 构造 resume → 验证 checkpoint 并恢复后续流程上下文。
- 覆盖 legacy 缺字段、未知 schema、digest mismatch、output 缺失、正文为空、重复写入和
  revision 不倒退。
- focused tests、`cargo fmt --check`、`cargo check` 和完整 Rust tests 必须通过。

## Acceptance Criteria

- [x] `BusinessCheckpointV1`、boundary、output reference 和 read/validation 结果由单一 typed owner 定义。
- [x] 序列化结果只包含白名单字段，安全测试证明不含 Prompt、正文、API Key、Authorization、完整 URL。
- [x] canonical idempotency key 稳定且以 `sha256:` 开头；身份字段变化会改变 key。
- [x] batch 章节成功后，现有 `workflow_runtime_state` 保留旧字段并新增
  `business_checkpoint.schema_version = business-checkpoint/v1`。
- [x] checkpoint revision 在 retry/resume/legacy merge 中不倒退。
- [x] checkpoint `input_digest` 等于同一 snapshot 的 R4 generation contract digest。
- [x] legacy runtime state 无 business checkpoint 时继续按旧 resume 行为工作。
- [x] 未知 schema 或非法 payload 不 panic，且不会被当作合法恢复证据。
- [x] digest mismatch、dangling chapter 或空正文会阻止 resume runtime launch并返回 typed error。
- [x] 至少一个 DB-backed create → chapter success → resume 测试通过，证明恢复从业务边界继续。
- [x] 未新增 migration、任务系统、公开 API/SSE breaking change。
- [x] focused tests、完整 Rust tests、fmt/check 与文件编码检查全部通过。
- [x] 优化路线文档更新 R6 实现证据和剩余路线状态。


## Acceptance Evidence（2026-07-16）

- Typed owner：`business_checkpoint_service` 按 schema、canonical、snapshot、recovery
  sibling owner 拆分，唯一 schema 为 `business-checkpoint/v1`，首个 boundary 为
  `chapter_draft_saved`，output reference 仅允许 chapter ID。
- Revision / idempotency：chapter success 使用已完成章节数与既有 revision 的最大值，canonical
  `sha256:` key 覆盖 task、boundary、revision、R4 digest 与 typed output；tamper 和身份变化测试通过。
- Persistence：`BatchGenerationRuntimePersistencePlan::chapter_succeeded` 从现有 snapshot 读取 R4
  contract digest，通过既有 snapshot merge owner additive 写入 `business_checkpoint`；legacy 缺
  contract 时跳过 checkpoint，不影响原成功持久化。
- Recovery：resume prepare 在 reset/dispatch 前校验 schema、payload、idempotency key、R4 digest、
  task/project scope、chapter 存在性和 trim 后非空正文；legacy missing 保持旧恢复路径。
- DB-backed proof：
  `should_prepare_db_backed_resume_after_persisted_business_checkpoint_after_chapter_success` 使用真实 SQLite/SeaORM
  fixture，执行 contract snapshot → chapter content save → `chapter_succeeded.persist()` → 后续
  `failed.persist()` → resume prepare，并证明 checkpoint 在失败 merge 后保留且从 chapter-2 继续。
- Regression：business checkpoint `14/14`、resume command `81/81`、runtime state `137/137`、
  完整 Rust tests `1755/1755` 通过；`cargo fmt --check`、`cargo check` 通过。
- Compatibility / security：未新增 migration、表、task store、API/SSE 字段或 event kind；旧 runtime
  `checkpoint` 保留。12 个 R6 文件通过 UTF-8 无 BOM、LF-only、无 trailing whitespace 检查；
  checkpoint allowlist 与固定 typed error 不保存或回显 Prompt、正文、API Key、Authorization、完整 URL。

## Out of Scope

- 任意 Token/流式片段位置续跑。
- 跨设备分布式锁、全局 exactly-once 执行或消息队列重放。
- R7 批量/长流程完整 orchestration 重构、G1-Cancel 和 G2 门禁实现。
- 新增生产数据库 migration 或把 checkpoint 暴露为新的公开 API。
- 为大纲确认、审校结果生成等所有业务边界一次性接线；R6 先建立标准并验证批量章节边界。

## Open Questions

无阻塞问题。现有路线、R4/R5 合同和用户“确认授权后续直接开发”已经确定首个边界、
兼容策略与实施权限。
