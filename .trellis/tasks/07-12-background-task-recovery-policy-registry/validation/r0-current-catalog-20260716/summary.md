# R0 当前 PostgreSQL catalog 复验摘要（2026-07-16）

> 验证日期：2026-07-16
> 验证范围：current catalog、Alembic head、Rust migration executor、release readiness 与
> `user_passwords.password_hash` storage contract。
> 环境边界：本机 Docker PostgreSQL 18 的随机临时数据库；验证完成后已清理。未连接、修改或迁移
> 生产数据库。

## 当前 catalog

- Rust/Python migration catalog：**21 revisions / 123 upgrade SQL steps**；
- canonical PostgreSQL head：`20260716_autopilot_invocation_audit`。

## 验证结果

| 检查 | 结果 | 原始证据 |
|---|---|---|
| Alembic graph health | 通过；graph healthy，head 一致 | `alembic-health.log` |
| 空库 Alembic upgrade | 已执行至 current head；最后记录的 upgrade 为 `20260712_password_hash_phc_text -> 20260716_autopilot_invocation_audit` | `alembic-upgrade.log` |
| Alembic current | `20260716_autopilot_invocation_audit (head)` | `alembic-current.log` |
| Rust migration executor | `exit_code=0`，`status=already_at_catalog_head` | `migration-executor.log` |
| Release preflight | `release_ready=true`、`runtime_ready=true`、`status=ready` | `release-preflight.log` |
| 数据库存储合同 | `alembic_head=20260716_autopilot_invocation_audit`；`password_hash=text|NO|-` | `database-contract.log` |

## 编码说明

Windows 默认 GBK stdout 在验证脚本输出 `✅` 时会触发 `UnicodeEncodeError`。该异常发生在打印阶段；
`alembic-upgrade.log` 已记录完整 upgrade 运行，且最终 head、Rust executor、release preflight 和
information_schema 查询均成功。复现该验证时仅为当前 PowerShell 进程设置：

```powershell
$env:PYTHONUTF8 = '1'
```

这不是 schema 或 migration 缺陷，且不得为此修改冻结的历史 migration revision。

## 结论与边界

当前隔离 PostgreSQL 验证通过，说明 canonical catalog 与 Rust/Python/Alembic/readiness 合同一致。
它不授权 production migration、downgrade CLI、历史 revision 重写或任何 schema 扩展；上述动作仍需
单独的授权与验证。
