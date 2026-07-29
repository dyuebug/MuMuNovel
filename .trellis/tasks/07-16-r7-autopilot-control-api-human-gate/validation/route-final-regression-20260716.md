# R7 当前工作树全路线回归引用（2026-07-16）

> 类型：验证索引。记录当前工作树执行结果；不替代各子任务的 focused test 明细。

## 当前执行结果

- `$env:RUSTFLAGS='-C link-arg=/DEBUG:NONE'; cargo test --manifest-path backend-rs/Cargo.toml -j 1 -- --nocapture`
  已通过；默认 suite 保留一个显式隔离 PostgreSQL 测试为 ignored，避免意外连接外部数据库。
- `npm --prefix frontend run e2e` 已通过。
- `npm --prefix frontend run lint` 已通过；仅保留既有 React Hook dependency warnings。
- `npm --prefix frontend run build` 已通过；仅保留既有 circular chunk warning。
- `npm --prefix frontend run e2e -- e2e/project-workflow-state.spec.ts`：**6/6 passed**。

## 冻结边界

R7 仍只授权单次、人工确认、`NonResumable` 的受控 Tool 调用。该回归不授权
Pause/Resume/Steer、replay、retry、checkpoint/recovery、Provider/MCP、多 Tool 或多步骤无人值守执行。
