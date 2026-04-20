# MuMuNovel - AI 智能小说创作助手

> 面向长篇小说创作的 AI 协作平台，覆盖项目创建、世界观/角色/大纲生成、章节写作、灵感恢复、提示词工坊与多模型接入。

## 变更记录

### 2026-04-20
- 刷新根级项目上下文文档
- 修正文档中已过时的测试、版本、入口与运行说明
- 对齐当前前后端入口、健康检查与 E2E 状态

---

## 项目目的

MuMuNovel 通过前后端分离架构，把小说创作工作流拆成可逐步完成的 AI 能力：
- 项目创建与智能向导
- 世界观、角色、职业体系与大纲生成
- 章节写作、重写、分析、批量生成与后台任务恢复
- 灵感模式与联网研究透传
- 伏笔、写作风格、记忆与提示词工坊
- 多模型接入：OpenAI、Anthropic、Gemini、MCP 工具链

---

## 仓库地图

```text
MuMuNovel/
├── backend/                 # FastAPI 后端、数据库、AI 服务、后台任务
│   ├── app/
│   │   ├── main.py          # 应用入口、路由注册、健康检查、SPA 回退
│   │   ├── config.py        # 配置与环境变量
│   │   ├── database.py      # 数据库与会话管理
│   │   ├── api/             # REST/SSE 路由
│   │   ├── services/        # 业务服务与 AI 编排
│   │   ├── models/          # SQLAlchemy 模型
│   │   ├── schemas/         # Pydantic schema
│   │   ├── middleware/      # 认证与 request id 中间件
│   │   └── mcp/             # MCP 集成
│   ├── tests/               # pytest API / service / schema 测试
│   ├── alembic/             # PostgreSQL / SQLite 迁移
│   └── CLAUDE.md
├── frontend/                # React + TypeScript 前端
│   ├── src/
│   │   ├── main.tsx         # React 入口 + ThemeProvider
│   │   ├── App.tsx          # 路由树、懒加载、全局后台任务中心
│   │   ├── pages/           # 页面
│   │   ├── components/      # 通用组件
│   │   ├── services/        # API/SSE 客户端
│   │   ├── theme/           # 主题模式与配置
│   │   └── routes/          # 懒加载辅助
│   ├── e2e/                 # Playwright 用例
│   ├── playwright.config.ts # E2E 配置
│   └── CLAUDE.md
├── .github/workflows/       # CI / 镜像构建工作流
└── CLAUDE.md                # 根级协作上下文
```

---

## 模块索引

| 模块 | 语言 | 职责 | 主入口 | 文档 |
|---|---|---|---|---|
| `backend/` | Python | FastAPI API、SSE、数据库、AI 服务、后台任务 | `backend/app/main.py` | `backend/CLAUDE.md` |
| `frontend/` | TypeScript | React UI、路由、主题、SSE 消费、E2E | `frontend/src/main.tsx` | `frontend/CLAUDE.md` |

---

## 运行与开发

### 本地后端

```bash
cd backend
python -m venv .venv
source .venv/bin/activate
pip install -r requirements.txt
cp .env.example .env
python -m uvicorn app.main:app --host 127.0.0.1 --port 8000 --reload
```

### 本地前端

```bash
cd frontend
npm install
npm run dev
```

### 常用命令

```bash
# backend
cd backend
pytest
alembic -c alembic-postgres.ini upgrade head
alembic -c alembic-sqlite.ini upgrade head

# frontend
cd frontend
npm run build
npm run build:analyze
npm run lint
npm run e2e
npm run e2e:auth
```

### 健康检查

- `GET /health` - 兼容健康检查
- `GET /livez` - liveness probe
- `GET /readyz` - 启动阶段 + 数据库 readiness
- `GET /health/db-sessions` - 数据库会话统计

---

## 当前测试状态

### 已有
- 后端：`pytest`，覆盖 `tests/test_api`、`tests/test_services`、`tests/test_schemas`
- 前端：Playwright E2E，覆盖登录、后台任务页、章节/大纲相关流程、灵感恢复、联网研究透传
- 构建检查：`frontend` 已有 `lint`、`build`、`build:analyze`

### 仍缺失
- 前端组件级单元测试
- 统一的仓库根级测试编排命令
- 端到端的真实外部 AI 提供商回归

---

## 关键依赖与外部服务

### 后端
- `fastapi`, `uvicorn`
- `sqlalchemy`, `asyncpg`, `aiosqlite`, `alembic`
- `openai`, `anthropic`, `mcp`
- `chromadb`, `sentence-transformers`, `transformers`
- PostgreSQL / SQLite

### 前端
- `react`, `react-router-dom`, `antd`, `zustand`, `axios`
- `@xyflow/react`, `dagre` 用于关系图/图布局
- `@playwright/test` 用于 E2E
- `vite`, `typescript`, `eslint`

---

## 编码与协作约定

### 后端
- 优先在 `app/services/` 放业务逻辑，`app/api/` 只保留路由与编排
- 所有异步 I/O 使用 `async/await`
- 数据访问遵循用户隔离；涉及项目/章节/记忆数据必须检查 `user_id`
- 章节相关能力已拆分为多个 route/service 文件，改签名时必须追踪所有调用链

### 前端
- 页面走 `src/pages/`，可复用 UI 放 `src/components/`
- 路由由 `src/App.tsx` 统一维护，页面默认懒加载
- API/SSE 客户端优先复用 `src/services/` 与 `src/utils/`
- 主题切换相关逻辑集中在 `src/theme/`

---

## 当前风险与注意事项

- 文档、页面文件与真实路由并不总是完全同步，改动前应以 `backend/app/main.py` 和 `frontend/src/App.tsx` 为准
- 后端章节能力拆分较细，生成/分析/重写/后台任务恢复互相耦合，改动前要追踪 route → service → model
- 前端构建产物默认输出到 `backend/static`，前后端联调时不要误删静态目录
- `readyz` 依赖数据库 warmup；本地数据库未启动时 API 可能表现为部分可访问、readiness 失败

---

## 推荐阅读顺序

1. `backend/CLAUDE.md`
2. `frontend/CLAUDE.md`
3. `backend/app/main.py`
4. `frontend/src/App.tsx`
5. 目标功能对应的 `api/`、`services/`、`pages/` 文件

---

## 相关资源

- 项目仓库：`https://github.com/dyuebug/MuMuNovel`
- Swagger UI：`http://localhost:8000/docs`
- ReDoc：`http://localhost:8000/redoc`
- Linux DO 讨论帖：`https://linux.do/t/topic/1106333`

---

**最后更新**: 2026-04-20
**文档版本**: 1.1.0
**项目版本**: 1.3.9
