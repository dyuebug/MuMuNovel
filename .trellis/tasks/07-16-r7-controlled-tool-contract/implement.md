# Implementation Plan: R7 Controlled Tool Contract First Slice

## Preconditions

- [x] G1 Contract Gate 于 2026-07-16 结论为 GO。
- [x] R7 第一纵切已完成代码调研与边界确认。
- [x] 用户已授权后续直接开发。
- [x] 复查当前共享工作区，避免覆盖并行任务对 `services/mod.rs` 的变更。

## Phase A — Contract Schema Owner

- [x] 新增 `autopilot_tool_contract_service` module 和 `schema_owner.rs`。
- [x] 定义 schema version、stable Tool 名称、side-effect/confirmation metadata 和静态 registry。
- [x] 定义 strict `TransitionProjectWorkflowArgs`、Tool result DTO 和安全 error enum。
- [x] 实现 `ToolDef` JSON Schema 投影：object、required、公共 phase enum、
  `additionalProperties=false`。
- [x] 为 unknown field、non-object、unknown phase、empty project id 和 `user_id` injection
  添加 unit tests。

## Phase B — Controlled Dispatcher

- [x] 新增 `dispatch_owner.rs`，定义内部 authenticated execution context 和确认状态。
- [x] 按 allowlist → typed validation → confirmation → domain adapter 固定顺序执行。
- [x] 仅通过 `novel_workflow_service::transition` 实施写入；将 existing audit context 原样交给
  domain owner。
- [x] 将 `NovelWorkflowError` 映射为稳定、无内部 detail 的 Tool contract errors。
- [x] 记录不含 raw arguments 的 structured tracing outcome。

## Phase C — Tests

- [x] static registry / ToolDef schema snapshot assertions。
- [x] unknown Tool、invalid JSON/non-object、unknown field、missing required、unknown phase、empty
  project id 和 `user_id` injection 拒绝测试。
- [x] missing confirmation 测试，断言没有产生 workflow state change。
- [x] SQLite-backed happy path，断言 receipt/state 与 `novel_workflow_service` 一致。
- [x] SQLite-backed unauthorized project 与 stale expected phase 测试。
- [x] 断言 result / error debug 或序列化文本不含 raw arguments、token、prompt、URL 等敏感字段。

## Phase D — Validation

1. `cargo fmt --manifest-path backend-rs/Cargo.toml -- --check`
2. `cargo check --manifest-path backend-rs/Cargo.toml`
3. `cargo test --manifest-path backend-rs/Cargo.toml autopilot_tool_contract_service -- --nocapture`
4. `cargo test --manifest-path backend-rs/Cargo.toml novel_workflow_service -- --nocapture`
5. `cargo test --manifest-path backend-rs/Cargo.toml --quiet`
6. UTF-8 无 BOM、LF-only、trailing whitespace 检查（只覆盖 R7 修改文件）。

若 Windows 默认 MSVC linker 出现 `LNK1318: PDB LIMIT (12)`，仅测试进程可使用：

```powershell
$lld="$env:USERPROFILE/.rustup/toolchains/stable-x86_64-pc-windows-msvc/lib/rustlib/x86_64-pc-windows-msvc/bin/rust-lld.exe"
$env:RUSTFLAGS="-C linker=$lld -C debuginfo=0 -C link-arg=/DEBUG:NONE"
cargo test -j 1
```

这是验证环境规避，不是产品代码配置变更。

## Risky Shared Files

```text
backend-rs/src/services/mod.rs
backend-rs/src/services/novel_workflow_service.rs
```

原则：优先新增 sibling owner；R7 adapter 不修改 workflow transition 的业务规则。
若 workflow service 现有公开类型不足，以最小 `pub` 暴露为限，并在同轮测试其向后兼容性。

## Rollback Boundary

- 删除新增的 `autopilot_tool_contract_service` 和 `services/mod.rs` 中唯一 registration 即可回滚。
- 不修改 migration、HTTP route、task registry 或 existing workflow persistence，故无数据回滚。
- 发现 Tool adapter 试图复制 workflow 或直接写数据库时，停止实施并回到本设计修正。

## Completion Evidence

- [x] PRD Acceptance Criteria 全部勾选并附对应测试命令/结果。
- [x] quality check 记录 focused/full Rust gate 与文件编码检查。
- [x] 路线文档标记 R7 Tool Contract first slice 完成，但不得把 R7 Coordinator、task、UI、audit
  后续工作误标为完成。
- [x] 不 archive、不 commit、不 push，保持当前 Trellis task 目录供后续路线继续使用。

## 实施与质量记录（2026-07-16）

- 实现文件：`backend-rs/src/services/autopilot_tool_contract_service.rs`、
  `schema_owner.rs`、`dispatch_owner.rs`、`tests.rs`；仅在 `services/mod.rs` 注册新模块。
- 合同顺序保持为 static allowlist → typed arguments → confirmation → canonical workflow service；无 route handler、直接 SQL 或重复 workflow state。
- 质量门禁：fmt/check 通过；受控 Tool 5/5、workflow owner 17/17、完整 Rust 1766/1766 测试通过。
- Windows 本地默认 linker 的 `LNK1318` 仅通过测试进程环境变量规避；未落盘或改变产品构建配置。
- 文件编码检查通过；不 archive、不 commit、不 push。任务保持 `in_progress`，用于承接 R7 后续纵切。
