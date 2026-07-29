# Implement: Cooperative Cancellation 与迟到结果防护

## Ordered Checklist

1. [x] 新增 `cooperative_cancellation_service`：token、scope/key、registry、registration ID、幂等 cleanup、全局 owner 和单元测试。
2. [x] 在 `services/mod.rs` 注册内部模块；确认无 Cargo 依赖变化。
3. [x] 改造 generic `spawn_task_execution`：Running admission 前注册，顶层 biased `select!`，取消不调用 `fail_task`，所有退出路径 cleanup。
4. [x] 将 token 传给所有 progress bridge 分支；bridge 在 token signal 后退出，保留原有 progress/terminal SSE 合同。
5. [x] 在 generic cancel owner 成功投影 `Cancelled` 后 signal 当前 registration；补 Pending/Running/重复 cancel/bridge 竞态测试。
6. [x] 改造 batch dispatch：每次 startup/resume 注册新实例，生命周期 Future 与 token select，退出 cleanup。
7. [x] 泛化 chapter/batch snapshot helper 的 connection 参数，使其支持 `DatabaseTransaction`。
8. [x] 将 runtime persistence 改为 task 条件更新 + snapshot 同事务；terminal rows affected 0 时拒绝迟到 patch。
9. [x] 将 cancel persistence 改为 active-status 条件更新 + cancelled snapshot 同事务；commit 成功后 signal，失败不 signal。
10. [x] 添加 DB-backed 取消/完成竞态、迟到 success/failure 拒绝、旧 cleanup 不影响 resume、新 registration 测试。
11. [x] 运行 focused tests并修复：cancellation service、background tasks、runtime state、resume command。
12. [x] 运行 `cargo fmt --check`、`cargo check` 和完整 Rust tests；确认无 frontend/API/schema/migration 差异。
13. [x] 使用 `trellis-check` 自检，更新 backend quality spec 与路线文档的 G1-Cancel 状态、证据、风险和下一步 G1。

## Validation Commands

```powershell
Set-Location "backend-rs"
cargo test cooperative_cancellation_service
cargo test api::background_tasks
cargo test chapter_batch_generation_runtime_state_service
cargo test chapter_batch_generation_resume_task_command_service
cargo fmt --check
cargo check
cargo test --locked
```

如仓库已有更精确 test filter，以实际模块名补充，不删除以上门禁。

## Risky Files and Rollback Points

- `backend-rs/src/api/background_tasks.rs`
  - 风险：取消与 complete/fail 的 select 竞态、child bridge 泄漏。
  - 回滚点：只撤回 token 参数与 select wrapper，不修改 R2 状态机函数。
- `runtime_persistence_owner.rs`
  - 风险：ActiveModel 条件 update 字段遗漏、事务错误映射改变测试文本。
  - 回滚点：保留原 stage builder，限制改动在 persist owner。
- `startup_and_command_projection_owner.rs`
  - 风险：cancel 与 completion 并发、snapshot 写失败。
  - 回滚点：保持现有 response projection，限制改动在 persist/command 顺序。
- `snapshot_persistence_owner.rs` / `runtime_snapshot_owner.rs`
  - 风险：泛型连接签名编译影响面。
  - 回滚点：仅调整内部参数类型，不改变写入模式或字段。

## Review Gates Before Start

- [x] 路线明确 G1-Cancel 是 R6 后唯一主线。
- [x] 用户已确认后续直接开发，无需重复请求普通实现授权。
- [x] 复杂任务具备 `prd.md`、`design.md`、`implement.md`。
- [x] 无生产 migration、核心依赖升级、删除或 git 操作。
- [x] 实现范围可通过 scoped patch 与现有 Rust 测试验证。


## Implementation Result

- Added the single in-process cooperative cancellation owner with scoped keys, unique registration IDs,
  replacement-safe cleanup, idempotent cancellation, `Drop` cleanup, and registry unit tests.
- Propagated the same token through generic background execution, every progress bridge, and batch startup/resume;
  lifecycle owners use biased `tokio::select!`, and cancellation exits never project `Failed`.
- Moved runtime task updates and snapshot updates into one transaction. Runtime persistence rejects
  `completed` / `failed` / `cancelled`, while cancel persistence accepts only `pending` / `running`.
- Treated `rows_affected == 0` as a rejected terminal-ownership attempt. Cancellation is signalled only after
  the cancelled task/snapshot transaction commits; injected persistence failure leaves the token unsignalled.
- Added DB-backed late-result rejection and a barrier-driven cancel-vs-completion race proving that exactly one
  terminal owner wins and that task/snapshot terminal states remain aligned.
- Corrected two test scenarios without weakening production CAS: the resume checkpoint fixture now follows
  `running -> chapter success checkpoint -> failed -> resume`, and asynchronous create-workflow assertions no
  longer assume the database remains at its initial `pending/queued` phase after runtime dispatch.
- Preserved public HTTP/JSON/SSE contracts, status strings, database schema, migrations, Cargo dependencies,
  task-store ownership, workflow facts, and recovery protocol boundaries.
- Closed the G1 review finding for replacement registration ownership: `register()` now captures the replaced
  registration under the write lock, releases the lock, then cancels the previous token. The existing replacement
  test now proves old-token cancellation, replacement-token isolation, old-cleanup safety, and idempotent cleanup.

## Validation Evidence

- `cargo fmt --check`: passed.
- `cargo check`: passed; only pre-existing unused/dead-code warnings remain.
- `cargo check --tests`: passed.
- Focused cancellation, background bridge, runtime persistence, resume checkpoint, create workflow, failure
  injection, and DB-backed cancel tests passed.
- The direct cancel-vs-completion race passed 10/10 repeated executions.
- Replacement regression test: `cooperative_cancellation_service` **3 passed**; background task suite
  **26 passed**; batch runtime state suite **139 passed**.
- Full Rust test binary result: **1761 passed, 0 failed, 0 ignored**.
- The default MSVC linker can fail locally with `LNK1318` (PDB `LIMIT (12)`). Verification succeeded by linking
  with `rust-lld`, `-C debuginfo=0`, and `-C link-arg=/DEBUG:NONE`; this is an environment workaround, not a
  product-code change.
- Task remains `in_progress` for Trellis bookkeeping; it is intentionally not archived and no Git operation was
  performed. The next roadmap activity is the G1 review.
