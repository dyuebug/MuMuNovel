# PRD：Rust 生产 CI 与真实 Rust E2E 对齐

## 目标

让 MuMuNovel 的持续集成门禁与当前生产所有权一致：Rust 是唯一生产运行时，Rust migration executor 是生产数据库迁移入口，Playwright smoke 必须连接真实 Rust server。Python 后端测试继续保留为迁移兼容和支持性回归，但不再被命名或描述为生产后端门禁。

## 用户价值

- Rust 代码变更在合并前获得格式、编译、测试和 lint 门禁。
- E2E 能发现“前端在 Python 可用、Rust 生产不可用”的真实回归。
- CI 数据库准备链与生产部署保持一致，减少 SQLite/Python 专用路径漂移。
- Rust 与 Python 的剩余所有权边界在 workflow 中可直接识别。

## 已确认事实

1. `deploy/nginx/mumunovel.conf` 和 `docker-compose.yml` 均声明 Rust 是唯一运行时所有者。
2. `.github/workflows/backend-ci.yml` 只在 `backend/**` 变化时运行 Python pytest，没有覆盖 `backend-rs/**`。
3. `.github/workflows/e2e-smoke.yml` 当前启动 Python Uvicorn，并引用仓库中不存在的 `backend/alembic-sqlite.ini`。
4. `backend-rs/src/main.rs` 提供可执行的 `migration-executor` 命令。
5. Rust migration contract 声明 `production_migration_mode=explicit_rust_db_migrator_before_rust_startup`，并覆盖 PostgreSQL 初始 schema 和迁移尾部。
6. Playwright 通过 Vite proxy 使用 `APP_PORT` 连接后端，现有 smoke 测试可复用 `8003`。

## 功能要求

1. 后端 CI 必须在 `backend-rs/**`、Rust workflow 或 Rust manifest 变化时触发。
2. Rust CI 必须依次执行：
   - `cargo fmt -- --check`；
   - `cargo check --locked`；
   - `cargo test --locked`；
   - `cargo clippy --locked --all-targets -- -D warnings`，或在既有 warning 无法一次清零时使用明确、可追踪的临时基线策略。
3. Python pytest job 继续存在，但 workflow/job 命名必须表明它是 Python migration/support regression。
4. E2E workflow 必须使用 PostgreSQL service，而不是 SQLite 专用路径。
5. E2E 必须先运行 Rust `migration-executor`，成功后再启动 Rust server。
6. Playwright 的真实后端 smoke 必须连接 Rust server，覆盖本地认证和后台任务页面请求链。
7. Rust server 未就绪、迁移失败或 smoke 失败时，workflow 必须输出 Rust 日志和可诊断信息。
8. 所有 YAML 和新增文本必须为 UTF-8 无 BOM。

## 验收标准

- [ ] 修改 `backend-rs/**` 会触发 Rust CI 和 E2E smoke。
- [ ] Rust fmt、check、test 或 Clippy 门禁失败会使 workflow 失败。
- [ ] Python pytest job 的名称和触发范围明确为 support/migration regression。
- [ ] `e2e-smoke.yml` 不再包含 `uvicorn` 或 Python runtime 启动命令。
- [ ] `e2e-smoke.yml` 不再引用不存在的 SQLite Alembic 配置。
- [ ] E2E 顺序为 PostgreSQL ready → Rust migration executor → Rust server ready → Playwright。
- [ ] Playwright 仍运行 `auth.spec.ts` 与 `background-task-pages.spec.ts`。
- [ ] workflow 保留失败时的 Rust backend 日志输出和进程清理。
- [ ] 本地 Rust fmt、check、test 以及前端构建通过。
- [ ] workflow YAML 能被解析，`git diff --check` 通过。

## 非目标

- 不删除 Python 后端或 Python pytest。
- 不在本任务中修改业务 API、数据库模型或 migration catalog。
- 不在本任务中实现后台任务快照原子化和恢复策略。
- 不扩展 Playwright 为完整回归套件。
- 不引入新的第三方 CI 平台或自建 runner。

## 兼容性约束

- 保留现有分支 `main`、`dev` 的 push 门禁。
- 保留前端本地 Playwright 的默认启动方式。
- 不改变生产端口约定；E2E 可继续使用隔离端口 `8003`。
- CI 中的数据库凭据只能使用 runner 内临时测试值。

## 开放问题

无阻塞问题。Clippy 若暴露既有 warning，应优先修复 Rust warning；仅当数量或范围明显超出 R0 时，才采用有注释的临时基线，并单独记录后续清理任务。

## R0.1 补充需求：PostgreSQL Auth Schema Compatibility

### 触发原因

真实 PostgreSQL 18 + Rust migration + Rust server 链路已证明，Rust 本地认证使用的 Argon2
PHC 字符串无法写入历史 `user_passwords.password_hash VARCHAR(64)` 字段。该错误发生在
`AuthService::ensure_local_admin()` 创建本地管理员时，并以 HTTP 500 暴露到登录接口。

### 功能要求

1. `password_hash` 必须作为不透明 password verifier 存储，不再绑定 SHA-256 的 64 字符形状。
2. 新数据库通过 Rust initial schema 直接获得目标字段类型；已有数据库通过新 Rust catalog
   revision 从当前 head 升级，禁止改写已有 revision 的身份或历史 SQL。
3. Rust migration executor 继续是唯一生产迁移 owner；Python Alembic revision 和
   `migrator_app` model 只作为冻结 source-map/metadata 契约同步，不恢复 Python runtime owner。
4. 现存 64 字符 legacy SHA-256 值必须原样保留，并继续由 Rust 登录流程验证后升级为 Argon2。
5. 新建管理员、修改密码、管理员创建用户和 legacy hash 自动升级四条写入路径必须共享同一
   可存储契约，不允许只修复 `ensure_local_admin()`。
6. migration upgrade 必须可重复判定；migration downgrade 不得截断 Argon2 数据。当存在长度
   大于 64 的值时，降级必须显式失败并保留数据。
7. 不增加 startup schema sync，不在 HTTP 请求内执行 DDL，不回退到 Python/Uvicorn E2E。

### 验收标准

- Rust revision catalog 为单链 20 项，新 head 与 executable catalog、health contract 一致。
- 空 PostgreSQL 数据库执行 migration 后，`user_passwords.password_hash` 为 `TEXT NOT NULL`。
- 从旧 head 且包含 legacy SHA-256 数据的数据库升级后，原值不变且可成功登录。
- 本地管理员首次创建成功，落库值为可解析 Argon2 PHC 字符串且长度大于 64。
- legacy SHA-256 登录成功后自动升级为 Argon2，升级后的值可完整落库。
- downgrade 在存在大于 64 字符的 verifier 时明确失败，不发生截断或静默数据损坏。
- `auth.spec.ts`、`background-task-pages.spec.ts` 和 GitHub runner 真实 E2E 全部通过后，R0
  才能完成并进入 G0。
