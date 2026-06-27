# Backend 模块文档

[根目录](../CLAUDE.md) > **backend**

---

## 变更记录

### 2026-04-20
- 按当时 Python HTTP runtime 代码刷新模块文档
- 修正健康检查、测试状态、路由拆分与版本信息
- 补充后台任务、章节子路由与配置说明

### 2026-06-25
- 记录 Rust + Nginx runtime closeout 现状
- 更新 Alembic/db-migrator Python 边界到 `migrator_app`
- 标注 retired Python app package 已完成生产源码退出

---

## 模块职责

`backend/` 当前只保留冻结的 Python migration source-map、Alembic 迁移脚本、
历史测试支撑与运维工具。长运行 REST API、SSE、认证、AI 编排、后台任务、
静态资源托管与 SPA 回退均由 Rust backend + Nginx runtime 承担。

---

## 真实入口与启动

### 入口文件
- Rust backend now owns the long-running runtime entrypoint.
- Python production code is no longer part of the deploy entry; only frozen
  migration source-map and test-support code remains.
- `migrator_app/models/__init__.py`：test/Alembic SQLAlchemy metadata `Base`
  与模型注册入口
- `migrator_app/models/`：test/Alembic target metadata only; production
  runtime behavior is Rust-owned

### 启动命令

```bash
docker compose up -d --build
```

### 生命周期关键步骤
1. `db-migrator` runs `migration-executor` from `/app/server`.
2. The Rust executor opens PostgreSQL, applies the migration executor shell,
   and records the replay report.
3. Rust backend starts after migrations complete.
4. Nginx routes runtime traffic to Rust.

---

## 目录地图

```text
backend/
├── migrator_app/
│   ├── models/
│   └── __init__.py
├── tests/
├── alembic/
├── tools/
├── scripts/
├── requirements-migrator.txt
├── pytest.ini
└── alembic-postgres.ini
```

---

## 路由结构

### 当前生产路由 owner
- Rust backend owns production HTTP / SSE routes under `backend-rs/src/api/`.
- The retired Python app package has been physically removed from the production Python source
  tree. Do not recreate Python route shells for runtime traffic.

### 已迁移并删除的历史 Python route shell
- `auth.py`、`users.py`、`admin.py`、`settings.py`
- `projects.py`、`wizard_stream.py`、`inspiration.py`、`background_tasks.py`、`book_import.py`

### 章节域路由
章节相关 Python route shell 已基本物理退出；测试适配与回归入口主要位于
`backend/tests/test_support/`：
- `chapter_regeneration_route_test_adapter.py`
- `chapter_analysis_route_test_adapter.py`
- `chapter_crud_route_test_adapter.py`
- `chapter_annotation_route_test_adapter.py`
- `chapter_quality_route_test_adapter.py`
- `chapter_expansion_plan_route_test_adapter.py`
- `chapter_route_helpers_test_support.py`

### 已迁移并删除的内容管理 route shell
- `outlines.py`、`characters.py`、`careers.py`、`relationships.py`
- `organizations.py`、`foreshadows.py`、`writing_styles.py`、`memories.py`
- `prompt_templates.py`、`prompt_workshop.py`、`mcp_plugins.py`、`changelog.py`

---

## 健康检查与静态托管

### 健康检查
- `GET /health`
- `GET /livez`
- `GET /readyz`
- `GET /health/db-sessions`

### 静态托管
当前端已构建到 Rust runtime 静态目录后：
- `/assets/*` 由 Rust backend / Nginx runtime 托管
- 非 `/api/*` 路径回退到 `index.html`

---

## 配置与环境变量

### 关键配置来源
- `backend/.env.example`
- 仓库根目录 `.env`
- `backend/.env`
- `backend/tests/test_support/retired_runtime_test_support.py` for test-only retired
  Python runtime config/logging compatibility
- `backend/tests/test_support/database_test_support.py` for the remaining
  Python database/session test-support boundary
- `backend/migrator_app/models/__init__.py` for Alembic model registration
  in the frozen migrator metadata package
- Rust runtime config under `backend-rs/src/config.rs`

### 关键配置项
- `DATABASE_URL`，或 `POSTGRES_*` 组合生成默认 PostgreSQL DSN
- `DATABASE_POOL_SIZE=50`
- `DATABASE_MAX_OVERFLOW=30`
- `DATABASE_POOL_TIMEOUT=90`
- `DEFAULT_AI_PROVIDER`
- `DEFAULT_MODEL`
- `PRE_GENERATION_WEB_RESEARCH_*`
- `LOCAL_AUTH_*`
- `LINUXDO_*`
- `WORKSHOP_*`

---

## 关键依赖

- Python source-map/test-support：`sqlalchemy`, `asyncpg`,
  `psycopg2-binary`, `alembic`, `pydantic-settings`, `python-dotenv`
- Legacy/manual SQLite tooling may still use `aiosqlite` outside the production
  migrator image.
- Rust runtime：see `backend-rs/Cargo.toml`
- Test support：historical Python adapters live under `backend/tests/`

---

## 测试与质量

### 当前状态
- 已有 `pytest.ini`
- API 测试集中在 `tests/test_api/`
- 服务测试集中在 `tests/test_services/`
- Schema 测试集中在 `tests/test_schemas/`
- 当前活跃回归覆盖包含章节、设置、灵感、后台任务、提示词质量等核心场景

### 常用命令

```bash
cd backend
pytest
pytest tests/test_api/test_settings.py
pytest tests/test_api/test_background_tasks.py
pytest tests/test_services/test_chapter_prompt_quality_compat_service.py
```

---

## 开发约定

- 路由层保持薄；复杂逻辑尽量下沉到 `services/`
- 涉及用户数据的查询必须显式考虑 `user_id` 隔离
- 生成、重写、分析、后台任务恢复共享状态模型，修改 schema 或 service 返回值时需同步所有调用点
- 兼容层文件 `*_compat_service.py` 用于渐进重构，改动前先确认新旧调用链是否仍共存
- `backend/alembic/postgres/versions/`
  只作为 frozen source-map / historical input 保留，不再是 production deploy entry。
- `backend/scripts/migrate.py` 已删除；手动 Alembic revision 维护直接使用
  `alembic -c alembic-postgres.ini revision ...`。
- 健康检查与启动流程依赖数据库 warmup，本地调试不要仅看 `/health`

---

## 风险与注意事项

- 章节域服务数量多且命名相近，容易误改到兼容层而不是主入口
- 静态资源托管与 API 在同一进程中，前端构建失败会影响本地整体验证
- `readyz` 会在数据库未就绪时返回 503，这属于预期行为
- 后台任务会持久化到运行时文件，测试时需注意 `.runtime` 临时数据影响

---

## 下一步推荐阅读

1. `app/config.py`
2. `tests/test_support/database_test_support.py`
3. `app/models/__init__.py`
4. `backend/alembic/README`
5. 当前任务相关的 `chapter_*` / `batch_generation_*` Rust owner 或测试适配服务

---

**最后更新**: 2026-04-20
**模块版本**: 1.3.9

