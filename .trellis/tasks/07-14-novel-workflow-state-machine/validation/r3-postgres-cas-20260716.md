# R3 PostgreSQL 并发 CAS 复验（2026-07-16）

## 目的

验证小说 workflow 的条件更新在真实 PostgreSQL 并发场景下，面对两个携带相同
`expected_phase` 的转换请求，至多允许一个请求改变项目阶段。该验证用于补足默认
SQLite/mock 单元测试无法覆盖的数据库隔离级与 compare-and-swap 行为。

## 验证范围

- 目标测试：`postgres_concurrent_transitions_with_same_expected_phase_change_at_most_once`
- 实现 owner：`backend-rs/src/services/novel_workflow_service.rs`
- 核心约束：项目 owner scope、`expected_phase` 条件、单一 `projects.status` 事实 owner，及
  成功转换后的审计写入。

## 执行方式与结果

测试在本机 Docker PostgreSQL 的随机临时 role 与数据库中执行；数据库在测试前初始化，
测试结束后删除临时 role 和数据库。未连接、读取、修改或迁移生产数据库，也没有保留连接
URL、端口、账号、密码或容器环境变量等敏感运行配置。

```text
cargo test --manifest-path backend-rs/Cargo.toml -j 1   postgres_concurrent_transitions_with_same_expected_phase_change_at_most_once   -- --ignored --nocapture

result: 1 passed; 0 failed; 0 ignored; 1801 filtered out
```

## 结论与边界

本次结果证明当前工作树中的 PostgreSQL 条件更新可以阻止同一预期阶段的并发请求重复
改变 workflow phase。它不替代全路线质量门，也不授权生产数据库变更、增加 workflow
状态 owner、扩展转换表，或新增 checkpoint/recovery/replay 控制面。
