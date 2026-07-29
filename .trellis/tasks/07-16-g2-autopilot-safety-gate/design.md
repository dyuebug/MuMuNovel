# Design: G2 Autopilot Safety Gate

## Boundary

G2 不是 Autopilot 能力扩展，而是为 R7 已有的单次受控闭环建立更强的回归证据：

```text
fixed, non-sensitive fixture
  -> action/task payload and Coordinator tests
  -> Tool Contract / workflow CAS / durable audit tests
  -> injected persistence failures
  -> generic task terminal owner + readonly history behavior
  -> existing frontend readonly workflow E2E
```

生产运行时不读取 fixture，不新增 API、数据库表/字段、migration、Provider、Prompt 或任务控制。

## Fixed Fixture Contract

新增一个仅在 `cfg(test)` 编译的共享 fixture module，集中定义：

- canonical project/owner/task identifiers；
- confirmed `foundation -> world_building` payload；
- scope-mismatched payload；
- stale expected-phase payload；
- 允许结果与稳定错误码的断言常量。

fixture 的 payload 仅含 R7 已公开的 Tool 与 workflow phase；禁止包含 reason 原文、raw audit
arguments、Prompt、凭据、Provider/model 或 digest。各模块测试通过此 fixture 取得测试输入，
但继续在各自 owner 内断言自己的行为，避免创建第二套业务 contract owner。

## Failure Injection Strategy

| 场景 | 注入方式 | 必须证明 |
| --- | --- | --- |
| queued audit 写入失败 | 保留 project/workflow 表但不创建 audit 表，调用既有 task creation/action path。 | 不创建/不执行 task；workflow 不变；只返回稳定 creation error。 |
| Tool/CAS 失败 | 使用 fixture 的 stale phase 或 scope mismatch 进入既有 Coordinator/Tool。 | 无 workflow mutation；audit 为 stable failed code；无 raw error。 |
| succeeded terminal audit 写入失败 | 在 SQLite workflow update 上安装 test-only SQL trigger，使同一 transaction 内 audit row 暂时不可更新；`mark_succeeded` 返回 failure。 | transaction rollback；workflow 不变；Coordinator 返回安全错误；generic task lifecycle 仍拥有 failed terminal。 |
| terminal audit fallback | succeeded audit failure rollback 后，以既有 audit owner best-effort 写入 `tool_execution_failed`。 | 若数据库恢复可写，audit 进入 safe failed terminal；若仍不可写，仅记录 stable structured log，绝不抢占 generic task terminal owner。 |
| history read 失败 | project access table 保持可用，但不创建 audit 表。 | owner 得到稳定 history-unavailable error；无 audit 内容、task 或 workflow mutation。 |

SQLite trigger 只存在于测试数据库；它模拟成功投影写入时的竞态/持久化失败，不引入生产
fault flag、运行时 hook 或可被外部调用的测试 API。

## Owner Rules

- API 继续只处理 Claims/path/strict DTO，并委托 generic task lifecycle。
- generic task subsystem 继续拥有 pending/running/terminal state；G2 不给 audit 或 Coordinator
  增加 task terminal 写入权。
- Tool Contract 继续拥有 schema、confirmation、scope 及 workflow adapter；G2 不复制转换规则。
- workflow service 继续拥有 CAS 和业务事务；audit service 继续拥有 durable projection/redaction。
- frontend 继续只消费 history allowlist 和 generic task feedback；不新增控制/恢复 UI。

## Minimal Production Fix

当前 Coordinator 在 workflow 成功但 succeeded audit projection 失败时会 rollback workflow 并返回
安全错误。G2 在该分支增加一次 **rollback 后、best-effort 的既有 audit-owner failed projection**：
使用稳定 `tool_execution_failed`，不携带 raw persistence detail。该补偿只改善 audit 可追溯性；若
再次写入失败，仍由 generic task runner 保持唯一终态 owner。

## Compatibility and Rollback

- 无 API/schema/migration/public task status/SSE event 改动。
- fixture 仅 `cfg(test)`；移除 fixture 与 G2 tests 即可回滚测试资产。
- Coordinator fallback 是一个小的既有 audit service 调用；回滚时删除该 best-effort 块即可恢复
  R7 原行为，不触及 workflow/task owner。
- 不执行真实 Provider、生产数据库或网络依赖。
