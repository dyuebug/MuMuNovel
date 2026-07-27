# Implement: 章节自动返修质量重试收敛

## Implementation Checklist

- [x] 读取 backend spec 索引及数据库、质量、测试规范，确认目标文件现有未提交状态。
- [x] 为 Autopilot 返修失败候选定义最小作用域与证据构建/读取 helper，复用 `chapter_draft_attempt` 和完整候选提取逻辑。
- [x] 扩展 `chapter_repair_generation_service`：加载同 run/epoch/source digest/analysis 的最新失败候选，合并质量反馈并作为后续返修基线。
- [x] 扩展 `chapter_repair_repository`：把失败候选插入、run/step 状态更新和 `result_digest` 写入同一事务。
- [x] 调整 `chapter_repair_adapter`：质量失败传递候选证据；预算耗尽时保存最后候选并进入一次人工复核；任务结果返回安全质量摘要。
- [x] 补充单元/仓储测试：作用域隔离、损坏候选回退、失败证据原子写入、下一次返修消费最近候选、预算耗尽停止重试。
- [x] 回归正常通过、首次返修通过、provider failure、cancel、stale fence 和 business-data-changed 路径。
- [x] 执行定向 `cargo fmt --check`、相关 `cargo test`、`cargo check`；按 Trellis check 结果修正问题。
- [x] 使用 `trellis-break-loop` 复盘为何候选质量证据会在耐久边界丢失，并更新 backend spec。

## Expected Files

- `backend-rs/src/services/chapter_repair_generation_service.rs`
- `backend-rs/src/services/novel_autopilot/chapter_repair_adapter.rs`
- `backend-rs/src/services/novel_autopilot/chapter_repair_repository.rs`
- `backend-rs/src/services/novel_autopilot/*tests.rs`（按现有测试归属选择）
- `.trellis/tasks/07-26-chapter-auto-repair-quality-retry/*`

## Validation Commands

```powershell
cargo fmt --manifest-path "backend-rs/Cargo.toml" -- --check
cargo test --manifest-path "backend-rs/Cargo.toml" chapter_repair
cargo test --manifest-path "backend-rs/Cargo.toml" novel_autopilot
cargo check --manifest-path "backend-rs/Cargo.toml"
```

若仓库已有更窄的 nextest/脚本门禁，以 backend spec 指定命令为准。

## Risky Boundaries

- `chapter_repair_repository` 的事务 fence 不得弱化，新增 insert 必须与现有 run/step 更新共用事务。
- 最近失败候选不得跨 run、epoch、源正文或分析记录复用。
- 失败候选可包含完整章节正文，但不得包含 prompt 或供应商敏感信息。
- 当前 `backend-rs/src/services/novel_autopilot/` 为用户未跟踪目录；修改前后必须用 diff/no-index 或内容核对保护现有工作。

## Rollback Points

- 若候选读取导致生成行为回归，可保留失败证据持久化，关闭“最近候选作为返修基线”的消费逻辑。
- 若事务扩展无法兼容现有数据库，回滚到只写 step digest 的最小诊断修复，但不得把失败候选写入正式章节。
- 不涉及 schema 迁移，无数据库结构回滚。

## Review Gate Before Start

- PRD、设计和实施计划已完成。
- 用户确认该计划后才执行 `task.py start`，进入 Phase 2。

## Completion Evidence

- `cargo test ... chapter_repair -- --nocapture`（`rust-lld`）：15 passed，0 failed。
- `cargo test ... novel_autopilot -- --nocapture`（`rust-lld`）：150 passed，0 failed。
- `cargo check --manifest-path "backend-rs/Cargo.toml"`：通过；保留仓库既有 unused/dead-code warnings。
- `cargo check --tests --manifest-path "backend-rs/Cargo.toml"`：通过；保留仓库既有 unused/dead-code warnings。
- `cargo fmt --manifest-path "backend-rs/Cargo.toml" -- --check`：通过。
- `git diff --check`：返回成功；仅提示两个本任务范围外文件的既有末尾空行及 frontend CRLF 转换告警。
- 本任务涉及的 Rust/Trellis 文件均为 UTF-8 无 BOM，且无尾随空白。
- `trellis-check`：规范、数据流、事务、测试与安全字段审查通过，无需追加 Rust 修正。
- `trellis-break-loop`：复盘见 `retrospective.md`；可执行合同已写入 backend Durable Novel Autopilot spec。
- 仓库不存在 `src/templates/markdown/spec/`，因此没有可同步的规范模板。
- 未执行 `git commit`、`git push`、分支切换或 reset。
