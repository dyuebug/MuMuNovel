# Backend 模块文档

[根目录](../CLAUDE.md) > **backend**

---

## 变更记录 (Changelog)

### 2026-02-21 23:14:25
- 初始化 backend 模块文档
- 完成模块结构扫描

---

## 模块职责

Backend 模块是 MuMuAINovel 的后端服务，基于 FastAPI 框架构建，提供 RESTful API 和 SSE 流式接口。负责处理所有业务逻辑、数据库操作、AI 服务调用、用户认证授权等核心功能。

**核心职责：**
- 提供 RESTful API 接口
- SSE 流式响应（AI 生成内容实时传输）
- 用户认证与授权（LinuxDO OAuth + 本地账户）
- 数据库操作与多用户数据隔离
- AI 服务集成（OpenAI、Gemini、Claude）
- MCP 插件系统管理
- 数据库迁移管理（Alembic）
- 向量数据库与长期记忆系统（ChromaDB）

---

## 入口与启动

### 主入口文件

**`app/main.py`** - FastAPI 应用主入口

```python
# 启动命令
python -m uvicorn app.main:app --host 0.0.0.0 --port 8000 --reload

# 或使用配置文件中的设置
python app/main.py
```

**应用生命周期：**
1. 启动时：注册 MCP 状态同步服务
2. 运行时：处理 HTTP 请求和 SSE 流
3. 关闭时：清理 MCP 插件、HTTP 客户端池、数据库连接

### 配置文件

**`app/config.py`** - 应用配置管理

- 使用 Pydantic Settings 管理配置
- 从 `.env` 文件加载环境变量
- 支持 PostgreSQL 和 SQLite 数据库
- 连接池优化配置（支持 150-200 并发用户）

**关键配置项：**
```python
# 应用配置
app_name: str = "MuMuAINovel"
app_version: str = "1.0.0"
debug: bool = True

# 数据库配置
database_url: str  # PostgreSQL 或 SQLite
database_pool_size: int = 50
database_max_overflow: int = 30

# AI 服务配置
openai_api_key: Optional[str]
default_ai_provider: str = "openai"
default_model: str = "gpt-4"

# 认证配置
LOCAL_AUTH_ENABLED: bool = True
LINUXDO_CLIENT_ID: Optional[str]
```

---

## 对外接口

### API 路由结构

所有 API 路由位于 `app/api/` 目录，按功能模块划分：

| 路由文件 | 前缀 | 功能 |
|---------|------|------|
| `auth.py` | `/api/auth` | 用户认证（登录、登出、会话管理） |
| `users.py` | `/api/users` | 用户管理 |
| `admin.py` | `/api/admin` | 管理员功能 |
| `settings.py` | `/api/settings` | 用户设置与 API 配置 |
| `projects.py` | `/api/projects` | 项目管理（CRUD、导入导出） |
| `wizard_stream.py` | `/api/wizard-stream` | 智能向导（SSE 流式生成） |
| `inspiration.py` | `/api/inspiration` | 灵感模式（创意生成） |
| `outlines.py` | `/api/outlines` | 大纲管理 |
| `characters.py` | `/api/characters` | 角色管理 |
| `careers.py` | `/api/careers` | 职业等级体系 |
| `chapters.py` | `/api/chapters` | 章节管理 |
| `relationships.py` | `/api/relationships` | 角色关系管理 |
| `organizations.py` | `/api/organizations` | 组织管理 |
| `foreshadows.py` | `/api/foreshadows` | 伏笔管理 |
| `writing_styles.py` | `/api/writing-styles` | 写作风格管理 |
| `prompt_templates.py` | `/api/prompt-templates` | 提示词模板 |
| `prompt_workshop.py` | `/api/prompt-workshop` | 提示词工坊 |
| `memories.py` | `/api/memories` | 长期记忆管理 |
| `mcp_plugins.py` | `/api/mcp` | MCP 插件管理 |
| `changelog.py` | `/api/changelog` | 更新日志 |

### 核心 API 端点

**认证相关：**
- `POST /api/auth/local/login` - 本地账户登录
- `GET /api/auth/linuxdo/url` - 获取 LinuxDO OAuth URL
- `GET /api/auth/user` - 获取当前用户信息
- `POST /api/auth/refresh` - 刷新会话
- `POST /api/auth/logout` - 登出

**项目管理：**
- `GET /api/projects` - 获取项目列表
- `POST /api/projects` - 创建项目
- `GET /api/projects/{id}` - 获取项目详情
- `PUT /api/projects/{id}` - 更新项目
- `DELETE /api/projects/{id}` - 删除项目
- `POST /api/projects/{id}/export-data` - 导出项目数据
- `POST /api/projects/import` - 导入项目

**智能向导（SSE 流式）：**
- `POST /api/wizard-stream/world-building` - 生成世界观（流式）
- `POST /api/wizard-stream/characters` - 生成角色（流式）
- `POST /api/wizard-stream/career-system` - 生成职业体系（流式）
- `POST /api/wizard-stream/outline` - 生成大纲（流式）

**章节管理：**
- `GET /api/chapters/project/{project_id}` - 获取项目章节列表
- `POST /api/chapters` - 创建章节
- `PUT /api/chapters/{id}` - 更新章节
- `POST /api/chapters/{id}/partial-regenerate-stream` - 局部重写（流式）

**伏笔管理：**
- `GET /api/foreshadows/projects/{project_id}` - 获取项目伏笔列表
- `GET /api/foreshadows/projects/{project_id}/stats` - 获取伏笔统计
- `POST /api/foreshadows` - 创建伏笔
- `POST /api/foreshadows/{id}/plant` - 标记伏笔为已埋入
- `POST /api/foreshadows/{id}/resolve` - 标记伏笔为已回收

### API 文档

- **Swagger UI**: http://localhost:8000/docs
- **ReDoc**: http://localhost:8000/redoc

---

## 关键依赖与配置

### Python 依赖 (`requirements.txt`)

**Web 框架：**
- `fastapi==0.121.0` - Web 框架
- `uvicorn[standard]==0.38.0` - ASGI 服务器
- `python-multipart==0.0.20` - 文件上传支持

**数据库：**
- `sqlalchemy==2.0.25` - ORM 框架
- `asyncpg==0.29.0` - PostgreSQL 异步驱动
- `psycopg2-binary==2.9.9` - PostgreSQL 同步驱动（迁移脚本）
- `alembic==1.14.0` - 数据库迁移工具
- `aiosqlite==0.22.1` - SQLite 异步驱动

**数据验证：**
- `pydantic==2.12.4` - 数据验证
- `pydantic-settings==2.11.0` - 配置管理

**AI 服务：**
- `openai==2.7.0` - OpenAI SDK
- `anthropic==0.72.0` - Claude SDK
- `mcp==1.22.0` - Model Context Protocol SDK

**向量数据库与 Embedding：**
- `chromadb==1.3.2` - 向量数据库
- `transformers==4.57.1` - Transformers 库
- `sentence-transformers==5.1.2` - Sentence Transformers

**工具库：**
- `httpx==0.28.1` - HTTP 客户端
- `python-dotenv==1.1.0` - 环境变量加载
- `psutil==6.1.1` - 系统监控

### 数据库配置

**PostgreSQL（生产环境）：**
```bash
DATABASE_URL=postgresql+asyncpg://mumuai:password@localhost:5432/mumuai_novel

# 连接池配置
DATABASE_POOL_SIZE=50
DATABASE_MAX_OVERFLOW=30
DATABASE_POOL_TIMEOUT=90
DATABASE_POOL_RECYCLE=1800
DATABASE_POOL_PRE_PING=True
DATABASE_POOL_USE_LIFO=True
```

**SQLite（开发环境）：**
```bash
DATABASE_URL=sqlite+aiosqlite:///./data/mumuai.db
```

### Alembic 迁移配置

**PostgreSQL 迁移：**
- 配置文件：`alembic-postgres.ini`
- 迁移脚本：`alembic/postgres/versions/`
- 环境脚本：`alembic/postgres/env.py`

**SQLite 迁移：**
- 配置文件：`alembic-sqlite.ini`
- 迁移脚本：`alembic/sqlite/versions/`
- 环境脚本：`alembic/sqlite/env.py`

**迁移命令：**
```bash
# 创建新迁移
alembic revision --autogenerate -m "描述"

# 应用迁移
alembic upgrade head

# 回滚迁移
alembic downgrade -1

# 查看迁移历史
alembic history
```

---

## 数据模型

### 核心数据表

所有数据模型位于 `app/models/` 目录：

| 模型文件 | 表名 | 功能 |
|---------|------|------|
| `user.py` | `users`, `user_passwords` | 用户信息与密码 |
| `project.py` | `projects` | 项目基本信息 |
| `outline.py` | `outlines` | 大纲 |
| `character.py` | `characters` | 角色与组织 |
| `career.py` | `careers`, `character_careers` | 职业等级体系 |
| `chapter.py` | `chapters` | 章节 |
| `relationship.py` | `character_relationships`, `organizations`, `organization_members`, `relationship_types` | 角色关系与组织 |
| `foreshadow.py` | `foreshadows` | 伏笔管理 |
| `writing_style.py` | `writing_styles` | 写作风格 |
| `project_default_style.py` | `project_default_styles` | 项目默认风格 |
| `prompt_template.py` | `prompt_templates` | 提示词模板 |
| `prompt_workshop.py` | `prompt_workshop_items`, `prompt_submissions`, `prompt_workshop_likes` | 提示词工坊 |
| `memory.py` | `story_memories`, `plot_analyses` | 长期记忆与剧情分析 |
| `mcp_plugin.py` | `mcp_plugins` | MCP 插件配置 |
| `settings.py` | `settings` | 用户设置 |
| `generation_history.py` | `generation_histories` | 生成历史 |
| `analysis_task.py` | `analysis_tasks` | 分析任务 |
| `batch_generation_task.py` | `batch_generation_tasks` | 批量生成任务 |
| `regeneration_task.py` | `regeneration_tasks` | 重新生成任务 |

### 数据隔离策略

**PostgreSQL 模式：**
- 所有用户共享同一个数据库
- 通过 `user_id` 字段隔离数据
- 所有查询自动添加 `user_id` 过滤条件
- 中间件验证用户身份和权限

**关键字段：**
```python
class Project(Base):
    __tablename__ = "projects"

    id = Column(String, primary_key=True)
    user_id = Column(String, nullable=False, index=True)  # 用户隔离
    title = Column(String, nullable=False)
    # ...
```

---

## 测试与质量

### 当前状态

- **单元测试**: 暂无
- **集成测试**: 暂无
- **API 测试**: 通过 Swagger UI 手动测试
- **健康检查**: `/health` 和 `/health/db-sessions` 端点

### 建议补充

1. **单元测试（pytest）**
   - 测试业务逻辑服务
   - 测试数据模型
   - 测试工具函数

2. **API 集成测试**
   - 使用 `httpx.AsyncClient` 测试 API 端点
   - 测试认证流程
   - 测试数据库操作

3. **性能测试**
   - 连接池压力测试
   - SSE 流式响应性能测试
   - 并发用户测试

---

## 常见问题 (FAQ)

### 如何添加新的 API 端点？

1. 在 `app/api/` 创建或修改路由文件
2. 定义路由函数，使用 `@router.get/post/put/delete` 装饰器
3. 使用 `Depends(get_db)` 注入数据库会话
4. 在 `app/main.py` 中注册路由：`app.include_router(router, prefix="/api")`

**示例：**
```python
from fastapi import APIRouter, Depends
from sqlalchemy.ext.asyncio import AsyncSession
from app.database import get_db

router = APIRouter(tags=["example"])

@router.get("/example")
async def get_example(db: AsyncSession = Depends(get_db)):
    return {"message": "Hello World"}
```

### 如何实现 SSE 流式响应？

1. 使用 `StreamingResponse` 返回生成器
2. 生成器使用 `yield` 返回数据
3. 数据格式：`data: {json}\n\n`

**示例：**
```python
from fastapi import APIRouter
from fastapi.responses import StreamingResponse
import json

router = APIRouter()

@router.post("/stream")
async def stream_example():
    async def generate():
        for i in range(10):
            data = {"progress": i, "message": f"Step {i}"}
            yield f"data: {json.dumps(data)}\n\n"
            await asyncio.sleep(0.5)
        yield f"data: {json.dumps({'done': True})}\n\n"

    return StreamingResponse(
        generate(),
        media_type="text/event-stream"
    )
```

### 如何处理数据库会话泄漏？

1. **监控会话统计**：访问 `/health/db-sessions` 查看活跃会话数
2. **确保会话关闭**：使用 `Depends(get_db)` 自动管理会话生命周期
3. **避免手动创建会话**：不要直接创建 `AsyncSession`
4. **SSE 流式响应**：确保在 `finally` 块中关闭会话

**会话统计指标：**
- `created`: 总创建会话数
- `closed`: 总关闭会话数
- `active`: 当前活跃会话数（应接近 0）
- `errors`: 错误次数
- `generator_exits`: SSE 断开次数

### 如何调试 AI 服务调用？

1. **启用调试日志**：设置 `LOG_LEVEL=DEBUG`
2. **查看日志文件**：`logs/app.log`
3. **测试 API 连接**：使用 Settings 页面的"测试连接"功能
4. **检查 API Key**：确保 API Key 有效且有足够额度
5. **查看错误详情**：API 返回的错误信息包含详细的错误类型和建议

---

## 相关文件清单

### 核心文件

```
backend/
├── app/
│   ├── main.py                 # 应用入口
│   ├── config.py               # 配置管理
│   ├── database.py             # 数据库连接
│   ├── logger.py               # 日志配置
│   ├── api/                    # API 路由
│   │   ├── auth.py
│   │   ├── projects.py
│   │   ├── wizard_stream.py
│   │   └── ...
│   ├── models/                 # 数据模型
│   │   ├── user.py
│   │   ├── project.py
│   │   └── ...
│   ├── schemas/                # Pydantic 模型
│   ├── services/               # 业务逻辑
│   │   ├── ai_service.py
│   │   ├── ai_providers/
│   │   └── ...
│   ├── middleware/             # 中间件
│   │   ├── auth_middleware.py
│   │   └── request_id.py
│   └── mcp/                    # MCP 插件系统
├── alembic/                    # 数据库迁移
│   ├── postgres/
│   └── sqlite/
├── scripts/                    # 工具脚本
│   ├── entrypoint.sh
│   ├── migrate.py
│   └── init_postgres.sql
├── requirements.txt            # Python 依赖
├── alembic-postgres.ini        # Alembic 配置（PostgreSQL）
└── alembic-sqlite.ini          # Alembic 配置（SQLite）
```

### 配置文件

- `.env` - 环境变量配置
- `.env.example` - 环境变量示例
- `alembic-postgres.ini` - PostgreSQL 迁移配置
- `alembic-sqlite.ini` - SQLite 迁移配置

### 数据目录

- `data/` - SQLite 数据库文件（开发环境）
- `logs/` - 应用日志文件
- `embedding/` - Sentence-Transformers 模型缓存

---

**最后更新**: 2026-02-21 23:14:25
**模块版本**: 1.3.5
