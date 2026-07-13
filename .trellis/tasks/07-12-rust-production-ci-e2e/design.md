# Design：Rust 生产 CI 与真实 Rust E2E 对齐

## 架构边界

R0 只调整 GitHub Actions 的所有权和启动链，不改变生产代码的 API 行为。

```text
backend-rs change
  ├─ rust-production job
  │    ├─ rustfmt
  │    ├─ cargo check
  │    ├─ cargo test
  │    └─ cargo clippy
  └─ rust-e2e-smoke job
       ├─ PostgreSQL service
       ├─ cargo run -- migration-executor
       ├─ cargo run (Rust server :8003)
       ├─ Vite dev server :5175 -> proxy :8003
       └─ Playwright smoke

backend change
  └─ python-migration-support job
       └─ pytest
```

## Workflow 所有权

### `.github/workflows/backend-ci.yml`

保留一个 workflow，但拆成语义明确的两个 job：

- `rust-production`：生产后端门禁；默认工作目录 `backend-rs`。
- `python-migration-support`：Python 兼容、迁移和测试支撑回归；默认工作目录 `backend`。

workflow 的 path filter 同时覆盖：

```text
backend-rs/**
backend/**
Cargo.toml / Cargo.lock 对应路径
.github/workflows/backend-ci.yml
```

Rust job 使用固定 toolchain，与 `backend-rs/Dockerfile` 的 Rust 1.88 保持一致，并启用 `rustfmt`、`clippy` components 和 Cargo cache。

## E2E 数据流

### PostgreSQL

使用 GitHub Actions service container：

```text
image: postgres:18-alpine
POSTGRES_DB: mumu_e2e
POSTGRES_USER: mumu_e2e
POSTGRES_PASSWORD: mumu_e2e
health: pg_isready
```

该配置与生产 PostgreSQL profile 一致，但不挂载生产数据或执行外部请求。

### Rust migration executor

在 server 启动前运行：

```bash
cargo run --locked --manifest-path backend-rs/Cargo.toml -- migration-executor
```

环境契约：

```text
DEBUG=false
DATABASE_URL=postgresql://mumu_e2e:mumu_e2e@127.0.0.1:5432/mumu_e2e
JWT_SECRET=<CI-only stable secret>
ENABLE_STARTUP_SCHEMA_SYNC=false
```

命令非零退出立即阻止 E2E，确保 schema 已到 Rust catalog head。

### Rust runtime

迁移成功后后台启动：

```bash
cd backend-rs
cargo build --locked
nohup ./target/debug/mumu-novel-backend
```

运行环境补充：

```text
APP_HOST=127.0.0.1
APP_PORT=8003
LOCAL_AUTH_ENABLED=1
LOCAL_AUTH_USERNAME=admin
LOCAL_AUTH_PASSWORD=admin123
STATIC_DIR=../backend/static
```

Rust 日志写入 `e2e-diagnostics/rust-backend.log`。workflow 直接启动已构建的
`mumu-novel-backend`，并将同一个 server PID 写入 `/tmp/rust-backend.pid` 与
`e2e-diagnostics/rust-backend.pid`，禁止保存 `cargo run` wrapper PID。

### Frontend / Playwright

保持 `frontend/playwright.config.ts` 不变。Vite 读取 `APP_PORT=8003`，将 `/api` 代理到 Rust backend。Playwright 继续运行：

```text
e2e/auth.spec.ts
e2e/background-task-pages.spec.ts
```

## 失败与诊断契约

- PostgreSQL health 失败：service container 自身阻止 job。
- migration executor 失败：直接输出 migration JSON/log，server 不启动。
- Rust server 60 秒内未 ready：输出 `e2e-diagnostics/rust-backend.log` 后失败。
- Playwright 失败：保留 Playwright HTML report/artifact；always cleanup 输出 Rust 日志并终止直接 server PID。
- 清理 step 不依赖前置步骤成功，必须在 success manifest 和 diagnostics upload 之前运行。
- cleanup 将 `not_started`、`already_exited`、`terminated` 或 `forced_kill` 写入
  `rust-backend-lifecycle.json`；PID 已提前退出或 TERM 十秒后仍存活均令 job 失败，后者再 KILL。

## Clippy 策略

目标门禁为：

```bash
cargo clippy --locked --all-targets -- -D warnings
```

若当前代码存在既有 warning：

1. 先运行命令确认数量和类型；
2. 对小范围、无行为影响的 warning 在 R0 内修复；
3. 若涉及大规模业务重构，不使用全局 `allow` 掩盖；改为记录明确的临时门禁范围和后续任务。

## 兼容与回滚

- Python pytest 不删除，发生 Rust CI 误判时可单独修复 Rust job，不影响 support job。
- E2E 回滚点仅是 workflow 启动链；生产 Compose 不修改。
- 如 Rust migration executor 在 GitHub PostgreSQL 环境失败，应修复 executor/环境契约，不回退到不存在的 SQLite 配置。

## R0.1 设计补充：Password Verifier 存储契约

### 1. Scope / Trigger

触发点是 Rust Argon2 PHC 字符串写入 PostgreSQL `user_passwords.password_hash VARCHAR(64)`
失败。R0.1 只修复 password verifier 的持久化契约及其 migration/source-map 一致性，不改变
认证 API、JWT、用户名规则或后台任务 API。

目标字段类型确定为 PostgreSQL `TEXT NOT NULL`。理由是该字段存储由密码算法产生的不透明
PHC verifier，长度属于算法和参数的实现细节；再次选择 `VARCHAR(128/255)` 只会把当前错误
替换成新的猜测上限。

### 2. Signatures

拟新增 Rust/PostgreSQL revision：

```text
revision: 20260712_password_hash_phc_text
down_revision: 20260517_project_core_defaults
filename: 20260712_1200_password_hash_phc_text.py
```

升级 SQL：

```sql
ALTER TABLE user_passwords
ALTER COLUMN password_hash TYPE TEXT;

COMMENT ON COLUMN user_passwords.password_hash
IS '密码校验值（Argon2 PHC 或兼容的 legacy SHA256）';
```

Rust initial schema 的新库契约：

```sql
password_hash TEXT NOT NULL
```

Rust SeaORM `user_password::Model.password_hash: String` 无需改变；Python migrator metadata
模型从 `String(64)` 同步为 `Text`。

### 3. Contracts

- Rust `migration-executor` 是唯一生产 DDL owner。
- `POSTGRES_ALEMBIC_HEAD`、`POSTGRES_REVISION_CATALOG` 和
  `RUST_EXECUTABLE_POSTGRES_REVISIONS` 必须同时增加同一 revision。
- catalog revision 数从 19 变为 20；health API 中所有固定 head/count 断言同步更新。
- initial schema 面向空库，新增 revision 面向已经位于旧 head 的数据库，两条路径最终 schema
  必须一致。
- 历史 Python initial revision 不改写；新增 Python revision 文件作为单链 source-map，
  `migrator_app/models/user.py` 同步 metadata 类型和注释。
- legacy 64 位 SHA-256 verifier 原样兼容；Rust 认证成功后继续调用现有
  `upgrade_legacy_password_hash()` 写入 Argon2。
- 不对 verifier 增加数据库默认值、截断、重编码或业务最大长度校验。

### 4. Validation & Error Matrix

| 场景 | 预期结果 |
|---|---|
| 空库执行全部 Rust migrations | 字段为 `TEXT NOT NULL`，head 为新 revision |
| 旧 head + 无密码记录 | 升级成功，字段变为 `TEXT` |
| 旧 head + 64 位 SHA-256 | 升级成功，值逐字节保持一致 |
| 首次本地管理员创建 | Argon2 PHC 完整写入，登录成功 |
| legacy 用户首次登录 | SHA-256 验证成功并原子更新为 Argon2 PHC |
| 已有 Argon2 用户登录 | 直接验证成功，不重复 rehash |
| downgrade + 所有值长度 ≤64 | 允许恢复为 `VARCHAR(64)` |
| downgrade + 任一值长度 >64 | migration 明确失败，数据和当前 schema 保持不变 |
| catalog/head/source-map 任一不一致 | revision health/contract 测试失败 |

### 5. Good / Base / Bad Cases

- Good：新库和旧库升级后都使用 `TEXT`，首次管理员登录及 legacy 自动升级均通过真实 PostgreSQL。
- Base：数据库中只有旧 SHA-256 值，迁移仅改变列类型，不主动重写凭据。
- Bad：只把 Rust initial schema 改成 `VARCHAR(255)`，已有数据库仍停留在 64 字符。
- Bad：直接编辑历史初始 Alembic revision，导致已部署数据库没有新 revision 可执行。
- Bad：downgrade 使用截断、substring 或隐式 cast 强行恢复 64 字符。

### 6. Tests Required

1. metadata 单元测试断言 catalog/executable catalog 均为 20 项，head 为新 revision。
2. revision chain 测试断言新 revision 唯一且 `down_revision` 指向旧 head。
3. initial schema 契约测试断言 `password_hash TEXT NOT NULL`，且不再存在
   `password_hash VARCHAR(64)` 或仅描述 SHA-256 的注释。
4. auth 单元测试断言 Rust 生成的是可解析 Argon2 PHC verifier，并显式覆盖长度大于 64。
5. 真实 PostgreSQL 回归覆盖空库 bootstrap、旧 head 升级、legacy 值保留、首次管理员创建和
   legacy 自动升级。
6. downgrade guard 测试断言长 verifier 存在时 migration 失败且数据未改变。
7. 运行 revision health、Rust fmt/check/test/Clippy、Python source-map tests、前端 build/lint
   和 Playwright auth/background-task smoke。

### 7. Wrong vs Correct

Wrong — 继续为算法输出猜测固定长度，或只修复新库：

```sql
password_hash VARCHAR(255) NOT NULL
```

Correct — 将 verifier 视为不透明文本，并同时覆盖新库和已有库：

```sql
ALTER TABLE user_passwords
ALTER COLUMN password_hash TYPE TEXT;
```

**降级边界**：如果数据库已经存有 Argon2 verifier，应用代码回滚仍可继续读取 `TEXT`；不应为了
代码回滚强制执行 schema downgrade。只有确认所有值均不超过 64 字符时，才允许显式降级列类型。
