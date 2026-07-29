# Implementation Plan: G2 Autopilot Safety Gate

## Ordered Checklist

1. 读取 R7 收口评审、G2 路线条款和 autopilot audit/backend/frontend 规范，确认禁止能力清单。
2. 在 `backend-rs/src/services/` 新增 `cfg(test)` shared safety fixture，并在 `services/mod.rs`
   仅以 test-only module 方式注册；不改变运行时模块图。
3. 使用 fixture 扩展现有 action API、Coordinator/audit 或 generic executor tests，覆盖确定性的
   confirmed、scope-reject 和 stale-CAS 样本。
4. 新增 queued audit 写入失败与 owner history 读取失败的 SQLite 缺表回归，证明安全 error 和
   workflow/task 不变。
5. 新增 succeeded terminal audit failure 的 SQLite trigger 回归。仅在该回归揭示 audit 终态无法
   保持可追溯时，在 Coordinator 增加 rollback 后的 best-effort stable failed projection。
6. 断言 generic task lifecycle 的 terminal owner 不被 audit failure 替换，且 workflow CAS rollback
   后状态不变；检查错误/历史输出不含 raw input 或内部持久化 detail。
7. 复跑现有 workflow Playwright，继续验证 readonly history、无控制/恢复 UI 与无 optimistic
   workflow mutation。
8. 执行质量门，更新 G2 task evidence、路线文档和最终 GO/NO-GO 判定。G2 若所有验收通过才可
   标记 GO；仍不得实现无人值守能力。

## Validation Commands

```powershell
cargo fmt --manifest-path backend-rs/Cargo.toml -- --check
cargo check --manifest-path backend-rs/Cargo.toml -j 1
$env:RUSTFLAGS='-C link-arg=/DEBUG:NONE'
cargo test --manifest-path backend-rs/Cargo.toml -j 1 autopilot -- --nocapture
npm --prefix frontend run lint
npm --prefix frontend run build
npm --prefix frontend run e2e -- e2e/project-workflow-state.spec.ts
```

可在实现期间先运行相应 module tests，但最终 `autopilot` 聚焦集合与 frontend workflow E2E
不可省略。

## Review Gates

- fixture 是测试输入 owner，不是第二套 Tool/workflow schema；它必须仅表达既有公开 contract。
- 注入故障不能通过 production environment flag、公开 endpoint、runtime retry 或 provider mock 实现。
- audit fallback 不得附带 raw error、arguments、reason、Prompt、credential、provider/model/digest。
- SQLite trigger 场景必须断言 workflow rollback 和 generic terminal owner；不能把 audit 的失败
  投影伪装成 workflow 成功。
- G2 文档必须明确 GO 不等于无人值守 Autopilot 授权，且继续禁止所有路线明确延期能力。
- 所有新增文本为 UTF-8 无 BOM、LF、无尾随空白。

## Risky Files and Rollback Points

- `backend-rs/src/services/autopilot_coordinator_service.rs`：不得让 audit fallback 接管 task terminal；
  只允许 rollback 后 best-effort 稳定 failed code。
- `backend-rs/src/services/autopilot_invocation_audit_service.rs`：不得扩大 read model、保存敏感输入，
  或将测试 hook 放入生产路径。
- `backend-rs/src/api/autopilot.rs` / `api/background_tasks.rs`：不得增加控制或恢复 route；测试必须
  复用当前 handler/lifecycle。
- `frontend/e2e/project-workflow-state.spec.ts`：仅增强 existing readonly/negative assertions，
  不改 UI 行为。
- 回滚仅撤销 test-only fixture、G2 tests 与 Coordinator fallback；绝不回滚 R7 owner 边界。

## 实施结果 / G2 Gate Result（2026-07-16）

- 新增仅 `cfg(test)` 可见的 `autopilot_safety_gate_fixture`，以固定 project/owner/task、
  allowlisted Tool 与 `foundation -> world_building` phase 变迁表达样本；不含 Prompt、凭据、
  Provider/model、digest、raw arguments 或原始错误。
- action API 的 SQLite 缺表回归证明 queued audit 写入失败时不创建/执行 task，owner history
  读取失败时不返回 audit 且不改变 workflow/task。
- Coordinator 的 SQLite trigger 回归证明 succeeded audit projection 失败时，workflow transaction
  回滚并 best-effort 写入安全 `failed/tool_execution_failed` audit；fallback 不接管 task terminal。
- generic runner 回归证明 terminal audit failure 后 `TaskStatus::Failed` 仍由 generic task lifecycle
  唯一持有，workflow 保持 fixture 初始 phase，错误对外仅为稳定字符串。
- 完成 `cargo fmt --check`、`cargo check -j 1`、Autopilot Rust tests **29/29**、frontend lint/build 和
  workflow Playwright **5/5**；既有 warnings 已记录但不阻断本门禁。

**G2 = GO（仅当前受控单次 MVP 安全门禁）**。本结论不授权无人值守、多步骤、多 Tool、整书/多卷
Autopilot，也不授权 Pause/Resume/Steer、checkpoint/recovery/replay、自动重试、Provider/MCP runtime
或 production migration。完整评审见 `docs/21-g2-autopilot-safety-gate-review.zh-CN.md`。
