# R7 Controlled Autopilot MVP 最终验收评审

- 日期：2026-07-16
- 路线阶段：`R7 Controlled Autopilot MVP`
- 结论：**PASS（仅限受控 MVP）**
- 后续门禁：**G2 尚未开始，未获 GO**

## 1. 评审范围与非目标

R7 已交付的是一个可审计的、单次的、人工确认的项目工作流受控调用：

```text
Project workflow panel
  -> POST /projects/{project_id}/autopilot/actions
  -> existing generic novel_autopilot task
  -> one-shot Coordinator
  -> static Tool Contract
  -> canonical novel_workflow_service transition
  -> durable invocation audit
  -> owner-scoped readonly history
```

本评审不把这条路径描述成无人值守小说生成能力。它不包含多 Tool 编排、Provider 或
Prompt 运行时调用、自动重试、Pause/Resume/Steer、checkpoint、recovery/replay，或整书/
多卷自治流程。

## 2. 需求—实现—回归矩阵

| R7 验收维度 | 实现 owner 与边界 | 可复现证据 | 结论 |
| --- | --- | --- | --- |
| 唯一 allowlisted Tool | 静态 `autopilot-tool-contract/v1` 仅公开 `transition_project_workflow`；参数使用严格 schema/typed parse。 | `backend-rs/src/services/autopilot_tool_contract_service.rs`；`exposes_only_the_static_transition_tool_with_strict_schema`、参数拒绝测试。 | PASS |
| 人工确认与可信身份/项目作用域 | `confirmed_by_user` 必须为 true；actor 仅来自 Claims；project 仅来自 route；DTO 禁止未知字段。 | `backend-rs/src/api/autopilot.rs:36-110,244-280`；`request_contract_rejects_injected_scope_actor_unknown_tool_and_invalid_phase`。 | PASS |
| 不新建第二执行链 | API 只委托 generic task lifecycle；Coordinator 只调用 Tool Contract；workflow 写入由 canonical service 负责。 | `backend-rs/src/api/autopilot.rs:260-277`；`backend-rs/src/services/autopilot_coordinator_service.rs`；`confirmed_task_uses_task_actor_and_project_scope_for_canonical_transition`。 | PASS |
| 失败不越权、不乐观写入 | project scope mismatch、未确认、非法参数和 stale phase 在 workflow mutation 前拒绝或安全失败。 | `task_project_scope_must_match_tool_arguments_before_workflow_mutation`、`requires_confirmation_without_calling_workflow_service`、`stale_transition_records_a_redacted_failed_audit_without_updating_workflow`。 | PASS |
| 可追溯 durable audit | 创建前落 queued；workflow CAS 与 succeeded projection 同事务；失败/取消仅稳定码。 | `backend-rs/src/services/autopilot_invocation_audit_service.rs`；`queued_audit_redacts_reason_and_provider_prompt_fields`、`cancellation_marks_active_audits_without_overwriting_failed_terminal_state`。 | PASS |
| history 隐私与所有者范围 | 先执行 owner access check，再将内部 audit record 投影为显式 UI allowlist；不返回 actor/project/digest/raw arguments。 | `backend-rs/src/api/autopilot.rs:199-241`；API history 回归；`frontend/e2e/project-workflow-state.spec.ts:312-330`。 | PASS |
| 前端不抢占运行时 owner | 仅在用户打开 Modal 时请求只读 history；确认操作只创建任务，不乐观更新 workflow；无暂停/恢复/重试/重放控件。 | `frontend/src/features/projects/workflow/ProjectWorkflowStatePanel.tsx`；`queues a confirmed background-controlled transition without optimistic workflow mutation`。 | PASS |
| 兼容性与可重复质量门 | 复用 generic task、workflow、audit owner 和既有 task center；不改为第二套状态系统。 | 2026-07-16：fmt/check、R7 Rust tests 25/25、frontend lint/build、workflow Playwright 5/5。 | PASS |
| 禁止能力 | `novel_autopilot` 显式 NonResumable；无 Provider/MCP、控制/恢复端点、自动重试或多步骤自治。 | `backend-rs/src/tasks/recovery.rs` 的 `novel_autopilot_orphan_fails_as_explicit_non_resumable_without_replay`；API route/UI E2E 负向断言。 | PASS |

## 3. 质量门结果（2026-07-16）

```powershell
cargo fmt --manifest-path backend-rs/Cargo.toml -- --check
cargo check --manifest-path backend-rs/Cargo.toml -j 1
$env:RUSTFLAGS='-C link-arg=/DEBUG:NONE'
cargo test --manifest-path backend-rs/Cargo.toml -j 1 autopilot -- --nocapture
npm --prefix frontend run lint
npm --prefix frontend run build
npm --prefix frontend run e2e -- e2e/project-workflow-state.spec.ts
```

结果：全部命令退出码为 0；Rust 聚焦集合 **25/25** 通过，workflow Playwright
**5/5** 通过。保留的 Rust unused-code、frontend Hook dependency 与 Vite circular-chunk
warnings 属于既有告警，未由 R7 改动引入；它们不改变本次受控 MVP 的通过结论。

## 4. 最终结论与下一步

R7 的已授权范围已形成一条单 owner、可审计、可回归的受控闭环，因此标记为
**PASS（受控 MVP）**。此结论不等于 G2 通过，也不构成对无人值守、多步骤或整书/多卷
Autopilot 的授权。

下一阶段必须先建立 **G2 自动驾驶安全门禁**，最小新增证据为：

1. 固定样本（fixed sample）回归：在冻结的 workflow fixture 上验证允许、拒绝与脱敏输出；
2. 故障注入（failure injection）：覆盖 queued audit 写入失败、Tool/CAS 失败、terminal audit
   写入失败与 owner history 读取失败，并验证人工工作台不被破坏；
3. 跨层安全矩阵：确认 route、generic task、Coordinator、Tool、workflow、audit 和 history
   各层仍无第二 owner、无控制/恢复行为；
4. G2 通过前，继续禁止自动重试、Pause/Resume/Steer、checkpoint/replay、Provider/MCP runtime
   与多步骤无人值守生成。

## 5. 后续状态说明（2026-07-16）

本文件保留 R7 收口当时“R7 PASS 不等于 G2 GO”的历史边界。后续 G2 已完成固定样本、
failure-injection 和跨层 owner 审查，并判定为 **GO（仅当前受控单次 MVP 安全门禁）**；详见
`docs/21-g2-autopilot-safety-gate-review.zh-CN.md`。该结论不扩大本 R7 审查中明确排除的无人值守、
多步骤、多 Tool、控制/恢复或 Provider/MCP 授权范围。
