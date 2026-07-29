# Cooperative Cancellation 与迟到结果防护

## Goal

为已经进入 Running 的长操作建立统一的进程内 cooperative cancellation 合同，使用户取消不再只是投影终态，而能主动停止正在等待的 AI、HTTP、数据库或编排 Future；同时保证取消、正常完成、失败与恢复竞态中只保留一个终态，并保持现有 API、SSE、数据库 Schema 和任务存储兼容。

## Confirmed Facts

- 正式路线要求严格按 `G1-Cancel -> G1 -> R7 -> G2 -> R8` 执行，G1-Cancel 当前验收项为 token 传播、幂等清理、迟到结果拒绝、失败注入和兼容审查。
- 通用后台任务在 `background_tasks.rs` 中由 `tokio::spawn` 驱动；现有取消只将 `TaskRegistry` 记录更新为 `Cancelled`，不会通知正在运行的 `execute_task()`。
- R2 已保证 Pending admission、终态单调性、迟到 registry/progress/SSE 写入安全；G1-Cancel 不重做 R2，也不改变公开任务状态模型。
- 部分通用任务会额外 spawn progress bridge；父 Future 被 drop 时普通 `JoinHandle` drop 不会终止 bridge，必须显式监听同一 token 或使用等价清理合同。
- 批量章节生成由 `dispatch_batch_generation_runtime()` spawn `BatchGenerationRuntimeLifecyclePlan`；取消命令目前只写 task 与 snapshot，不向运行时发 signal。
- `BatchGenerationRuntimePersistencePlan::persist()` 当前使用读取后 ActiveModel update，可能在取消后把 task/snapshot 写回 running/completed/failed；task 与 snapshot 也不在同一条件事务中。
- 项目当前没有 `tokio-util`、`CancellationToken` 或 `AbortHandle` 依赖；G1-Cancel 不需要升级核心依赖。

## Requirements

1. 建立单一、可复用的进程内 cooperative cancellation owner，至少支持：创建 registration、克隆 token、幂等 cancel、等待 cancelled、查询 cancelled、仅删除当前 registration。
2. registration 必须包含唯一实例身份；旧执行的清理不得删除同一 scope/task ID 上恢复或重启后的新 registration。
3. 通用后台任务必须在 Running admission 前完成 registration，并在顶层执行 Future 与 token 之间选择；取消分支不得调用 `fail_task`。
4. 通用 progress bridge 必须监听同一 token，并在取消时退出；现有正常完成、失败和 SSE 终态事件保持不变。
5. 批量生成 dispatch 必须为每次启动/恢复创建新 registration，并在生命周期 Future 与 token 之间选择。
6. 通用取消在 `TaskRegistry` 成功持久化 `Cancelled` 后发送 signal；批量取消在 task 与 snapshot 成功持久化后发送 signal。持久化失败不得错误发送 signal。
7. 批量运行时 task patch 与 snapshot patch 必须在同一数据库事务中完成，并通过数据库条件更新拒绝已进入 completed/failed/cancelled 的迟到写入。
8. 批量取消 task patch 必须使用 active-status 条件更新；取消与正常完成竞态只能有一方取得终态写权限。
9. 重复 cancel、重复 cleanup、缺失 registration 的 signal 均应安全且不 panic；恢复创建的新 registration 不受旧 registration cleanup 影响。
10. 保持向后兼容：不新增 migration、表、公开 API 必填字段、SSE event kind、第二套 task store、项目状态事实或 Coordinator。

## Acceptance Criteria

- [x] 独立 token 测试证明：首次 cancel 唤醒 waiter，重复 cancel 幂等，取消前后调用 `cancelled().await` 均可完成。
- [x] registry 测试证明：同 key 新 registration 替换旧实例；旧实例 cleanup 不删除新实例；当前实例重复 cleanup 安全。
- [x] Running 通用后台执行在 signal 后停止，且取消分支不投影 failed。
- [x] Pending cancel 与 Running admission 竞态保持 R2 单调性；没有 registration 时取消仍成功。
- [x] progress bridge 在 token 取消后及时退出，不产生迟到 progress/SSE 污染。
- [x] 批量 runtime 在受控阻塞 Future 中收到 signal 后退出；resume 使用全新 token。
- [x] 批量 task 已 cancelled/completed/failed 后，迟到 preparing/running/succeeded/failed persistence 被数据库条件拒绝，snapshot 不被覆盖。
- [x] batch cancel 与 normal completion 并发时最终只能是 completed 或 cancelled 之一，不出现终态回退。
- [x] cancel persistence 失败注入证明 signal 不会发送；注册/取消/cleanup 缺失或重复路径安全。
- [x] `cargo fmt --check`、focused tests、runtime/resume tests、完整 Rust tests 与 `cargo check` 通过。
- [x] 路线文档和 backend quality spec 记录 owner、竞态合同、兼容边界、验证证据及剩余风险。

### 验收证据（2026-07-16）

token 传播、replacement cancellation、幂等 cleanup、progress bridge 退出、terminal 条件更新、
cancel persistence 失败注入和 completion/cancel 并发均由本任务 `implement.md` 与对应 Rust tests 覆盖。
路线级最终回归索引见 `validation/route-final-regression-20260716.md`；本次不新增跨进程 cancellation、
checkpoint/replay 或第二套 task owner。

## Out of Scope

- 任意 token 位置的 durable 断点续跑或跨进程 cancellation 传播。
- 新增数据库 migration、取消记录表、第二套任务存储或独立 Coordinator。
- 更改前端取消 API、现有 JSON 字段、HTTP 状态码或 SSE 事件种类。
- 为所有底层 provider 引入新的公开 cancellation 参数；本阶段以 drop 顶层 Future 的 cooperative cancellation 为边界，并对已知 child bridge 显式传播 token。
- G1 Autopilot、R7 Tool/Agent、G2 或 R8 能力。

## Open Questions

无阻塞性产品问题。路线、兼容边界和实现授权均已由现有文档与用户持续授权明确。
