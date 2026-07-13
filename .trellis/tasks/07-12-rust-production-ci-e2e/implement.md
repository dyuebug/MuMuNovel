# Implementation Plan：Rust 生产 CI 与真实 Rust E2E 对齐

## Current Status（2026-07-13）

```text
R0.1 = PASS
R0.2 = PASS
R0.3 = LOCALLY COMPLETE / GITHUB RUNNER PENDING
G0   = NO-GO
```

本文后续按时间记录的早期状态仅代表当时切片；以本节和文末
“R0.3 Local Runner Contract Completion”为当前权威状态。R0.3 必须由实际 GitHub Runner 绿色运行与
可审计 artifact 完成，本地合同和 R0.2 证据均不可替代。

## Steps

1. 读取 backend/Trellis 规范和现有 workflow，确认路径触发、Rust 版本及 runtime 环境变量。
2. 本地运行严格 Clippy，评估是否存在 R0 内可修复的既有 warning。
3. 修改 `.github/workflows/backend-ci.yml`：
   - 增加 `backend-rs/**` path filter；
   - 增加 Rust 1.88 setup/cache；
   - 增加 fmt/check/test/clippy 生产门禁；
   - 将 Python job 重命名为 migration/support regression。
4. 修改 `.github/workflows/e2e-smoke.yml`：
   - path filter 覆盖 `backend-rs/**`；
   - 增加 PostgreSQL service；
   - 安装 Rust 1.88；
   - 移除 SQLite Alembic 和 Uvicorn 启动；
   - 运行 Rust migration executor；
   - 后台启动 Rust server 并等待 `/health`；
   - 保留 Playwright smoke、日志输出和 PID 清理。
5. 搜索 workflow 中残留的 `uvicorn`、缺失 SQLite 配置和生产所有权错误描述。
6. 运行验证命令并记录结果。
7. 将 CI/runtime 所有权契约补充到 backend quality spec，并更新路线文档 R0 状态。

## Validation

```powershell
cargo fmt --manifest-path "backend-rs/Cargo.toml" -- --check
cargo check --locked --manifest-path "backend-rs/Cargo.toml"
cargo test --locked --manifest-path "backend-rs/Cargo.toml"
cargo clippy --locked --manifest-path "backend-rs/Cargo.toml" --all-targets -- -D clippy::correctness -D clippy::suspicious
npm run build --prefix "frontend"
python -c "import yaml, pathlib; [yaml.safe_load(pathlib.Path(p).read_text(encoding='utf-8')) for p in ['.github/workflows/backend-ci.yml','.github/workflows/e2e-smoke.yml']]"
E:/Code/SoftWare/Tools/Git/cmd/git.exe diff --check
```

如果本机 Docker/PostgreSQL 可用，额外执行与 CI 等价的迁移和 Rust server smoke；否则以 Rust migration executor 单元/集成测试、YAML 解析和 GitHub service contract 作为本地证据，并明确记录环境限制。

## 风险文件

- `.github/workflows/backend-ci.yml`
- `.github/workflows/e2e-smoke.yml`
- 可能的小范围 Rust Clippy 修复文件
- `.trellis/spec/backend/quality-guidelines.md`
- `docs/15-ainovel-cli-comparison-and-mumunovel-optimization.zh-CN.md`

## 回滚点

- backend CI 两个 job 可独立回滚。
- E2E 仅回滚 workflow，不修改 Playwright 配置和生产 Compose。
- 不通过增加全局 lint allow 或跳过 Rust tests 来换取绿色 CI。

## 启动前检查

- [x] Rust runtime 所有权已由 Nginx/Compose 证实。
- [x] Rust migration executor 可替代 Python migrator 的契约已证实。
- [x] E2E 使用的认证和后台任务 smoke 路径已确认。
- [x] 非目标和回滚边界已明确。
- [x] 用户已要求持续进行 Rust 优化开发。

## Implementation Results (2026-07-12)

### Completed Changes

- [x] `.github/workflows/backend-ci.yml` now triggers on `backend-rs/**`.
- [x] Added the `rust-production` job with Rust 1.88, Rust cache, fmt, check,
  test, and the incremental high-confidence Clippy gate.
- [x] Renamed the Python job to `python-migration-support` and retained pytest
  as migration/support regression coverage.
- [x] `.github/workflows/e2e-smoke.yml` now uses PostgreSQL 18, Rust 1.88, the
  Rust migration executor, the Rust server on port 8003, and the existing auth
  plus background-task Playwright smoke specs.
- [x] Removed Python setup, SQLite Alembic, and Uvicorn runtime startup from the
  E2E workflow.
- [x] Added Rust log output, PID cleanup, and failure-only Playwright report
  upload.
- [x] Fixed the migration lock file open contract with explicit
  `.truncate(false)` in production and test lock acquisition.

### Clippy Baseline Decision

Strict `cargo clippy --all-targets -- -D warnings` exposed about 208 historical
diagnostics and would expand R0 into a broad unrelated refactor. The accepted
incremental gate is:

```powershell
cargo clippy --locked --manifest-path "backend-rs/Cargo.toml" --all-targets -- -D clippy::correctness -D clippy::suspicious
```

It initially found the migration lock open-options diagnostic. After adding
`.truncate(false)`, the gate passes. About 206 non-blocking historical warnings
remain and must be removed by a dedicated cleanup task before switching to full
`-D warnings`. No global or crate-wide lint suppression was added.

### Validation Evidence

- `cargo fmt --check`: PASS.
- `cargo check --locked`: PASS.
- `cargo test --locked`: PASS, 1524/1524 tests.
- Clippy correctness + suspicious gate: PASS.
- `npm run build --prefix frontend`: PASS.
- PyYAML parsing for both workflow files: PASS.
- UTF-8 without BOM check for both workflow files: PASS.
- Workflow contract search: PASS; no `uvicorn` or `alembic-sqlite` remains in
  `e2e-smoke.yml`.
- `git diff --check`: PASS.

### Real Local E2E Follow-up (2026-07-12)

Docker Desktop later became available, so the real local chain was executed with
a temporary PostgreSQL 18 container, the Rust migration executor, the Rust server,
`/health`, and the existing Playwright auth/background-task smoke specs.

Verified successfully:

- PostgreSQL became healthy.
- Rust migrations completed successfully.
- The Rust server started successfully.
- `/health` returned HTTP 200.
- Vite and Playwright reached the real Rust API.

The chain then failed during local administrator authentication. A direct
`POST /api/auth/local/login` returned:

```text
HTTP 500
Query Error: value too long for type character varying(64)
```

The failure path is `AuthService::ensure_local_admin()` -> Argon2 password hash ->
`user_passwords.password_hash`. The current PostgreSQL column is `VARCHAR(64)`,
which cannot store the generated Argon2 hash. This is a real production contract
failure, not an environment-variable propagation issue.

### Revised Remaining Verification

R0 is **not implementation-complete**. The required order is now:

1. R0.1: after explicit user approval for a database schema change, widen the
   `password_hash` contract and keep the Rust migration, initial schema, and frozen
   Python migration/source-map contracts aligned.
2. R0.2: rerun the complete local PostgreSQL + Rust + Playwright chain, including
   `auth.spec.ts` and `background-task-pages.spec.ts`.
3. R0.3: obtain a green GitHub runner execution of the same chain.
4. Run G0 only after R0.1-R0.3 and the already implemented R1/R2 gates are green.

No schema change was made as part of this documentation update. R0 and G0 remain
open, and R3 must not start.

## R0.1 执行清单（待数据库 Schema 变更确认）

> 状态：设计完成，尚未实施。以下任何 Schema/migration 源码修改都必须在用户明确确认后执行。

### A. Rust migration catalog

- [ ] 将 `POSTGRES_ALEMBIC_HEAD` 更新为 `20260712_password_hash_phc_text`。
- [ ] 在 `POSTGRES_REVISION_CATALOG` 末尾追加第 20 项，`down_revision` 指向
  `20260517_project_core_defaults`。
- [ ] 新增 upgrade steps：将 `user_passwords.password_hash` 转为 `TEXT` 并更新列注释。
- [ ] 新增 guarded downgrade steps：长值存在时抛错，禁止截断；安全时才恢复 `VARCHAR(64)`。
- [ ] 在 `RUST_EXECUTABLE_POSTGRES_REVISIONS` 注册相同 revision、filename 和步骤。

### B. Bootstrap 与冻结 source-map

- [ ] 将 Rust `schema_migration_initial_schema.sql` 的字段更新为 `TEXT NOT NULL`，注释改为
  Argon2 PHC/legacy SHA-256 兼容语义。
- [ ] 新增 Python PostgreSQL Alembic revision 文件作为 frozen source-map；不改写历史初始 revision。
- [ ] 将 `backend/migrator_app/models/user.py` 的 metadata 类型从 `String(64)` 更新为 `Text`。
- [ ] 保持 Rust migration executor 为唯一生产 owner，不恢复 Python startup migration。

### C. 测试和固定契约

- [ ] 更新 `schema_migration_metadata_service.rs` 中 revision count/head/step 断言。
- [ ] 更新 `api/health.rs` 中 revision count、expected head、live head 和 smoke fixture 断言。
- [ ] 增加 initial schema 字段类型/注释防漂移测试。
- [x] 增加 Argon2 verifier 形状测试，证明当前生成值长度大于 64 且可被现有验证器解析。
- [ ] 增加真实 PostgreSQL migration/auth 回归，覆盖空库和旧 head 两条路径。
- [ ] 在真实 PostgreSQL 中验证 legacy SHA-256 值迁移前后保持一致，并在首次成功登录后升级为 Argon2。
- [x] 增加 Rust `AuthService::login_local()` 数据库回归：正确 legacy 密码登录后升级为 canonical
  Argon2，错误密码不修改 verifier 或 `updated_at`。
- [ ] 验证 downgrade guard 在存在长 verifier 时失败且不修改数据。

### D. R0.2 / R0.3 门禁

```powershell
cargo fmt --manifest-path "backend-rs/Cargo.toml" -- --check
cargo check --locked --manifest-path "backend-rs/Cargo.toml"
cargo test --locked --manifest-path "backend-rs/Cargo.toml"
cargo clippy --locked --manifest-path "backend-rs/Cargo.toml" --all-targets -- -D clippy::correctness -D clippy::suspicious
python backend/tools/check_alembic_revision_health.py
npm --prefix frontend run build
npm --prefix frontend run lint
```

真实 PostgreSQL/Rust E2E 必须按顺序证明：

```text
PostgreSQL 18 healthy
  -> Rust migration-executor reaches 20260712_password_hash_phc_text
  -> information_schema reports password_hash as text and NOT NULL
  -> Rust server /health 200
  -> POST /api/auth/local/login succeeds
  -> auth.spec.ts passes
  -> background-task-pages.spec.ts passes
```

本地 R0.2 全绿后才能请求 R0.3 GitHub runner 证据；R0.3 全绿后才执行 G0。

### E. 风险与回滚

- 应用代码回滚不要求 schema 降级：旧代码读取 `TEXT` 与读取 `VARCHAR` 的 SeaORM `String`
  映射兼容。
- schema downgrade 是独立高风险动作，仅在所有 verifier 长度 ≤64 时允许。
- 如果 migration 执行失败，保持旧 head，不启动 Rust server，不通过 startup sync 绕过。
- 如果 auth 回归失败，保留 PostgreSQL/Rust 日志和 Playwright report，不回退到 Python runtime。

### R0.1 安全前置：Password Hash Owner 收敛（2026-07-12）

在未修改数据库 Schema 的前提下，Rust 密码哈希算法实现已收敛为唯一 Service Owner：

- 新增 `backend-rs/src/services/password_hash_service.rs`，统一负责 Argon2 PHC 生成、验证、
  legacy SHA-256 识别和兼容验证；
- `services/auth.rs` 不再维护私有 Argon2/SHA-256 副本，只负责本地认证工作流和 legacy
  成功登录后的持久化升级；
- `api/auth.rs`、`api/admin.rs`、`api/user_admin_shared_owner.rs` 的密码创建、设置和重置
  均直接调用 Service Owner；API 层仍把错误映射为原有 HTTP 500 detail 文案；
- 仓库静态审计确认 `Argon2::default()`、`PasswordHash::new()` 和 `SaltString::generate()`
  只存在于 `password_hash_service.rs`。

固定兼容契约：

- Argon2 verifier 使用独立随机 salt，同一密码连续生成的 verifier 不相同；
- PHC verifier 以 `$argon2` 开头且长度大于 64，继续证明历史 `VARCHAR(64)` 契约不足；
- 正确密码返回 `true`；只有 `password_hash::Error::Password` 映射为普通密码错误并返回 `false`；
- 64 字符十六进制 legacy SHA-256 verifier 保持正确/错误密码兼容，并接受历史大写十六进制值；
- 非 PHC、非 legacy 的损坏 verifier 返回显式 `invalid password hash` 错误，不伪装成密码错误；
- 可解析但不受支持的 PHC algorithm、version 或参数错误返回 `InvalidVerifier`，不得降级为 `false`；
- Rust 当前生成的 Argon2 verifier 固定为 canonical 合同：`argon2id`、`v=19`、`m=19456`、
  `t=2`、`p=1`、32 字节输出；验证前先检查该合同，再执行昂贵的 Argon2 计算，避免数据库中
  被篡改的超大 `m/t/p` 参数放大登录资源消耗；
- `AuthService::authenticate_local()` 通过 `LocalPasswordDecision` 显式区分认证成功和普通凭证错误，
  并保持 `InvalidVerifier` 错误向上游传播，不得伪装为用户名或密码错误；
- 回归测试分别覆盖 `$scrypt$` algorithm、`v=42` version、`m=0` 非法参数，以及
  `m=65536`、`t=3`、`p=2` 三类非 canonical 参数；
- legacy SHA-256 verifier 先解码到固定 32 字节缓冲区，再使用 `subtle::ConstantTimeEq`
  比较原始摘要，保留大写十六进制兼容并避免普通字符串 `==` 的潜在提前退出；
- `subtle 2.6.1` 原本已由 Argon2/password-hash 锁定，本步骤只将其声明为 Rust 后端直接依赖，
  未升级依赖版本。

验证证据：

```powershell
cargo test --locked --manifest-path "backend-rs/Cargo.toml" password_hash
cargo test --locked --manifest-path "backend-rs/Cargo.toml" services::auth::tests
cargo test --locked --manifest-path "backend-rs/Cargo.toml" api::auth::tests
cargo test --locked --manifest-path "backend-rs/Cargo.toml" api::admin::tests
cargo test --locked --manifest-path "backend-rs/Cargo.toml" api::user_admin_shared_owner::tests
cargo fmt --manifest-path "backend-rs/Cargo.toml" -- --check
cargo check --locked --manifest-path "backend-rs/Cargo.toml"
cargo test --locked --manifest-path "backend-rs/Cargo.toml"
cargo clippy --locked --manifest-path "backend-rs/Cargo.toml" --all-targets -- -D clippy::correctness -D clippy::suspicious
```

结果：密码哈希 10/10、AuthService 业务与数据库边界 7/7、Auth API 9/9、Admin API 14/14、
共享用户管理 20/20、完整 Rust 1558/1558 全部通过；fmt、check、Clippy
correctness+suspicious、唯一 Owner 静态审计和 UTF-8 无 BOM 均通过。
新增的 AuthService 内存数据库回归经过真实 `login_local()` 控制流，证明 legacy SHA-256 正确密码
会在 JWT 返回前持久化升级为可验证的 canonical Argon2 PHC，错误密码路径保持原 verifier 和时间戳不变；
canonical Argon2 正确登录不会重复 rehash，也不会更新 `updated_at`；损坏 verifier 返回显式
`invalid password hash` 错误，并保持数据库 verifier 与 `updated_at` 不变。
完整测试期间发现 `prompt_workshop` 两条测试并发修改进程级 `INSTANCE_ID` 的既有竞态，现已使用
测试专用 `Mutex` 与 RAII guard 串行化并保证 panic 路径恢复；相关 12 条测试以 16 线程连续
20 轮全部通过。Clippy 仍输出仓库既有 warning 基线，本步骤未新增 correctness/suspicious 失败。

本步骤没有修改 `password_hash` 字段类型、revision catalog、initial schema、Python Alembic
source-map、migrator metadata 或 health revision contract。R0.1、R0.2、R0.3 和 G0 仍保持未完成。

### R0.1 Migration revision 原子事务与 Settings 测试夹具稳定化（2026-07-13）

在未修改数据库 Schema、revision catalog、initial schema、Python Alembic source-map、
migrator metadata 或 health revision contract 的前提下，补齐 R0.1 继续实施前的失败原子性与
测试稳定性安全前置。

Migration Executor 变更：

- `run_rust_migration_tail_hardening_replay()` 不再直接逐条执行 revision SQL 后单独更新
  `alembic_version`，改为调用 `execute_rust_migration_revision_atomically()`；
- 每个 revision 使用独立事务，事务内依次执行全部 SQL steps 并更新 revision head，只有两者
  同时成功才提交；
- SQL step 失败返回 `blocked_sql_execution_error`，head 更新失败返回
  `blocked_alembic_version_update_error`，两类失败均显式 rollback，并在诊断中记录
  `revision transaction rolled back`；
- transaction begin/commit 分别保持 `blocked_transaction_begin_error` 和
  `blocked_transaction_commit_error`；
- 回滚边界只覆盖当前失败 revision，已经提交的前一 revision 不回滚；
- `execute_raw_sql_step()` 与 `update_live_alembic_revision()` 改为接收实现
  `ConnectionTrait` 的连接，使同一逻辑可同时用于 `DatabaseConnection` 和
  `DatabaseTransaction`。

新增回归测试：

- `revision_transaction_commits_sql_steps_and_head_together`：SQL steps 与 head 一起提交；
- `revision_transaction_rolls_back_prior_steps_when_sql_fails`：后续 SQL 失败时回滚当前 revision
  已执行的 SQL，head 保持旧值；
- `revision_transaction_rolls_back_sql_when_head_update_fails`：head 更新失败时回滚当前 revision SQL。

定向结果：`schema_migration_metadata_service` 26/26 通过。

完整门禁首次运行时，Settings 本地临时 HTTP server 测试出现并行非确定性：失败用例单独重跑
均通过，但 16 线程压力运行可随机复现 OpenAI/Sub2API/Gemini probe 失败。测试夹具已做最小加固：

- 合并重复 server 启动逻辑为 `spawn_test_http_server()`；
- 增加测试专用 `GET /__test_ready`，helper 返回前使用禁用代理、2 秒超时的独立 HTTP client
  验证 Axum 已实际处理请求，不使用固定 sleep；
- 使用 `TestHttpServerHandle` RAII guard，在显式清理遗漏或 panic 路径中兜底 abort；
- 保留现有 `handle.abort()` 调用契约，不修改生产 API probe 行为。

验证证据：

```powershell
cargo test --locked --manifest-path "backend-rs/Cargo.toml" api::settings::tests -- --test-threads=16
# 连续 20 轮
cargo fmt --manifest-path "backend-rs/Cargo.toml" -- --check
cargo check --locked --manifest-path "backend-rs/Cargo.toml"
cargo test --locked --manifest-path "backend-rs/Cargo.toml"
cargo clippy --locked --manifest-path "backend-rs/Cargo.toml" --all-targets -- -D clippy::correctness -D clippy::suspicious
```

结果：Settings 50 条测试以 16 线程连续 20 轮全部通过，累计 1000 条无失败；完整 Rust
1561/1561 通过，fmt、check、Clippy correctness+suspicious 全部通过。Clippy 仅保留仓库既有
warning 基线。本步骤仍未修改 `password_hash` Schema，R0.1、R0.2、R0.3 和 G0 均保持未完成，
也不构成数据库 Schema 变更授权。

### R0 Workflow 契约防漂移测试（2026-07-13）

在 R0.1 尚未取得独立数据库 Schema 变更授权期间，继续完成不涉及 Schema 的高价值可靠性工作：
使用 Rust 测试固化 GitHub Actions 的生产所有权、触发范围、门禁顺序和真实 E2E 链路，防止
workflow 回退到 Python runtime 或 SQLite 专用假绿路径。本步骤不修改 workflow 内容、不增加依赖，
测试模块仅在 `cfg(test)` 下编译，不进入生产二进制。

实现：

- `backend-rs/src/main.rs` 注册测试专用 `production_ci_contract_tests` 模块；
- `backend-rs/src/production_ci_contract_tests.rs` 通过 `include_str!` 读取
  `.github/workflows/backend-ci.yml` 与 `.github/workflows/e2e-smoke.yml`；
- `workflows_trigger_for_rust_backend_changes` 固化 `backend-rs/**` 对 backend CI 和 E2E 的触发；
- `backend_ci_keeps_rust_quality_gates_in_order` 固化 fmt → check → test → Clippy 顺序；
- `backend_ci_keeps_python_in_migration_support_role` 固化 Python 仅承担 migration/support 回归；
- `e2e_smoke_keeps_postgres_rust_and_playwright_execution_order` 固化 PostgreSQL → Rust migration →
  Rust server → health → Playwright 顺序；
- `e2e_smoke_rejects_python_runtime_and_preserves_failure_diagnostics` 禁止 `uvicorn`、
  `alembic-sqlite.ini`、`sqlite+aiosqlite` 回流，并保护 Playwright 报告、Rust 日志和进程清理。

验证证据：

```powershell
cargo test --locked --manifest-path "backend-rs/Cargo.toml" production_ci_contract_tests
cargo fmt --manifest-path "backend-rs/Cargo.toml" -- --check
cargo check --locked --manifest-path "backend-rs/Cargo.toml"
cargo test --locked --manifest-path "backend-rs/Cargo.toml"
cargo clippy --locked --manifest-path "backend-rs/Cargo.toml" --all-targets -- -D clippy::correctness -D clippy::suspicious
npm run build
```

结果：Workflow 契约测试 5/5、完整 Rust 1566/1566 全部通过；fmt、check、Clippy
correctness+suspicious 和前端构建通过；`backend-ci.yml`、`e2e-smoke.yml` 均通过 YAML 解析。
验证日志保存在 `.trellis/tasks/07-12-rust-production-ci-e2e/validation/`。

本步骤没有修改 `password_hash` 字段类型、Rust revision catalog、initial schema、Python Alembic
source-map、migrator metadata 或 health revision contract，不构成数据库 Schema 变更授权。
R0.1、R0.2、R0.3 和 G0 仍保持未完成；下一动作仍是取得独立 Schema 授权后实施 R0.1，随后
串行完成 R0.2、R0.3 与 G0。

### R0 Rust Readiness Migration Head Gate（2026-07-13）

审计发现 `/readyz` 虽然输出 `schema_migration.live_database_head`，但最终 `is_ready` 只依赖
startup 与数据库 ping；数据库可连接但 `alembic_version` 缺失、为空或与 Rust catalog head 不一致时，
接口仍可能返回 200。E2E workflow 同时只轮询 `/health`，因此没有消费已有 migration metadata
诊断，可能在 migration 状态错误时继续进入 Playwright。

实现：

- `backend-rs/src/api/health.rs` 将 `live_alembic_head_check.matches_catalog_head` 纳入
  `/readyz` 最终判定；
- 保留 `/health` 纯 liveness 行为和既有 readiness payload，不新增数据库写入；
- live head 匹配时返回 `200 ready`；head mismatch、缺表、空表、查询失败或数据库不可用时返回
  `503 not_ready`，具体原因继续由 `checks.schema_migration.live_database_head` 提供；
- `.github/workflows/e2e-smoke.yml` 的 Rust server wait 从 `/health` 切换为 `/readyz`；
- `production_ci_contract_tests` 固化 `/readyz` 执行顺序，并禁止 workflow 回退到
  `http://127.0.0.1:8003/health`。

新增回归：

- `should_report_ready_when_live_alembic_head_matches_catalog`；
- `should_report_not_ready_when_live_alembic_head_mismatches_catalog`；
- 原有 `should_expose_schema_migration_metadata_in_readiness_payload` 继续验证数据库不可用时的
  metadata 和 `503 not_ready` 契约。

验证结果：readiness 定向测试 3/3、Workflow 契约 5/5、完整 Rust 1568/1568 全部通过；fmt、
check、Clippy correctness+suspicious 和 `e2e-smoke.yml` YAML 解析通过。日志保存在：

```text
validation/readiness-cargo-fmt-check.log
validation/readiness-cargo-check-locked.log
validation/readiness-cargo-test-locked.log
validation/readiness-cargo-clippy-correctness-suspicious.log
```

本步骤没有修改 `password_hash` 字段类型、Rust revision catalog、initial schema、Python Alembic
source-map、migrator metadata 或 health 中固定 revision head/count contract，不构成数据库 Schema
变更授权。R0.1、R0.2、R0.3 和 G0 仍保持未完成；`/readyz` 通过也不能替代真实 auth E2E。


### R0 Rust Readiness Password Verifier Storage Gate（2026-07-13）

在未取得数据库 Schema 变更授权的前提下，继续补齐 `/readyz` 对历史 Auth Schema 的只读诊断。
仅检查 migration head 并不足以证明本地认证可用：真实 PostgreSQL 即使处于 catalog head，
`user_passwords.password_hash VARCHAR(64)` 仍无法容纳当前 canonical Argon2 PHC verifier（97 字符），
创建本地管理员时会在业务接口中延迟暴露为 HTTP 500。

实现：

- `password_hash_service.rs` 导出 canonical Argon2 verifier 的实际存储长度契约，并由真实哈希形状
  测试固定为 97，避免 readiness 与密码哈希 owner 漂移；
- `schema_migration_metadata_service.rs` 只读查询 PostgreSQL `information_schema.columns`，检查
  `user_passwords.password_hash` 的类型和容量，并输出
  `checks.schema_migration.auth_password_hash_storage` 结构化诊断；
- PostgreSQL `TEXT`、无界 `VARCHAR` 或容量至少 97 的 bounded `VARCHAR` 允许 readiness；其中只有
  `TEXT` 满足 R0.1 的最终 `unbounded_text` target contract；
- PostgreSQL `VARCHAR(64)`、缺列、不支持的类型或元数据查询失败均阻断 readiness 并返回
  `503 not_ready`；非 PostgreSQL 测试环境明确返回 `not_applicable_non_postgres`，不伪造
  PostgreSQL 兼容性证据；
- 数据库不可用时返回 `not_checked_database_unavailable` 并阻断 readiness。

新增/扩展回归覆盖容量足够、历史 64 字符容量、缺列、不支持类型、SQLite 非适用分支、数据库
不可用 payload 以及 readiness 组合判定。完整验证结果：

```text
cargo fmt --manifest-path backend-rs/Cargo.toml -- --check                       PASS
cargo check --locked --manifest-path backend-rs/Cargo.toml                       PASS
cargo test --locked --manifest-path backend-rs/Cargo.toml                        PASS (1574/1574)
cargo clippy --locked --manifest-path backend-rs/Cargo.toml --all-targets --
  -D clippy::correctness -D clippy::suspicious                                   PASS
```

验证日志：

```text
validation/password-storage-readiness-cargo-fmt-check.log
validation/password-storage-readiness-cargo-check-locked.log
validation/password-storage-readiness-cargo-test-locked.log
validation/password-storage-readiness-cargo-clippy-correctness-suspicious.log
```

该查询只读，不修改 `password_hash` 字段类型、Rust revision catalog、initial schema、Python
Alembic source-map、migrator metadata 或 health 固定 revision head/count contract，也不构成数据库
Schema 变更授权。真实 PostgreSQL `VARCHAR(64) -> /readyz 503` 仍需在 R0.2 或取得相应环境授权后
完成真实查询/解码链路验证；当前单元测试不能替代该证据。R0.1、R0.2、R0.3 和 G0 继续保持未完成。


### R0.3 Runner Readiness 结构化诊断证据准备（2026-07-13）

审计发现 E2E workflow 虽已等待 `/readyz`，但使用 `curl ... > /dev/null` 丢弃响应体；超时时仅
打印 Rust 后端日志。这样即使 `auth_password_hash_storage` 正确阻断了历史 `VARCHAR(64)`，GitHub
runner 也无法直接保留结构化原因，不利于 R0.2/R0.3 证据审查。

实现：

- Rust 后端日志改为写入工作区 `e2e-diagnostics/rust-backend.log`；
- readiness 循环保存最后一次响应到 `e2e-diagnostics/readyz.json`，并保存 HTTP 状态到
  `e2e-diagnostics/readyz-http-status.txt`；
- readiness 超时时在 step 日志中打印最后一次 HTTP 状态、JSON 响应和 Rust 后端日志；
- 任一后续步骤失败时上传独立 `rust-readiness-diagnostics` artifact；
- `production_ci_contract_tests` 新增防漂移契约，要求保存 readiness body/status、上传 artifact，
  并禁止重新将 `/readyz` 重定向到 `/dev/null`。

验证结果：Rust Workflow 契约 6/6、完整 Rust 1575/1575、fmt、check、Clippy
correctness+suspicious、`e2e-smoke.yml` YAML 解析和 readiness wait Bash 语法检查全部通过。
验证日志：

```text
validation/runner-readiness-cargo-fmt-check.log
validation/runner-readiness-cargo-check-locked.log
validation/runner-readiness-cargo-test-locked.log
validation/runner-readiness-cargo-clippy-correctness-suspicious.log
validation/runner-readiness-workflow-parse-and-shell-syntax.log
```

本步骤只增强 runner 诊断和证据保全，不修改数据库 Schema、migration catalog、initial schema、
Python Alembic source-map、migrator metadata 或 health revision contract。它不构成 R0.1 Schema
授权，也不能替代真实 PostgreSQL、Playwright 或 GitHub runner 执行证据；R0.1、R0.2、R0.3 与
G0 继续保持未完成。


### R0.2 Production Target Schema Contract Gate（2026-07-13）

进一步审计发现 `/readyz 200` 只证明数据库可以安全运行 canonical Argon2：无界 `VARCHAR` 或
容量至少 97 的 bounded `VARCHAR` 也会允许 readiness，但它们不满足 R0.1 已确定的最终
`unbounded_text` 契约。如果 R0.2 只检查 HTTP 200，临时兼容 Schema 可能被误判为 G0 候选。

实现：

- 在 readiness wait 与 Playwright 之间新增 `Verify Rust readiness production contracts`；
- 使用 workflow 已显式安装的 Node 20 读取保存在 `e2e-diagnostics/readyz.json` 的真实响应；
- 强制验证 readiness 状态、live migration head、canonical Argon2 容量支持；
- 额外要求 `matches_target_storage_contract === true` 且
  `target_storage_contract === 'unbounded_text'`；
- 失败时打印全部 readiness JSON，现有 `rust-readiness-diagnostics` artifact 会继续保留证据；
- Rust Workflow 防漂移契约保证该检查位于 readiness wait 与 Playwright 之间，不能被绕过。

定向验证：Workflow 契约 7/7；内联 Node 脚本对 `TEXT target=true` 返回 0，对“容量足够但
`matches_target_storage_contract=false`”返回非 0，并输出 R0.1 target contract 错误。完整验证结果：

```text
cargo fmt --manifest-path backend-rs/Cargo.toml -- --check                       PASS
cargo check --locked --manifest-path backend-rs/Cargo.toml                       PASS
cargo test --locked --manifest-path backend-rs/Cargo.toml                        PASS (1576/1576)
cargo clippy --locked --manifest-path backend-rs/Cargo.toml --all-targets --
  -D clippy::correctness -D clippy::suspicious                                   PASS
Workflow production contract tests                                               PASS (7/7)
YAML、Bash syntax、Node target/non-target behavior                                PASS
```

验证日志：

```text
validation/target-schema-gate-cargo-fmt-check.log
validation/target-schema-gate-cargo-check-locked.log
validation/target-schema-gate-cargo-test-locked.log
validation/target-schema-gate-cargo-clippy-correctness-suspicious.log
validation/target-schema-gate-workflow-behavior.log
```

本 Gate 不修改数据库 Schema，也不代表 R0.1 或 R0.2 已完成；它确保未来真实 E2E 只能在
R0.1 最终目标已经落地时通过，避免用运行时兼容性替代正式路线验收。

### R0.2 Rust-owned Release Readiness Endpoint（2026-07-13）

进一步收口生产契约所有权：此前最终 Schema target 判定位于 GitHub Workflow 的内联 Node
脚本，本地 R0.2 若要复用只能再次解析 `/readyz` JSON，存在 CI、本地脚本与 Rust metadata owner
三处规则漂移风险。

实现：

- 保留 `/readyz` 的 runtime readiness 语义：数据库可用、live migration head 匹配且 verifier
  存储容量能运行 canonical Argon2 时允许 200；
- 新增公开只读 `/releasez`，复用同一组 Rust 检查，但额外要求
  `matches_target_storage_contract == Some(true)`；
- `/releasez` 返回 `readiness_scope=production_release`、`runtime_ready`、`release_ready` 以及完整
  `checks`，兼容但非目标的 `VARCHAR(97+)`、无界 `VARCHAR`、SQLite 非适用证据均失败关闭；
- E2E Workflow 删除内联 Node 字段规则，改为直接请求 Rust `/releasez`；
- 保存 `releasez.json` 与 `releasez-http-status.txt`，和既有 `/readyz`、Rust 日志一起进入
  `rust-readiness-diagnostics` artifact；
- Rust 防漂移契约固定 `/readyz -> /releasez -> Playwright` 顺序，并禁止重新引入 Node target
  判断。

本地 R0.2 可在 Rust server 运行后直接执行：

```text
curl -f http://127.0.0.1:8003/readyz
curl -f http://127.0.0.1:8003/releasez
```

验证结果：

```text
health 定向测试                                                                 PASS (15/15)
auth middleware 定向测试                                                        PASS (3/3)
Workflow 防漂移契约                                                             PASS (7/7)
cargo fmt --manifest-path backend-rs/Cargo.toml -- --check                       PASS
cargo check --locked --manifest-path backend-rs/Cargo.toml                       PASS
cargo test --locked --manifest-path backend-rs/Cargo.toml                        PASS (1578/1578)
cargo clippy --locked --manifest-path backend-rs/Cargo.toml --all-targets --
  -D clippy::correctness -D clippy::suspicious                                   PASS
YAML parse + readyz/releasez Git Bash syntax                                      PASS
```

验证日志：

```text
validation/releasez-cargo-fmt-check.log
validation/releasez-cargo-check-locked.log
validation/releasez-cargo-test-locked.log
validation/releasez-cargo-clippy-correctness-suspicious.log
validation/releasez-workflow-contract.log
```

本改动只增加只读判定与诊断，不修改 `password_hash` 字段、migration revision、initial schema、
Python Alembic source-map 或 migrator metadata。它使 R0.2 的本地/CI 判定统一归属 Rust，但真实
PostgreSQL Auth E2E 和 GitHub Runner 证据尚未取得，因此 R0.1、R0.2、R0.3 与 G0 继续未完成。

### R0.2 Rust-owned Release Readiness Preflight CLI（2026-07-13）

为本地 R0.2 增加无需启动 HTTP server 的只读入口，同时避免 CLI、API 与 Workflow 三处复制
production release 判定规则。

实现：

- 新增 `production_readiness_service`，统一数据库 ping、live migration head、password verifier
  storage runtime/target 判定和 `/readyz`/`/releasez` JSON payload；
- `api/health.rs` 只保留 HTTP `200/503` 映射，`/readyz` 与 `/releasez` 的响应字段保持兼容；
- 新增 `release-readiness-preflight` 命令，直接连接配置数据库并复用同一 service；
- stdout 只输出 `readiness_scope=production_release` 的结构化 JSON，tracing、配置错误和连接错误
  输出到 stderr；
- 仅当 `release_ready=true` 时退出 `0`，其余状态退出 `1`；
- 防漂移契约检查命令已注册、调用共享 owner，且命令函数不含 migration executor、临时 Schema
  或 DDL 路径。

定向验证：

```text
production_readiness_service good/base/bad                              PASS (6/6)
production CI / CLI read-only 防漂移契约                               PASS (8/8)
/releasez SQLite fail-closed API 回归                                   PASS (1/1)
/readyz live-head match API 兼容回归                                    PASS (1/1)
SQLite 进程级 CLI：stdout JSON、stderr 日志、exit 1                     PASS
配置失败进程级 CLI：fail-closed JSON、stderr 原因、exit 1               PASS
```

完整 Rust 质量基线：

```text
cargo fmt --manifest-path backend-rs/Cargo.toml --all -- --check             PASS
cargo check --locked --manifest-path backend-rs/Cargo.toml                    PASS
cargo test --locked --manifest-path backend-rs/Cargo.toml                     PASS (1583/1583)
cargo clippy --locked --manifest-path backend-rs/Cargo.toml --all-targets \
  -- -W clippy::correctness -W clippy::suspicious                             PASS
```

验证 artifact：

```text
.trellis/tasks/07-12-rust-production-ci-e2e/validation/release-preflight-cargo-fmt-check.log
.trellis/tasks/07-12-rust-production-ci-e2e/validation/release-preflight-cargo-check-locked.log
.trellis/tasks/07-12-rust-production-ci-e2e/validation/release-preflight-cargo-test-locked.log
.trellis/tasks/07-12-rust-production-ci-e2e/validation/release-preflight-cargo-clippy-correctness-suspicious.log
.trellis/tasks/07-12-rust-production-ci-e2e/validation/release-preflight-sqlite.json
.trellis/tasks/07-12-rust-production-ci-e2e/validation/release-preflight-sqlite-stderr.log
.trellis/tasks/07-12-rust-production-ci-e2e/validation/release-preflight-config-failure.json
.trellis/tasks/07-12-rust-production-ci-e2e/validation/release-preflight-config-failure-stderr.log
.trellis/tasks/07-12-rust-production-ci-e2e/validation/release-preflight-process-contract.log
```

该 CLI 不执行 migration 或 DDL，也未连接真实 PostgreSQL。真实 PostgreSQL `TEXT` 目标 Schema、
auth 写入链路和 Playwright 证据仍依赖独立 Schema 授权后的 R0.1/R0.2；因此四个阶段状态不变。

### R0.2/R0.3 Runner Preflight Evidence Gate（2026-07-13）

为让 GitHub Runner 在 Rust server 启动前暴露 Schema、migration head 或数据库连接问题，已把
`release-readiness-preflight` 接入真实 Rust E2E workflow。该步骤严格位于 PostgreSQL migration
成功之后、`Start Rust backend` 之前，仍由 Rust `production_readiness_service` 拥有全部判定规则。

Workflow 契约：

- 分别保存 stdout、stderr 与原始退出码：`release-preflight.json`、
  `release-preflight-stderr.log`、`release-preflight-exit-code.txt`；
- 使用 `set +e` 仅包围 CLI 调用，捕获状态后立即恢复 `set -e`；
- CLI 失败时传播原始非零退出码，禁止 `|| true`、吞掉失败或仅保留终端输出；
- 现有 `rust-readiness-diagnostics` failure artifact 覆盖 `e2e-diagnostics/`，因此会同时上传上述
  三份 preflight 证据；
- Server 启动后的 `/readyz`、`/releasez`、HTTP 状态文件和 backend log 继续保留，preflight
  不能替代运行时证据。

新增防漂移契约：

```text
production_ci_contract_tests                                               PASS (9/9)
- migration -> release-readiness-preflight -> Start Rust backend 顺序固定
- JSON / stderr / exit-code 三文件固定
- set +e -> 捕获退出码 -> set -e -> 原退出码传播固定
- 禁止 release-readiness-preflight || true
```

质量验证：

```text
YAML parse                                                                  PASS
Git Bash bash -n                                                            PASS
cargo fmt --manifest-path backend-rs/Cargo.toml --all -- --check            PASS
cargo check --locked --manifest-path backend-rs/Cargo.toml                   PASS
cargo clippy --locked --manifest-path backend-rs/Cargo.toml --all-targets \
  -- -W clippy::correctness -W clippy::suspicious                            PASS
```

第一次完整 `cargo test --locked` 为 `1582 passed; 2 failed`。失败项均为历史 Settings 本地 HTTP
mock 并行非确定性：Gemini tool-call owner-path 与 Sub2API root-base-url fallback。两个测试随后各自
连续复跑 `3/3` 通过，且第二次完整回归为 `1584/1584 PASS`。当前没有证据支持修改 AI client
proxy/transport 行为，因此按 KISS 保留首次失败日志与最终复跑证据，不扩大本轮范围。

验证 artifact：

```text
validation/release-preflight-workflow-step.sh
validation/runner-preflight-contract-tests.log
validation/runner-preflight-cargo-fmt-check.log
validation/runner-preflight-cargo-check-locked.log
validation/runner-preflight-cargo-test-locked.log
validation/runner-preflight-cargo-test-locked-rerun.log
validation/runner-preflight-cargo-clippy-correctness-suspicious.log
validation/runner-preflight-workflow-parse-shell-syntax.log
validation/runner-preflight-test-rerun-summary.log
```

本切片未修改 `password_hash` 字段类型、Rust revision catalog、initial schema、Python Alembic
source-map、migrator metadata 或 health 固定 revision head/count，也没有取得真实 GitHub Runner
执行证据。因此 R0.1、R0.2、R0.3 与 G0 继续保持未完成，当前主线阻塞仍是独立 Schema 授权。

### Settings Local Gateway System Proxy Isolation（2026-07-13）

`Runner Preflight Evidence Gate` 首次记录的“当前没有证据支持修改 AI client proxy/transport
行为”是当时基于单次复跑的阶段性结论。后续并发压力和增强诊断已推翻该结论，因此保留原始
失败记录，同时以本节作为最终根因与修复判定。

修复前证据：

- 首轮压力测试第 12 轮：Gemini tool-call owner-path 返回 `success=false`；
- endpoint/status/response 诊断压力测试第 46 轮：mock 明确返回 `500`，实际 diagnostics
  收到 `502`；
- endpoint diagnostics 增强后的修复前压力测试第 1 轮：OpenAI v1 404 用例意外得到
  `success=true`；
- 失败横跨 OpenAI/Gemini、404/500/502/成功响应，证明不是单一 provider 解析分支问题。

根因判定：

- 项目使用 `reqwest 0.12.28`，默认 feature 启用了 `system-proxy`；
- 当前 Windows WinHTTP proxy 指向 `127.0.0.1:7897`；
- Settings readiness client 已使用 `.no_proxy()`，但 OpenAI、Gemini、Anthropic client
  原先仍允许 system proxy 接管后续真实 probe；
- 各测试数据库、AI service/client 和 diagnostics 均为调用局部实例，不支持共享 DB、共享
  client 或共享 diagnostics 的假设。

最小修复：

- 在 `backend-rs/src/ai/clients/mod.rs` 新增唯一
  `should_bypass_system_proxy(base_url)` owner；
- 仅对 `127.0.0.1`、`localhost`、IPv6 loopback 和 `host.docker.internal` 调用
  `ClientBuilder::no_proxy()`；
- OpenAI、Gemini、Anthropic 统一复用该 helper；远程 Provider 继续保留 system/configured
  proxy 行为；
- 未引入全局 Mutex，未把测试改为串行，也未全局禁用代理。

验证结果：

```text
local proxy bypass helper                                              PASS (1/1)
Settings tests --test-threads=32                                       PASS (50/50)
Settings concurrent stress after fix                                  PASS (100/100)
cargo fmt --check                                                      PASS
cargo check --locked                                                   PASS
cargo clippy correctness+suspicious                                    PASS
cargo test --locked                                                    PASS (1585/1585)
git diff --check                                                       PASS
```

验证 artifact：

```text
validation/settings-http-mock-stress-before-fix.log
validation/settings-http-mock-stress-before-fix-summary.log
validation/settings-http-mock-diagnostic-stress.log
validation/settings-http-mock-diagnostic-stress-summary.log
validation/settings-http-mock-endpoint-diagnostic-stress.log
validation/settings-http-mock-endpoint-diagnostic-stress-summary.log
validation/settings-http-mock-stress-after-local-proxy-bypass.log
validation/settings-http-mock-stress-after-local-proxy-bypass-summary.log
```

该修复属于 P0 质量门禁稳定化，并为 R0.2 本地真实 E2E、R0.3 GitHub Runner 证据提供确定性
transport 基线；它未修改数据库 Schema、revision catalog、initial schema、Python Alembic
source-map、migrator metadata 或 health 固定 revision，也没有取得真实 PostgreSQL/Runner 证据。
因此 R0.1、R0.2、R0.3 与 G0 状态保持未完成，正式路线顺序不变。

### R0.2/R0.3 Migration Executor Evidence Gate（2026-07-13）

问题审计确认，`migration-executor` 是 R0.2/R0.3 真实 Rust 链路的首个生产门禁，但原 workflow
只在 Runner 终端运行命令。migration 失败发生在 `e2e-diagnostics/` 创建之前，因此没有独立
stdout、stderr 和原始退出码 artifact；同时 CLI 默认 tracing writer 会把日志与 stdout JSON
混合，无法把执行报告稳定交给机器解析。

本次完成以下无 Schema evidence gate：

1. `backend-rs/src/main.rs` 将 `migration-executor` 与 `release-readiness-preflight` 统一登记为
   structured CLI output owner。两者 stdout 只保留单一 JSON report，tracing、配置和连接诊断写入
   stderr；smoke command 的既有行为保持不变。
2. `.github/workflows/e2e-smoke.yml` 在 migration step 开始即创建 `e2e-diagnostics/`，分别保存
   `migration-executor.json`、`migration-executor-stderr.log` 和
   `migration-executor-exit-code.txt`，同时向 Runner 终端回显两条 stream。
3. workflow 使用 `set +e` 捕获 executor 原始退出码，再恢复 `set -e`；非零退出码按原值传播，
   migration 失败后不会启动 preflight 或 Rust server，也没有使用 `|| true` 吞错。
4. `production_ci_contract_tests` 新增
   `e2e_smoke_preserves_migration_executor_evidence_and_structured_stdout`，固定 migration/preflight
   顺序、三文件名称、退出码传播、禁止吞错和 CLI structured output owner。

验证结果：

```text
production_ci_contract_tests                                            PASS (10/10)
Workflow YAML parse                                                     PASS
Git Bash bash -n                                                        PASS
SQLite in-memory isolated process probe stdout JSON                     PASS
SQLite probe process/report exit code                                   PASS (1 == 1, fail-closed expected)
cargo fmt --check                                                       PASS
cargo check --locked                                                    PASS
cargo clippy correctness+suspicious                                     PASS
cargo test --locked                                                     PASS (1586/1586)
```

SQLite 进程探针使用非 PostgreSQL URL，executor 按契约 fail-closed 返回 1；该结果不是产品失败，
而是用来证明 stdout 是单一可解析 JSON、stderr 独立承载 tracing，并且 process exit code 与 report
`exit_code` 一致。首次运行 10 项 production CI contract 时，旧 preflight 测试仍断言单 owner
`init_tracing` 表达式，结果为 9/10；更新为共享 structured CLI owner 契约后复跑 10/10。初次
PowerShell 组合探针的路径变量/重定向 harness 不稳定，改用显式进程启动后探针通过，未修改产品逻辑。

验证 artifact：

```text
validation/migration-executor-evidence-step.sh
validation/migration-executor-sqlite-stdout.json
validation/migration-executor-sqlite-stderr.log
validation/migration-executor-sqlite-exit-code.txt
```

本切片未修改 `password_hash` 字段类型、Rust revision catalog、initial schema、Python Alembic
source-map、migrator metadata 或 health 固定 revision head/count contract，也没有取得真实
PostgreSQL Auth E2E 或 GitHub Runner 成功证据。因此它只属于 R0.2/R0.3 的无 Schema 证据准备，
R0.1、R0.2、R0.3 与 G0 状态保持未完成，正式优化路线顺序不变。

### R0.3 GitHub Runner Success Evidence Gate（2026-07-13）

后续审计发现，migration、release preflight、`/readyz` 和 `/releasez` 虽然都已生成结构化文件，
但 `rust-readiness-diagnostics` 仍只在 `failure()` 时上传。这样失败 Runner 有诊断，成功 Runner
却只剩终端状态，无法形成 R0.3 所需的持久化、可下载、可绑定到具体 commit/run 的成功证据。

本次完成以下无 Schema success evidence gate：

1. Playwright 两项真实后端 smoke 成功后新增 `Record successful Rust E2E evidence` step；只有此前
   migration、release preflight、runtime `/readyz`、release `/releasez` 与 Playwright 全部成功时，
   才生成 `e2e-diagnostics/runner-success.json`。
2. success manifest 固定记录 schema version、Rust runtime owner、PostgreSQL database、五类通过
   状态，以及 `GITHUB_SHA`、`GITHUB_RUN_ID`、`GITHUB_RUN_ATTEMPT`，使 artifact 可定位到具体
   Runner 执行和提交。
3. 保留既有 `rust-readiness-diagnostics` artifact 名称，上传条件从 `failure()` 调整为 `always()`；
   因此成功时持久化完整证据，失败时继续上传已有诊断，而失败链路不会生成 success manifest。
4. 新增
   `e2e_smoke_persists_successful_runner_evidence_and_always_uploads_diagnostics` 契约测试，固定
   Playwright → success manifest → always-upload 顺序、manifest 字段和 artifact 兼容名称。

验证结果：

```text
new runner success contract                                           PASS (1/1)
production_ci_contract_tests                                          PASS (11/11)
Workflow YAML parse                                                   PASS
Git Bash bash -n                                                      PASS
Runner success shell execution + exact JSON parse                     PASS
cargo fmt --check                                                     PASS
cargo check --locked                                                  PASS
cargo clippy correctness+suspicious                                   PASS
cargo test --locked                                                   PASS (1587/1587)
git diff --check                                                      PASS
```

首次 shell 行为探针通过 `Get-Command bash` 命中了本机 WSL shim，但该环境没有 `/bin/bash`，因此
失败属于验证 harness 选择错误，不是 workflow 或产品失败。改用项目 Git for Windows
`E:/Code/SoftWare/Tools/Git/bin/bash.exe` 后，语法检查、真实 JSON 生成与精确解析全部通过。

验证 artifact：

```text
validation/runner-success-evidence-step.sh
validation/runner-success-evidence.json
validation/runner-success-cargo-fmt-check.log
validation/runner-success-cargo-check.log
validation/runner-success-cargo-clippy.log
validation/runner-success-contract-tests.log
validation/runner-success-cargo-test.log
validation/runner-success-diff-check.log
```

本切片只补齐“成功 Runner 如何留下可审计 artifact”的能力，并未实际触发或取得 GitHub Runner
成功证据，也未修改数据库 Schema、revision catalog、initial schema、Python Alembic source-map、
migrator metadata 或 health 固定 revision。因此 R0.1、R0.2、R0.3 与 G0 仍保持未完成，正式
路线顺序不变。

### R0.3 GitHub Runner Backend Process Lifecycle Evidence Gate（2026-07-13）

审计发现原 workflow 使用 `nohup cargo run --locked &` 并保存 `$!`，该 PID 可能属于 Cargo
wrapper；cleanup 仅 kill wrapper，无法证明最终 Rust server 已退出。同时 diagnostics artifact 在
cleanup 前上传，任何清理结果都不会进入持久化证据。

本切片完成：

1. `Start Rust backend` 先执行 `cargo build --locked`，再直接启动
   `./target/debug/mumu-novel-backend`；`/tmp` 与 diagnostics 保存同一个 server PID。
2. cleanup 移到 success manifest 和 artifact upload 之前，保持 `if: always()`。
3. cleanup 生成 `rust-backend-lifecycle.json`，区分 `not_started`、`already_exited`、
   `terminated` 与 `forced_kill`；PID 提前退出会保留诊断并返回非零。
4. 正常路径发送 TERM 并轮询十秒；超时后 KILL、写入结构化证据并返回非零，禁止生成
   `runner-success.json`。
5. `runner-success.json` 新增 `backend_lifecycle=passed`，仅在清理门禁成功后创建。
6. 扩展既有 production CI 防漂移契约，禁止回退到 `nohup cargo run --locked` 或
   cleanup-after-upload 顺序。

定向验证：

```text
cargo fmt -- --check                                      PASS
production_ci_contract_tests                              PASS (11/11)
Workflow YAML parse                                       PASS
Git Bash bash -n                                          PASS
TERM lifecycle probe                                      PASS (terminated/TERM, exit 0)
already-exited lifecycle probe                            PASS (already_exited, exit 1)
forced KILL lifecycle probe                               PASS (forced_kill/KILL, exit 1)
lifecycle JSON exact parse                                PASS
```

首次行为探针因 PowerShell 到 `bash -lc` 的双层变量插值吞掉 `$!` 而失败；改用独立 UTF-8
Git Bash 脚本后两条真实信号分支均按合同通过。该 harness 问题不属于 workflow 或产品失败。

本切片仍只是 R0.3 的无 Schema 证据能力准备，尚未实际取得 GitHub Runner 绿色执行；也未
修改数据库 Schema、revision catalog、initial schema、Python Alembic source-map、migrator
metadata 或 health 固定 revision。因此 R0.1、R0.2、R0.3 与 G0 状态保持不变。

最终质量门禁：

```text
cargo fmt -- --check                                      PASS
cargo check --locked                                      PASS
cargo clippy correctness+suspicious                       PASS
production_ci_contract_tests                              PASS (11/11)
cargo test --locked                                       PASS (1587/1587)
frontend npm run build                                    PASS
Workflow YAML parse                                       PASS
Git Bash bash -n                                          PASS
TERM / already-exited / forced-KILL probes                PASS
runner-success + lifecycle JSON exact parse               PASS
UTF-8 no BOM / LF                                         PASS
targeted git diff --check                                 PASS
full git diff --check                                     PASS (exit 0)
```

Clippy 仍报告既有非 correctness/suspicious 技术债，未新增 crate-wide allow。全工作树 diff check
仅输出用户既有前端 CRLF→LF 提示，退出码为 0；frontend build 未产生额外 tracked static 变更。

验证 artifact：

```text
validation/backend-lifecycle-steps.sh
validation/backend-lifecycle-terminated.json
validation/backend-lifecycle-already-exited.json
validation/backend-lifecycle-forced-kill.json
validation/runner-success-lifecycle.json
validation/backend-lifecycle-cargo-fmt-check.log
validation/backend-lifecycle-cargo-check.log
validation/backend-lifecycle-cargo-clippy.log
validation/backend-lifecycle-contract-tests.log
validation/backend-lifecycle-cargo-test.log
validation/backend-lifecycle-frontend-build.log
validation/backend-lifecycle-final-static-validation.log
```

## R0.2 Local Real E2E Completion（2026-07-13）

### Reproducible Harness

新增：

```text
validation/run-local-r02-real-e2e.ps1
```

固定执行链：

```text
PostgreSQL 18-alpine
-> Rust migration-executor
-> release-readiness-preflight
-> cargo build --locked
-> Rust server
-> /readyz
-> /releasez
-> auth.spec.ts + background-task-pages.spec.ts
-> Rust lifecycle cleanup
-> PostgreSQL cleanup
-> local-r02-success.json
```

脚本使用唯一容器名、可配置数据库/应用端口和独立 evidence 目录；`finally` 负责清理 Rust
进程与 PostgreSQL 容器。成功清单只在 cleanup 完成后生成。Windows 本地 lifecycle 如实记录
`TerminateProcess`，不宣称 POSIX `TERM`。

### Failure Rounds and Fixes

1. `r02-local-real-e2e-20260713`：4/14。React StrictMode 重放 effect 后，
   `AuthCallback` 的 mounted guard 未在第二次 setup 恢复为 `true`；后台任务 helper 还依赖已经
   从 `AppRouter.tsx` 删除的 Sponsor 路由。
2. `r02-local-real-e2e-20260713-rerun1`：13/14。首次 OAuth 登录仍断言旧文案“当前账号：”。
3. `r02-local-real-e2e-20260713-rerun2`：端口 `8005` 被未知进程占用，harness 在数据库启动前
   失败关闭，没有终止未知进程。
4. `r02-local-real-e2e-20260713-rerun3`：13/14。用户名多处出现，非精确 locator 触发
   Playwright strict-mode violation。
5. `r02-local-real-e2e-20260713-final`：14/14，但 success manifest 早于 cleanup，证据顺序继续加固。
6. `r02-local-real-e2e-20260713-final2`：14/14，cleanup 后写 success，作为最终权威证据。

实施修复：

- `frontend/src/pages/AuthCallback.tsx`：每次 effect setup 设置 `mountedRef.current = true`；真实
  unmount 继续设为 `false` 并推进 request id，拒绝迟到响应。
- `frontend/e2e/helpers/backgroundTaskSmoke.ts`：直接进入当前注册的项目子路由，不再借用已删除的
  `/project/:projectId/sponsor` 导航跳板。
- `frontend/e2e/auth.spec.ts`：按当前可访问文本使用精确 locator，避免旧标点和多匹配漂移。

### Authoritative Evidence

权威目录：

```text
validation/r02-local-real-e2e-20260713-final2
```

结果：

```text
PostgreSQL image                     postgres:18-alpine
migration revisions                 20
migration SQL steps                 120
final revision                      20260712_password_hash_phc_text
release preflight                   PASS
/readyz                             HTTP 200 / ready
/releasez                           HTTP 200 / ready
Playwright                          14/14 PASS
Rust lifecycle                      terminated / TerminateProcess
leftover ports/containers           none
```

关键文件：

```text
local-r02-success.json
migration-executor.json
migration-executor-stderr.log
release-preflight.json
release-preflight-stderr.log
readyz.json
readyz-http-status.txt
releasez.json
releasez-http-status.txt
playwright-smoke.log
rust-backend.log
rust-backend-lifecycle.json
postgres.log
toolchain.json
```

### Final Quality Gates

```text
frontend npm run lint                         PASS
frontend npm run build                        PASS
cargo fmt -- --check                          PASS
cargo check --locked                          PASS
production_ci_contract_tests                  PASS (15/15)
cargo test --locked                           PASS (1612/1612)
workflow YAML parse                           PASS
UTF-8 no BOM / LF                             PASS
exact evidence validation                     PASS
git diff --check                              PASS (exit 0)
```

首次组合证据校验错误假设 migration JSON 顶层存在 `ok`，且 `Tee-Object` 未传播 Python 失败码；
该输出已作废。最终校验按真实 schema 检查 `exit_code`、20 个 revision、120 个 SQL step、最终 head、
14/14、readiness 与 cleanup，并显式传播 `$LASTEXITCODE`。可信日志结果为
`evidence=PASS`。

### Gate Decision

```text
R0.2 = PASS
R0.3 = READY TO EXECUTE
G0   = NO-GO
```

本地 Node `22.22.0`、Rust `1.92.0` 与 CI 的 Node 20、Rust 1.88 不同，因此 R0.2 不替代 R0.3。
下一步只采集实际 GitHub Runner 绿色运行、binary identity/SHA-256、lifecycle、success manifest
和失败诊断 artifact；R0.3 通过前不得审查通过 G0，也不得进入 R3。

## R0.3 Local Runner Contract Completion (2026-07-13)

### Scope

This slice completes every locally implementable R0.3 evidence contract without claiming a GitHub Runner pass.
The single workflow owner remains `.github/workflows/e2e-smoke.yml`; no parallel workflow or evidence owner was
introduced.

### Implemented Contracts

1. The Rust server is built first, then started from the resolved binary path rather than a Cargo wrapper.
2. Startup records the absolute binary path and SHA-256 and verifies both against Linux `/proc/<pid>/exe`.
3. Cleanup validates PID syntax, requires expected identity evidence, re-checks `/proc/<pid>/exe`, and refuses
   `TERM`/`KILL` when the process path or hash does not match.
4. Lifecycle evidence records `identity_status`, `cleanup_status`, observed/expected path and SHA-256, PID, and
   termination signal.
5. Playwright output and exit code are retained in `e2e-diagnostics` for success and failure.
6. Success evidence now binds backend identity, binary path/hash, GitHub SHA, run ID, and attempt after cleanup.
7. Failure writes `runner-failure.json`; Rust diagnostics upload remains `always()` and Playwright report upload
   remains failure-only.
8. `production_ci_contract_tests` adds Linux identity, signal refusal, Playwright diagnostics, and failure manifest
   anti-drift coverage.

### Local Evidence

Authoritative local R0.3 contract directory:

```text
validation/r03-local-runner-contract-20260713
```

Linux container probes:

```text
workflow run-block bash syntax                    PASS
verified identity + TERM cleanup                  PASS (exit 0)
identity mismatch signal refusal                  PASS (exit 1, target left alive)
invalid PID signal refusal                        PASS (exit 1)
already-exited fail-closed lifecycle               PASS (exit 1)
```

Quality gates:

```text
workflow YAML parse                               PASS
cargo fmt -- --check                              PASS
cargo check --locked                              PASS
production_ci_contract_tests                      PASS (16/16)
cargo test --locked                               PASS (1613/1613)
cargo clippy CI baseline                          PASS
UTF-8 no BOM / LF / no trailing whitespace        PASS
git diff --check                                  PASS (exit 0)
```

### Read-Only GitHub Audit

The public GitHub API returned two historical `e2e-smoke` runs:

```text
25501938047  success  936e9401f3bcf4d28545d74e041248710e16acb8  artifacts=0
25442713518  success  960ee8c7f8eda571e5c70bddbd0e91f851ec7b38  artifacts=0
```

Neither run matches local HEAD `d0cff4f5fafeeb22f6b4c0f319e9d493ba7a8346`, and neither contains an artifact.
The current R0.3 changes are uncommitted local work, so historical green runs are not admissible evidence.

### Gate Decision

```text
R0.2 = PASS
R0.3 = LOCALLY COMPLETE / GITHUB RUNNER PENDING
G0   = NO-GO
```

The next admissible step is an actual GitHub-hosted Runner execution for the exact commit containing this
workflow contract, followed by download and exact validation of the diagnostics artifact. G0 and R3 remain
blocked until that evidence exists.

## R0.3 Clean-Checkout Closure Audit (2026-07-13)

### Why This Audit Was Required

The working tree contains changes from several active tasks. A green test run against the whole working tree does
not prove that the R0.3 submission is self-contained. This audit reconstructed each gate from base HEAD
`d0cff4f5fafeeb22f6b4c0f319e9d493ba7a8346` plus an explicit candidate overlay and then ran the production CI
commands in the declared Node 20, Python 3.11, and Rust 1.88 environments.

Machine-readable closure evidence:

```text
validation/r03-clean-checkout-closure.json
```

### Rust Closure: 24 Files to 26 Files

The initial 24-file candidate passed `production_ci_contract_tests` (16/16) but failed the complete test suite with
1601 passing and 11 failing tests. Two failures were real missing chapter-generation dependencies, so the final
hosted-runner candidate adds:

```text
backend-rs/src/services/chapter_generation_runtime_service.rs
backend-rs/src/services/chapter_generation_execution_contract_service.rs
```

Settings/OpenAI client changes were deliberately excluded. Their failures were caused by the application Docker
runtime branch that checks `/.dockerenv` and uses `host.docker.internal`; GitHub-hosted Cargo jobs execute directly
on the Linux VM host. The final simulation therefore removed only the ephemeral validation container marker before
running the gates.

Final Rust V4 evidence:

```text
candidate files                                             26
SHA closure                                                 PASS
cargo fmt -- --check                                        PASS
production_ci_contract_tests                               PASS (16/16)
cargo check --locked                                        PASS
cargo test --locked                                         PASS (1612/1612)
cargo clippy correctness/suspicious baseline                PASS
```

Authoritative files:

```text
validation/r03-clean-candidate-runner-v4.json
validation/r03-synthetic-clean-rust-gates-runner-v4-retry.json
validation/r03-synthetic-clean-rust-gates-runner-v4-retry.log
```

The first V4 invocation used `bash -lc`, which reset the official image PATH before any Cargo gate ran. Its failure
JSON/log are retained. The passing retry uses `sh -c` and does not overwrite the failed attempt.

### Frontend Closure

The frontend candidate contains only `auth.spec.ts`, `backgroundTaskSmoke.ts`, and `AuthCallback.tsx`. Against
`HEAD:frontend`, Node `20-bookworm` completed:

```text
npm ci       PASS
npm run lint PASS (existing warnings, zero errors)
npm run build PASS
```

Authoritative files:

```text
validation/r03-clean-frontend-v1.json
validation/r03-synthetic-clean-frontend-gates-v1.json
validation/r03-synthetic-clean-frontend-gates-v1.log
```

### Python Migration/Support Closure

Installing the old full `backend/requirements.txt` on a cold cache exceeded the workflow's 20-minute timeout before
pytest started while downloading Torch and NVIDIA runtime packages. The migration/support job does not own that AI
runtime. It now reuses the existing narrow dependency boundary:

```text
python -m pip install -r requirements-migrator.txt -r requirements-test.txt
pytest tests/test_tools
```

Python 3.11 also rejected the nested double-quoted f-string in
`backend/tools/check_text_encoding_health.py`; using single quotes inside the expression restores Python 3.11
collection compatibility. The final clean candidate includes `HEAD:backend`, `HEAD:deploy`, three migration/support
overlays, and the tracked deploy probe manifest required by `tests/test_tools`.

```text
candidate overlay files                                    3
Python 3.11 dependency install                              PASS
pytest tests/test_tools                                     PASS (67/67)
```

Authoritative files:

```text
validation/r03-synthetic-clean-python-gates-v1-timeout.json
validation/r03-clean-python-v3.json
validation/r03-synthetic-clean-python-gates-v3.json
validation/r03-synthetic-clean-python-gates-v3.log
```

### Submission Boundary and Gate Decision

All `validation/*.tar` files are local reconstruction inputs and must not be staged or committed. Historical failure
logs remain evidence and must not be deleted. GitHub-hosted execution still requires an exact commit and downloaded
diagnostics artifact.

```text
R0.1 = PASS
R0.2 = PASS
R0.3 = LOCALLY COMPLETE / GITHUB RUNNER PENDING
G0   = NO-GO
R3   = BLOCKED BY G0
```

### Post-Write Quality Verification

```text
Python source compile                                     PASS
backend-ci YAML parse                                     PASS
cargo fmt -- --check                                      PASS
local production_ci_contract_tests                       PASS (16/16)
final evidence JSON parse                                 PASS
UTF-8 no BOM / LF / no trailing whitespace               PASS
targeted git diff --check                                 PASS
global git diff --check                                   PASS
workspace-local __pycache__ generated                     NO
```

The global diff check emitted existing line-ending conversion warnings for unrelated working-tree files, but it
returned exit code 0 and reported no whitespace error. No Git write operation was performed.
