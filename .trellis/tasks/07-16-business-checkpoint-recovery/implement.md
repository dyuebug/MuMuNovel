# Implementation Plan: Business Checkpoint 标准与恢复验证

## Phase A — Typed Business Checkpoint Owner

- [x] 新增 `business_checkpoint_service` 模块与 schema/canonical/snapshot owners。
- [x] 定义 v1 boundary、output reference、checkpoint、read result 与 typed error。
- [x] 实现 deterministic idempotency key、基础校验、runtime state read/merge helper。
- [x] 添加 schema round-trip、allowlist、敏感字段、unknown/invalid/legacy 和 key stability 测试。

Validation:

```powershell
cargo test --manifest-path backend-rs/Cargo.toml business_checkpoint_service
```

Rollback point：仅新增 sibling owner 和 `services/mod.rs` 注册，可独立移除，不影响旧 runtime。

## Phase B — Batch Success Persistence

- [x] 为 `BatchGenerationRuntimePersistencePlan` 增加可选 typed business checkpoint 输入。
- [x] 在 `chapter_succeeded` 成功边界读取已验证 R4 contract digest 与既有 checkpoint revision。
- [x] 生成 `chapter_draft_saved` checkpoint 并通过现有 runtime snapshot merge 持久化。
- [x] legacy 缺 contract 时保持旧持久化；重复写入/retry/resume revision 不倒退。
- [x] 添加 additive merge、legacy、重复写入和 monotonic revision 测试。

Validation:

```powershell
cargo test --manifest-path backend-rs/Cargo.toml chapter_batch_generation_runtime_state_service
```

Rollback point：移除 persistence plan 的可选 checkpoint，旧 `checkpoint` 和 task stage 不变。

## Phase C — Resume Validation

- [x] 扩展 `BatchGenerationPersistedRuntimeContext` 保存 business checkpoint read result。
- [x] 在 resume runtime launch 前校验 R4 digest 一致性。
- [x] 通过数据库校验 chapter output reference 存在、范围正确且正文非空。
- [x] legacy missing 继续旧恢复；unsupported/invalid/mismatch/dangling/empty 返回 typed error。
- [x] 保留合法 checkpoint 和其他 runtime state 到 resume seed。

Validation:

```powershell
cargo test --manifest-path backend-rs/Cargo.toml chapter_batch_generation_resume_task_command_service
cargo test --manifest-path backend-rs/Cargo.toml chapter_batch_generation_runtime_state_service
```

Rollback point：关闭 resume checkpoint validation 后仍可回到 legacy resume；不需要数据迁移。

## Phase D — DB-backed Recovery Proof

- [x] 添加真实 SQLite/SeaORM fixture：project、chapters、batch task、snapshot 与 R4 contract。
- [x] 执行章节成功 persistence，读取并断言 business checkpoint。
- [x] 执行 resume prepare/launch，证明有效 output 可恢复且后续章节上下文保持。
- [x] 覆盖 digest mismatch、output missing、empty content 与 legacy snapshot。

Validation:

```powershell
cargo test --manifest-path backend-rs/Cargo.toml business_checkpoint
cargo test --manifest-path backend-rs/Cargo.toml batch_generation_resume
```

## Phase E — Compatibility and Security Review

- [x] 审查 `git diff`，确认只做 additive JSON key 和 typed error，不改公开 API/SSE。
- [x] 搜索 migration、第二 checkpoint 表/任务 owner，确认未新增。
- [x] 对 checkpoint 序列化执行敏感字段 allowlist 断言。
- [x] 检查 UTF-8 无 BOM、LF-only、trailing whitespace。
- [x] 更新 PRD acceptance evidence 与路线文档 R6 状态。
- [x] 将 typed business checkpoint 的可执行合同固化到 `.trellis/spec/backend/quality-guidelines.md`。

## Phase F — Full Quality Gate

- [x] 设置低调试信息 Rust 环境，降低 Windows linker 压力。
- [x] `cargo fmt --manifest-path backend-rs/Cargo.toml -- --check`
- [x] `cargo check --manifest-path backend-rs/Cargo.toml`
- [x] focused tests 全通过。
- [x] `cargo test --manifest-path backend-rs/Cargo.toml --quiet`
- [x] 运行 `trellis-check` 并完成可维护性、测试、性能、安全、兼容性自评。

Rust environment:

```powershell
$env:RUSTFLAGS = "-C debuginfo=0 -C link-arg=/DEBUG:NONE"
$env:CARGO_INCREMENTAL = "0"
```


## Completion Evidence（2026-07-16）

```text
cargo test ... business_checkpoint                                  14 passed
cargo test ... chapter_batch_generation_resume_task_command_service 81 passed
cargo test ... chapter_batch_generation_runtime_state_service       137 passed
cargo fmt --manifest-path backend-rs/Cargo.toml -- --check          passed
cargo check --manifest-path backend-rs/Cargo.toml                    passed
cargo test --manifest-path backend-rs/Cargo.toml --quiet             1755 passed
```

真实恢复证明由
`should_prepare_db_backed_resume_after_persisted_business_checkpoint_after_chapter_success` 提供：测试不再直接伪造
合法 checkpoint，而是通过生产 `chapter_succeeded.persist()` 写入，再持久化后续失败并执行 resume。
安全/兼容性审查确认只增加现有 runtime snapshot 的 additive typed JSON key 和内部 typed error；
未新增 migration、公开 API/SSE 变更或第二套任务/checkpoint 存储。

## Risky Shared Files

```text
backend-rs/src/services/mod.rs
backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs
backend-rs/src/services/chapter_batch_generation_runtime_state_service/runtime_persistence_owner.rs
backend-rs/src/services/chapter_batch_generation_runtime_state_service/resume_restore_owner.rs
backend-rs/src/services/chapter_batch_generation_runtime_state_service/runtime_driver_owner.rs
backend-rs/src/services/chapter_batch_generation_resume_task_command_service.rs
docs/15-ainovel-cli-comparison-and-mumunovel-optimization.zh-CN.md
```

修改前读取当前 diff；只做窄 patch，禁止整文件覆盖、全仓无关格式化、git reset、commit、push 或 archive。
