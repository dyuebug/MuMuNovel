# G2 Autopilot Safety Gate Review（2026-07-16）

## 1. 结论

**G2 = GO**，但该结论只适用于当前已经实现的受控、单次、人工确认
`novel_autopilot` MVP 安全门禁。它证明现有闭环在固定样本、权限/确认拒绝、
workflow CAS、audit 持久化故障和只读 history 故障下保持安全。

G2 GO **不等于**授权无人值守、多步骤、多 Tool、整书或多卷 Autopilot。它也不授权
Pause/Resume/Steer、checkpoint/recovery/replay、自动重试、Provider/MCP runtime，
或生产数据库 migration。

## 2. 审查范围与固定样本边界

测试样本位于 `backend-rs/src/services/autopilot_safety_gate_fixture.rs`，仅以
`cfg(test)` 注册；它不参与 production runtime，也不是新的 Tool、workflow 或 audit
事实 owner。

fixture 使用固定的 project、owner、task、允许 Tool 与 phase transition：

```text
transition_project_workflow
foundation -> world_building
```

样本仅构造已确认的公开 contract 输入以及稳定失败码 `tool_execution_failed`。其中不包含
Prompt、reason、Provider/model、credential、digest、raw audit arguments 或内部持久化错误。

## 3. Failure-injection 与回归矩阵

| 场景 | 证据与安全断言 | 结论 |
| --- | --- | --- |
| 已确认的 allowlisted transition | fixture 驱动 Coordinator/Tool/workflow 既有合同；仅允许固定 phase 变迁。 | PASS |
| 未确认、越权、scope/schema 拒绝 | API/Tool 在 workflow mutation 前拒绝；不写入越权状态。 | PASS |
| stale CAS / Tool 失败 | workflow 不做乐观更新；audit 仅投影稳定、脱敏失败码。 | PASS |
| queued audit 写入失败 | `queued_audit_write_failure_does_not_create_task_or_mutate_workflow`；task 不创建、不执行，workflow 保持不变。 | PASS |
| terminal succeeded audit 投影失败 | SQLite trigger 删除 audit row，Coordinator 回滚 workflow transaction，再 best-effort 投影 `failed/tool_execution_failed`。 | PASS |
| generic task terminal owner | `novel_autopilot_terminal_audit_failure_keeps_generic_task_as_failed_terminal_owner` 证明 generic runner 仍是唯一 failed terminal owner。 | PASS |
| owner history 读取失败 | `owner_history_read_failure_is_safe_and_non_mutating` 返回稳定不可用错误，不返回 audit，不改变 workflow/task。 | PASS |
| non-owner history | 既有 R7 owner-scoped 回归保持拒绝与无泄露。 | PASS |
| 前端 history/确认动作 | workflow Playwright 继续证明 history 是只读展示，无控制/恢复 UI，确认动作不乐观更新 workflow。 | PASS |

terminal audit 投影失败的回归位于
`backend-rs/src/services/autopilot_coordinator_service.rs`。它明确验证：成功 audit
写入失败时，workflow transaction 回滚；若 fallback 可写，audit 最终为
`failed/tool_execution_failed`；fallback 再失败仅记录安全日志，不对外泄露 persistence
细节。Coordinator 与 audit 都不会接管 generic task 的 terminal lifecycle。

## 4. Owner 与边界矩阵

| 层 | 唯一职责 | 不承担的职责 |
| --- | --- | --- |
| Route | Claims、path scope、DTO/confirmation contract | task lifecycle、workflow 写入、audit history 事实 |
| Generic task | 创建、运行、失败/取消/完成的 terminal lifecycle owner | Tool schema、workflow CAS、audit read model |
| Coordinator | 单次 Tool 编排、事务回滚与安全 fallback | 直接数据库业务写入、task terminal 抢占、重试/恢复 |
| Tool Contract | allowlist、schema、confirmation 和 scope 验证 | task runtime、Provider/Prompt 执行 |
| Workflow service | canonical phase transition 与 CAS | audit projection、前端控制状态 |
| Durable audit | 脱敏、稳定的 queued/terminal 投影 | raw argument/error 保存、task terminal owner |
| History API | owner access 后的 readonly allowlist projection | 控制、恢复、replay 或第二份 audit 事实 |
| Frontend | 人工确认启动与只读展示 | workflow 乐观 mutation、Pause/Resume/Steer/retry UI |

## 5. 质量门（2026-07-16）

以下命令均以退出码 `0` 完成：

```powershell
cargo fmt --manifest-path backend-rs/Cargo.toml -- --check
cargo check --manifest-path backend-rs/Cargo.toml -j 1
$env:RUSTFLAGS='-C link-arg=/DEBUG:NONE'
cargo test --manifest-path backend-rs/Cargo.toml -j 1 autopilot -- --nocapture
npm --prefix frontend run lint
npm --prefix frontend run build
npm --prefix frontend run e2e -- e2e/project-workflow-state.spec.ts
```

结果：Rust fmt、check 通过；Autopilot 聚焦回归 **29/29 PASS**；frontend lint/build
通过；workflow Playwright **5/5 PASS**。

已记录但不阻断本门禁的既有告警包括：Rust chapter generation/generation contract
相关 unused code/import、frontend React Hook dependency、Vite circular-chunk warning。
这些告警并非 G2 引入，且不改变上述固定样本与 failure-injection 的安全结论。

## 6. 后续路线与冻结项

下一阶段为 **R8 Eval / 创作档案 / 运行指标**。R8 只能复用既有 workflow、task、
audit 与 generation-contract owner，建立脱敏、静态、可判定的评测和运行摘要；它不得
建立第二套事实、审计或恢复协议。

以下能力仍明确冻结，必须由未来独立任务重新设计、审查和验证，不能由本 G2 GO 推导获得：

- 无人值守、多步骤、多 Tool、整书或多卷 Autopilot；
- Pause/Resume/Steer、checkpoint/recovery/replay、自动重试；
- Provider/MCP runtime、真实 Prompt 执行或保存；
- 生产 migration/schema 扩展、任务中心控制 API/UI；
- 对现有 generic task、workflow、audit 或 history owner 的替换。
