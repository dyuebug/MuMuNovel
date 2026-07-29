# Implementation Plan: R8 Eval / 创作档案 / 运行指标

## Ordered Checklist

1. 读取 R7/G2 审查、project export、generation contract/execution audit、quality trend 与 task/workflow
   read model，枚举可安全复用的字段，先形成 allowlist 表。
2. 新增 `cfg(test)` R8 golden sample fixture 和纯 evaluator/projection regression；覆盖正常、缺失、
   unknown schema 和敏感字段拒绝，且不引入 runtime fixture 或网络依赖。
3. 在既有 project export context 后新增最小 archive projection（或收紧既有 projection）；编写 owner
   scope、redaction、旧 history 和无 mutation 回归。
4. 构建 readonly metrics summary：只聚合 existing workflow/task/quality/audit safe read model；为
   空数据、局部历史和查询失败定义稳定语义与错误投影。
5. 仅在后端 read model 已稳定后更新前端 types/展示；UI 不增加任何控制、恢复、重试或启动动作。
6. 更新路线与 R8 评审证据，记录字段 owner、redaction、质量门与仍冻结的 Autopilot 能力。
7. 运行 Rust fmt/check/focused tests、frontend lint/build 和相关 owner-scoped E2E；发现敏感字段、
   owner 边界或兼容问题则缩小输出并重跑。

## Validation Commands

```powershell
cargo fmt --manifest-path backend-rs/Cargo.toml -- --check
cargo check --manifest-path backend-rs/Cargo.toml -j 1
$env:RUSTFLAGS='-C link-arg=/DEBUG:NONE'
cargo test --manifest-path backend-rs/Cargo.toml -j 1 r8 -- --nocapture
npm --prefix frontend run lint
npm --prefix frontend run build
npm --prefix frontend run e2e -- <R8 owner-scoped readonly spec>
```

最终 focused test 名称必须包含 `r8`；在代码尚未形成前，先运行相关 generation/export/quality/
autopilot focused tests 验证复用路径。

## Review Gates

- static sample 不能成为 production 数据源或可上传的运行时配置。
- archive 与 metrics 必须从明确 allowlist DTO 产生，不得 clone/serialize 原始 export context、ORM model、
  audit record 或 JSON history。
- 任何 route/UI 只能 readonly；不得引入控制、恢复、retry、replay 或 Autopilot 执行。
- unknown schema、缺失历史、owner 无权与读取错误都必须 fail closed 或返回安全稳定摘要。
- 禁止 migration/schema、新 store/new owner、Provider/MCP、Prompt/credential/raw arguments/errors。
- 所有新增文本为 UTF-8 无 BOM、LF、无尾随空白。

## Risky Files and Rollback Points

- `backend-rs/src/api/projects.rs`：不得将现有 project export 扩大为内部数据转储。
- `backend-rs/src/services/generation_execution_audit_service.rs`：不得改变 audit schema 或保存敏感输入。
- `backend-rs/src/services/chapter_query_service/`：metrics 只能新增 derived read model，不得重定义质量事实。
- `backend-rs/src/api/autopilot.rs`：仅可读取现有安全 audit projection，不能添加控制或恢复路径。
- `frontend/src/types/index.ts` 与项目详情面板：只消费 versioned readonly DTO，不改变 workflow/task 状态。


## Implementation Evidence (2026-07-16)

### Completed Deliverables

- [x] `cfg(test)` static R8 fixture and deterministic evaluator project only approved
  generation-contract/audit schema versions, intent kind, execution role, and quality-summary
  presence. Unknown source or summary schemas are rejected with stable errors; private target,
  digest, provider/model, fallback, endpoint, Prompt, and raw execution fields are excluded.
- [x] `GET /projects/{project_id}/creative-archive` is owner-scoped and projects an explicit
  archive allowlist: generation timestamp, chapter feedback link, generation-contract version and
  intent, execution-audit version and role, plus quality score/gate decision. Legacy, malformed,
  or unknown history shapes fail closed to absent safe summaries.
- [x] `GET /projects/{project_id}/runtime-metrics` returns the versioned
  `runtime-metrics/v1` `derived_readonly` DTO. It reads the existing workflow service, runtime
  `TaskRegistry`, project quality-trend read model, and durable Autopilot invocation audit without
  adding a store, schema, migration, or canonical owner.
- [x] Metrics use `available`, `empty`, and `unavailable` as read-model availability states.
  Task and Autopilot-audit counts are fixed-limit runtime-observed samples (100 each), not durable
  history or control state; workflow/quality/audit sub-read failures become a safe unavailable
  section without leaking storage detail.
- [x] The Project Detail page consumes only the typed readonly DTO, fetches once per project entry,
  has cancellation-safe request cleanup, does not poll or auto-refresh, and exposes no R8 control,
  resume, retry, replay, checkpoint, or Autopilot-launch action.

### Validation Evidence

- `rustfmt --edition 2021 --check backend-rs/src/api/projects.rs backend-rs/src/services/runtime_metrics_service.rs backend-rs/src/services/mod.rs`
- `cargo check --manifest-path backend-rs/Cargo.toml -j 1`
- `$env:RUSTFLAGS='-C link-arg=/DEBUG:NONE'; cargo test --manifest-path backend-rs/Cargo.toml -j 1 r8 -- --nocapture`
  — 11 focused R8 tests passed.
- `npm --prefix frontend run lint`
- `npm --prefix frontend run build`
- `npm --prefix frontend run e2e -- e2e/project-workflow-state.spec.ts`
  — 6/6 passed, including the R8 derived readonly metrics contract case.

#### 路线级最终回归（2026-07-16）

- `$env:RUSTFLAGS='-C link-arg=/DEBUG:NONE'; cargo test --manifest-path backend-rs/Cargo.toml -j 1 -- --nocapture`
  — 1801 tests passed. Readiness 的 migration revision count 回归断言改为读取
  Rust-owned canonical `postgres_revision_catalog().len()`，避免后续新增 migration 时硬编码计数漂移。
- `npm --prefix frontend run e2e` — 14 passed / 13 skipped。恢复语义 E2E 的 mock 终态时间改为动态近时刻，
  使 fixture 不会被 production 的 12 小时 terminal-task retention 合法压缩；未改变 store 的 retention、
  恢复策略、UI 或 API 合约。
- `npm --prefix frontend run lint`、`npm --prefix frontend run build`、
  `rustfmt --edition 2021 --check backend-rs/src/api/health.rs`、
  `git -C backend-rs diff --check` 与 `git -C frontend diff --check` 均通过。

### Frozen Boundary

R8 is evidence and readonly projection only. It does not authorize unattended or multi-step
Autopilot, Provider/MCP execution, real Prompt evaluation, Pause/Resume/Steer, retry, replay,
checkpoint/recovery, a new Tool, a new task/workflow/audit owner, or schema/migration changes.
