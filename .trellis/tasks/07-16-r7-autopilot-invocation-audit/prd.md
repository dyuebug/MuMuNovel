# R7 Autopilot Durable Invocation Audit

## Goal

为现有 `novel_autopilot` 单次、已确认的受控业务 Tool 增加**独立、持久、可按项目授权读取**的调用审计记录。审计必须保留调用与终态事实，但不得成为工作流、checkpoint、任务恢复或自动重试的 owner。

## Requirements

1. 为每个已创建的 `novel_autopilot` 任务建立一条 durable invocation audit，使用 task ID 关联，不复用带 2 小时终态 TTL 的通用 `TaskRegistry` 持久化文件。
2. 审计记录至少包含：审计 ID、task ID、项目 ID、认证 actor、Tool 名称、Tool schema version、用户确认事实、状态、输入 SHA-256 摘要、安全输入摘要、结果摘要/错误码、创建/开始/完成时间。
3. 直接受控业务 Tool 必须显式记录为 `direct_business_tool`，且 provider/model/prompt 相关字段为 `null`；不得伪造模型、Provider 或 Prompt。
4. 不得存储原始 request payload、reason、prompt、凭据或未脱敏错误详情。输入/结果摘要必须是 allowlist 投影。
5. 启动前写入 queued 审计；Tool 成功时，工作流写入与 succeeded 审计终态必须在同一数据库事务内提交。Tool 失败时必须写入脱敏的 failed 终态。
6. 读取 API 必须使用项目 owner scope；非 owner 不得通过 task ID、project ID 或列表读取到审计数据。
7. 保持 `novel_autopilot` 非可恢复：不新增 Pause/Resume/Steer、checkpoint、自动恢复、重放或无人值守循环。
8. 新表必须通过 PostgreSQL Alembic revision 和 Rust migration-executor catalog 同步演进；SQLite 仅用于 SeaORM 的聚焦测试建表。

## Acceptance Criteria

- [x] 新增的 audit 表和 Rust entity 能承载 v1 调用记录，并有项目/任务查询索引。
- [x] 创建已确认 Autopilot task 时，queued audit 在任务 spawn 前已持久化；审计写入失败不会启动任务。
- [x] `transition_project_workflow` 成功时，workflow 状态与 audit `succeeded` 结果在同一 DB transaction 中提交。
- [x] 合法失败会写入 `failed` 和受控 error code，响应/日志/审计不泄漏 Tool 参数或 reason。
- [x] 项目 owner 可读取自己的 audit 列表；越权读取返回与项目不可见一致的 404。
- [x] Rust migration metadata、Alembic revision、entity 和服务聚焦测试通过；已有 Autopilot API 行为不被破坏。
- [x] 路线文档仍标记 R7 为进行中，且明确 durable audit 不等于 durable invocation recovery。

### 验收证据（2026-07-16）

confirmed task 在 registry spawn 前创建 queued durable audit；Coordinator 与 Tool/workflow CAS 成功
投影复用同一数据库事务，终态错误仅保存稳定脱敏码。实现位于
`backend-rs/src/services/autopilot_invocation_audit_service.rs`、
`backend-rs/src/services/autopilot_coordinator_service.rs` 与
`backend-rs/src/api/background_tasks.rs`；schema contract 使用批准的
`20260716_autopilot_invocation_audit` revision。当前全量 Rust、前端 E2E、lint/build 均通过，
且 audit 仍不提供 invocation recovery/replay 或自动重试。

## Non-goals

- Pause、Resume、Steer、自动重试、任务重放或任意多步骤 Coordinator；
- 修改 `TaskRegistry` 为长期审计库或将 audit 用作 checkpoint；
- Provider/MCP 调用、模型 Prompt 保存、无人值守整书/多卷生成；
- 前端审计中心/任务中心改造（读取 API 为后续只读 UI 纵切预留）。