# Backend 模块文档

[根目录](../CLAUDE.md) > **backend**

---

## 变更记录

### 2026-04-20
- 按当前 FastAPI 代码刷新模块文档
- 修正健康检查、测试状态、路由拆分与版本信息
- 补充后台任务、章节子路由与配置说明

---

## 模块职责

`backend/` 是 MuMuNovel 的服务端核心，负责：
- REST API 与 SSE 流式响应
- 用户认证、本地账户与 LinuxDO OAuth
- PostgreSQL / SQLite 数据存储与会话管理
- AI 模型调用与提示词编排
- 后台任务恢复、章节生成/分析/重写流程
- MCP 工具接入与插件状态同步
- 前端静态资源托管与 SPA 回退

---

## 真实入口与启动

### 入口文件
- `app/main.py`：FastAPI 应用入口、生命周期管理、路由注册、健康检查、静态资源挂载
- `app/config.py`：Pydantic Settings，加载仓库根目录与 `backend/.env`
- `app/database.py`：数据库连接、会话统计、健康检查

### 启动命令

```bash
cd backend
python -m uvicorn app.main:app --host 127.0.0.1 --port 8000 --reload
```

### 生命周期关键步骤
1. 注册 MCP 状态同步
2. 加载后台任务管理器
3. 执行数据库 warmup
4. 暴露 `/readyz` readiness 状态
5. 关闭时清理 MCP 客户端、AI HTTP 客户端与数据库连接

---

## 目录地图

```text
backend/
├── app/
│   ├── main.py
│   ├── config.py
│   ├── database.py
│   ├── logger.py
│   ├── api/
│   │   ├── auth.py
│   │   ├── settings.py
│   │   ├── projects.py
│   │   ├── inspiration.py
│   │   ├── background_tasks.py
│   │   ├── chapters.py
│   │   ├── chapter_*_routes.py
│   │   └── ...
│   ├── services/
│   │   ├── ai_service.py
│   │   ├── background_task_manager.py
│   │   ├── chapter_generation_*.py
│   │   ├── batch_generation_*.py
│   │   ├── *_compat_service.py
│   │   └── ...
│   ├── models/
│   ├── schemas/
│   ├── middleware/
│   └── mcp/
├── tests/
├── alembic/
├── requirements.txt
├── pytest.ini
├── alembic-postgres.ini
└── alembic-sqlite.ini
```

---

## 路由结构

### 基础路由
- `auth.py` - 登录、刷新、登出、当前用户
- `users.py` - 用户管理
- `settings.py` - 用户设置、模型配置、连接测试
- `admin.py` - 管理员接口
- `projects.py` - 项目 CRUD
- `wizard_stream.py` - 智能向导 SSE
- `inspiration.py` - 灵感模式与恢复
- `background_tasks.py` - 后台任务查询/恢复
- `book_import.py` - 拆书导入

### 章节域路由
章节能力已经拆分，不应再把 `chapters.py` 视作唯一入口：
- `chapter_crud_routes.py`
- `chapter_generation_routes.py`
- `chapter_batch_generation_routes.py`
- `chapter_regeneration_routes.py`
- `chapter_partial_regeneration_routes.py`
- `chapter_analysis_routes.py`
- `chapter_analysis_task_routes.py`
- `chapter_annotation_routes.py`
- `chapter_quality_routes.py`
- `chapter_draft_routes.py`
- `chapter_expansion_plan_routes.py`
- `chapters.py` / `chapter_route_helpers.py` 仍承担兼容与聚合职责

### 内容管理路由
- `outlines.py`
- `characters.py`
- `careers.py`
- `relationships.py`
- `organizations.py`
- `foreshadows.py`
- `writing_styles.py`
- `memories.py`
- `prompt_templates.py`
- `prompt_workshop.py`
- `mcp_plugins.py`
- `changelog.py`

---

## 健康检查与静态托管

### 健康检查
- `GET /health`
- `GET /livez`
- `GET /readyz`
- `GET /health/db-sessions`

### 静态托管
当前端已构建到 `backend/static` 后：
- `/assets/*` 由 FastAPI 直接托管
- 非 `/api/*` 路径回退到 `index.html`
- 若静态目录不存在，根路径返回 API 提示与构建说明

---

## 配置与环境变量

### 关键配置来源
- `backend/.env.example`
- 仓库根目录 `.env`
- `backend/.env`
- `app/config.py`

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

- Web：`fastapi`, `uvicorn`, `python-multipart`
- DB：`sqlalchemy`, `asyncpg`, `psycopg2-binary`, `aiosqlite`, `alembic`
- AI：`openai`, `anthropic`, `mcp`
- Infra：`httpx`, `python-dotenv`, `psutil`
- Memory：`chromadb`, `transformers`, `sentence-transformers`

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
- 健康检查与启动流程依赖数据库 warmup，本地调试不要仅看 `/health`

---

## 风险与注意事项

- 章节域服务数量多且命名相近，容易误改到兼容层而不是主入口
- 静态资源托管与 API 在同一进程中，前端构建失败会影响本地整体验证
- `readyz` 会在数据库未就绪时返回 503，这属于预期行为
- 后台任务会持久化到运行时文件，测试时需注意 `.runtime` 临时数据影响

---

## 下一步推荐阅读

1. `app/main.py`
2. `app/config.py`
3. `app/api/background_tasks.py`
4. `app/api/chapter_generation_routes.py`
5. `app/services/background_task_manager.py`
6. 当前任务相关的 `chapter_*` 或 `batch_generation_*` 服务

---

**最后更新**: 2026-04-20
**模块版本**: 1.3.9
