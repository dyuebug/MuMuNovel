# Implementation Plan：后台任务快照原子化

## Steps

1. 读取 backend/Trellis 规范和 `tasks/persistence.rs`、registry、startup 调用链。
2. 在 persistence 模块内引入路径结构、候选类型、结构化错误和模块级保存 mutex。
3. 提取 `load_from_dir()`：按 primary → backup → temp 加载，校验 version，隔离损坏候选。
4. 提取 `save_to_dir()`：序列化、同目录 temp 写入、flush/sync、旧主快照旋转、提交和失败 rollback。
5. 保持三个生产公开函数签名不变，并补充结构化日志。
6. 在同模块增加临时目录测试，覆盖正常、损坏、缺失、失败和并发路径。
7. 运行 targeted tests、完整 Rust 门禁和 `git diff --check`。
8. 将快照协议写入 backend quality spec，并更新路线文档 R1 状态。

## Validation

```powershell
cargo fmt --manifest-path "backend-rs/Cargo.toml" -- --check
cargo test --locked --manifest-path "backend-rs/Cargo.toml" tasks::persistence::tests
cargo check --locked --manifest-path "backend-rs/Cargo.toml"
cargo test --locked --manifest-path "backend-rs/Cargo.toml"
cargo clippy --locked --manifest-path "backend-rs/Cargo.toml" --all-targets -- -D clippy::correctness -D clippy::suspicious
E:/Code/SoftWare/Tools/Git/cmd/git.exe diff --check
```

## 风险文件

- `backend-rs/src/tasks/persistence.rs`
- `.trellis/spec/backend/quality-guidelines.md`
- `docs/15-ainovel-cli-comparison-and-mumunovel-optimization.zh-CN.md`

## 回滚点

- 生产公开 API 和 snapshot version 保持不变，可单文件回滚。
- 不新增依赖，不修改 Cargo.lock。
- 如果目录 sync 在特定平台不可用，只降级为 best-effort，不回退到直接覆盖主文件。
- 不通过吞掉错误、关闭 sync 或删除 fallback 来换取测试通过。

## 启动前检查

- [x] 唯一持久化 owner 已定位。
- [x] 直接覆盖风险已由现有实现证实。
- [x] Windows rename 覆盖差异已纳入设计。
- [x] 测试可以使用标准库临时目录，不需要新增依赖。
- [x] R1 与 R2 的恢复策略边界已明确。

## 实施记录（2026-07-12）

### 实际代码结构

- 在 `backend-rs/src/tasks/persistence.rs` 内保持三个公开生产入口不变。
- 新增 `SnapshotPaths`、`SnapshotSource`、`LoadOutcome` 和结构化
  `SnapshotPersistenceError`，将路径协议和错误边界集中在唯一 owner。
- `save_to_dir()` 使用模块级 Tokio mutex，执行序列化、固定同目录 temp 写入、
  `write_all`、`flush`、`sync_all`、primary 校验/轮换、temp 提交和失败 rollback。
- `load_from_dir()` 按 primary → backup → temp 加载；无效 JSON 和不支持的 version
  会被移动到唯一 `.corrupt-*` 文件后继续 fallback。
- Unix 在提交后 best-effort 同步父目录；非 Unix 保持兼容 no-op，不回退到直接覆盖。
- 未新增依赖，`TaskRecord` schema、snapshot version 1 和生产文件名保持不变。

### 新增测试

1. `first_save_commits_parseable_primary_snapshot`
2. `second_save_keeps_previous_primary_as_backup`
3. `corrupted_primary_is_quarantined_and_backup_is_loaded`
4. `missing_primary_falls_back_to_backup`
5. `complete_temporary_snapshot_is_last_resort_fallback`
6. `unsupported_version_is_quarantined_before_backup_fallback`
7. `temporary_open_failure_preserves_existing_primary`
8. `concurrent_saves_leave_primary_and_backup_parseable`
9. `production_snapshot_file_name_remains_backward_compatible`

### 验证结果

- Targeted persistence tests：9/9 通过。
- `cargo fmt --check`：通过。
- `cargo check --locked`：通过。
- `cargo test --locked`：1533/1533 通过。
- Clippy `-D clippy::correctness -D clippy::suspicious`：通过。
- Clippy 仍报告约 206 个非阻断历史 warning，均不属于本任务新增阻断项。

### 实现偏差与剩余风险

- 设计中的临时候选采用固定 `background_tasks.json.tmp`，由进程内 mutex 保证单 owner
  串行保存；没有采用每次写入不同 UUID 的 temp 文件。
- 父目录元数据持久性在 Unix 为 best-effort，在 Windows 不额外打开目录句柄；不同文件
  系统/断电模型的保证仍可能不同。
- 本任务只保证文件快照可恢复，不实现按 `task_type` 分类的恢复决策；该能力明确留给 R2。
- R0 仍等待 GitHub runner 的 PostgreSQL migration、Rust server 与 Playwright smoke 证据，
  因此 G0 尚未满足。
