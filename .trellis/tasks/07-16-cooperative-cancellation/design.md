# Design: Cooperative Cancellation 与迟到结果防护

## 1. Architecture and Boundaries

新增内部 service owner：

```text
services/cooperative_cancellation_service.rs
```

该 owner 只保存进程内“当前执行实例 -> cancellation token”映射，不保存业务状态，不替代 `TaskRegistry`、`batch_generation_tasks` 或 workflow state。业务终态仍由现有 owner 持久化；token 仅在终态提交成功后作为执行控制信号发送。

执行域使用明确 scope 隔离：

```text
background_task:<task_id>
batch_generation:<batch_id>
```

## 2. Token Contract

`CooperativeCancellationToken` 基于 `Arc<Inner>`：

- `AtomicBool` 保存单调 cancelled 状态；
- `tokio::sync::Notify` 唤醒等待者；
- `cancel()` 通过 compare/swap 或 swap 保证只有第一次发送通知；
- `cancelled().await` 在创建 notified future 前后检查状态，避免 lost wake；
- token clone 指向同一单调状态。

不新增 `tokio-util` 依赖，复用当前 Tokio full feature。

## 3. Registration Registry and Cleanup

`CooperativeCancellationRegistry` 使用：

- `Arc<std::sync::RwLock<HashMap<ExecutionKey, ActiveRegistration>>>`；
- `AtomicU64` 生成 registration ID；
- `register(scope, task_id)` 总是安装新实例并返回 registration；
- `cancel(scope, task_id)` 只 signal 当前实例，缺失时返回 false；
- `remove_if_current(key, registration_id)` 只删除身份匹配的实例。

`CooperativeCancellationRegistration` 提供显式幂等 `cleanup()`，并在 `Drop` 中执行同一同步清理。同步锁不跨 `.await`，因此 Drop 可保证 panic/early return 后不遗留当前 registration。旧 registration 被新实例替换后，其 Drop 不会误删新实例。

生产代码通过 `OnceLock` 暴露单一进程级 registry；测试可创建独立 registry，避免依赖全局计数。

## 4. Generic Background Task Data Flow

```text
spawn_task_execution
  -> register(background_task, task_id)
  -> mark_task_running (existing CAS-like update_if)
      -> false: cleanup and return
  -> tokio::select! (biased)
      -> token.cancelled(): return without fail_task
      -> execute_task(): preserve existing complete/fail behavior
  -> registration cleanup
```

取消 route 顺序：

```text
cancel_active_task() commits TaskRegistry Cancelled
  -> registry.cancel(background_task, task_id)
  -> existing terminal SSE fanout
```

如果 Pending cancel 发生在 registration 前，Running admission 会失败；如果发生在 registration 后，token 会被 signal。两条路径都不会执行或回退终态。

现有 `spawn_channel_progress_bridge` 接收 token，并用 `tokio::select!` 在 sleep tick 与 token 之间选择。这样父执行 Future 被 drop 后，bridge 仍能通过 token 自行退出；channel drain 在 sender drop 后自然退出。

## 5. Batch Runtime Data Flow

```text
dispatch_batch_generation_runtime
  -> register(batch_generation, batch_id)
  -> tokio::spawn
      -> tokio::select! (biased)
          -> token.cancelled(): stop lifecycle future
          -> BatchGenerationRuntimeLifecyclePlan::start(): normal path
      -> registration cleanup
```

startup 与 resume 已复用同一 dispatch owner，因此每次启动/恢复自然创建新的 registration。registration ID 防止旧执行 cleanup 删除 resume 实例。

取消 command 顺序：

```text
load/prepare cancel plan
  -> transactional conditional task + snapshot persistence
  -> success: registry.cancel(batch_generation, batch_id)
  -> error: no signal
```

## 6. Database Race Protection

### 6.1 Runtime persistence

`BatchGenerationRuntimePersistencePlan::persist()` 改为局部事务：

1. 在事务中加载当前 snapshot 并构建 merged checkpoint；
2. 读取 task 以计算 failed chapters 等现有 patch；
3. 使用 `Entity::update_many().set(active_patch)`，过滤：
   - `id == task_id`
   - `status NOT IN ('completed', 'failed', 'cancelled')`
4. `rows_affected == 0` 时返回 typed/internal cancellation rejection，回滚并且不写 snapshot；
5. 使用同一 transaction upsert snapshot；
6. commit。

task 条件 update 会取得行写锁。cancel 与 runtime 并发时，后写者在事务中重新判断 active status；task 与 snapshot 的写入顺序受同一事务保护，不会出现 cancelled task 配 running/completed snapshot 的迟到覆盖。

### 6.2 Cancel persistence

`BatchGenerationCancelledPersistencePlan::persist()` 同样使用事务和 active-status 条件更新：

- 只允许 `pending`/`running` 进入 `cancelled`；
- 若 normal completion/failed 已先提交，则 rows affected 为 0，取消返回既有 domain error，不发送 token；
- task 与 cancelled snapshot 同事务提交；
- commit 成功后才由 command owner发送 signal。

### 6.3 Snapshot helper

现有 snapshot load/persist/upsert helper 的 `&DatabaseConnection` 参数最小泛化为 `&impl ConnectionTrait`（或等价泛型），使其可同时接受 `DatabaseConnection` 与 `DatabaseTransaction`。函数可见性、业务语义和调用方行为不变。

## 7. Failure and Concurrency Tests

- token lost-wake、重复 cancel、cancel-before-wait；
- registration replacement、old cleanup、新实例保留；
- generic Running select cancellation、Pending admission race、cancelled 不 fail；
- bridge token exit；
- batch dispatch/lifecycle 可控阻塞 cancellation；
- DB-backed terminal rejection；
- barrier 驱动 cancel vs complete 竞态；
- mock/fixture 驱动 persistence error 后无 signal；
- resume registration replacement。

优先在 owner 单元测试和现有 SQLite DB-backed helper中完成，不引入生产 migration 或外部服务。

## 8. Compatibility and Rollback

兼容边界：

- HTTP route、JSON payload、状态字符串、SSE event kind 不变；
- 无 migration、表或配置变更；
- 无前端改动；
- cancellation registry 是非 durable 执行控制，进程重启后由既有恢复策略处理。

回滚可按三层独立撤回：

1. 移除 generic/batch select 接入，保留 service/tests；
2. 恢复 persistence 的非事务更新（仅在回归证据要求时）；
3. 删除内部 cancellation service 模块。

不执行 git reset/checkout；所有回滚均使用 scoped patch。
