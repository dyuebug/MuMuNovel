# MuMuAINovel - AI 智能小说创作助手

> 基于 AI 的智能小说创作助手，支持多 AI 模型、智能向导、角色管理、章节编辑等功能

## 变更记录 (Changelog)

### 2026-02-21 23:14:25
- 初始化项目架构文档
- 完成模块结构扫描和索引

---

## 项目愿景

MuMuAINovel 是一个基于 AI 的智能小说创作助手，旨在帮助作者通过 AI 技术提升创作效率。项目支持多种 AI 模型（OpenAI、Gemini、Claude），提供智能向导自动生成大纲、角色和世界观，支持角色关系可视化管理、章节编辑与润色、伏笔追踪、提示词工坊等功能。

**核心特性：**
- 多 AI 模型支持（OpenAI、Gemini、Claude）
- 智能向导自动生成大纲、角色、世界观
- 角色管理与组织架构可视化
- 章节创建、编辑、重新生成、润色
- 伏笔管理与时间线追踪
- 提示词工坊（社区驱动的 Prompt 分享平台）
- 职业等级体系（修仙境界、魔法等级等）
- 数据导入导出（项目、角色、组织）
- 多用户支持（LinuxDO OAuth + 本地账户）
- PostgreSQL 生产级数据库，多用户数据隔离

---

## 架构总览

MuMuAINovel 采用前后端分离架构，后端使用 FastAPI 提供 RESTful API 和 SSE 流式接口，前端使用 React + TypeScript 构建单页应用。

**技术栈：**
- **后端**: Python 3.11 + FastAPI + PostgreSQL/SQLite + SQLAlchemy + Alembic
- **前端**: React 18 + TypeScript + Ant Design + Zustand + Vite
- **AI 服务**: OpenAI、Gemini、Claude、MCP 插件系统
- **数据库**: PostgreSQL 18（生产）/ SQLite（开发）
- **部署**: Docker + Docker Compose
- **向量数据库**: ChromaDB（长期记忆系统）
- **Embedding**: Sentence-Transformers（多语言支持）

**架构特点：**
- 前后端分离，API 驱动
- SSE 流式响应，实时反馈 AI 生成进度
- PostgreSQL 多用户数据隔离（通过 user_id 字段）
- 连接池优化，支持 150-200 并发用户
- Alembic 数据库迁移管理
- MCP（Model Context Protocol）插件系统
- Docker 容器化部署，开箱即用

---

## 模块结构图

```mermaid
graph TD
    A["(根) MuMuAINovel"] --> B["backend"];
    A --> C["frontend"];
    B --> D["app/api"];
    B --> E["app/models"];
    B --> F["app/services"];
    B --> G["app/middleware"];
    B --> H["alembic"];
    C --> I["src/pages"];
    C --> J["src/components"];
    C --> K["src/services"];

    click B "./backend/CLAUDE.md" "查看 backend 模块文档"
    click C "./frontend/CLAUDE.md" "查看 frontend 模块文档"
```

---

## 模块索引

| 模块路径 | 语言 | 职责 | 入口文件 | 文档 |
|---------|------|------|---------|------|
| `backend/` | Python | FastAPI 后端服务，提供 RESTful API 和 SSE 流式接口 | `app/main.py` | [backend/CLAUDE.md](./backend/CLAUDE.md) |
| `frontend/` | TypeScript | React 前端应用，提供用户界面 | `src/main.tsx` | [frontend/CLAUDE.md](./frontend/CLAUDE.md) |

---

## 运行与开发

### Docker Compose 部署（推荐）

```bash
# 1. 克隆项目
git clone https://github.com/xiamuceer-j/MuMuAINovel.git
cd MuMuAINovel

# 2. 配置环境变量
cp backend/.env.example .env
# 编辑 .env 文件，填入必要配置（API Key、数据库密码等）

# 3. 启动服务
docker-compose up -d

# 4. 访问应用
# 打开浏览器访问 http://localhost:8000
```

### 本地开发

**后端：**
```bash
cd backend
python -m venv .venv
source .venv/bin/activate  # Windows: .venv\Scripts\activate
pip install -r requirements.txt

# 配置 .env 文件
cp .env.example .env

# 启动 PostgreSQL（可使用 Docker）
docker run -d --name postgres \
  -e POSTGRES_PASSWORD=your_password \
  -e POSTGRES_DB=mumuai_novel \
  -p 5432:5432 \
  postgres:18-alpine

# 运行数据库迁移
alembic upgrade head

# 启动后端
python -m uvicorn app.main:app --host localhost --port 8000 --reload
```

**前端：**
```bash
cd frontend
npm install
npm run dev  # 开发模式
npm run build  # 生产构建
```

### 环境变量配置

**必需配置：**
```bash
# PostgreSQL 数据库
DATABASE_URL=postgresql+asyncpg://mumuai:your_password@postgres:5432/mumuai_novel
POSTGRES_PASSWORD=your_secure_password

# AI 服务（至少配置一个）
OPENAI_API_KEY=your_openai_key
OPENAI_BASE_URL=https://api.openai.com/v1
DEFAULT_AI_PROVIDER=openai
DEFAULT_MODEL=gpt-4o-mini

# 本地账户登录
LOCAL_AUTH_ENABLED=true
LOCAL_AUTH_USERNAME=admin
LOCAL_AUTH_PASSWORD=your_password
```

**可选配置：**
```bash
# LinuxDO OAuth
LINUXDO_CLIENT_ID=your_client_id
LINUXDO_CLIENT_SECRET=your_client_secret
LINUXDO_REDIRECT_URI=http://localhost:8000/api/auth/callback

# PostgreSQL 连接池（高并发优化）
DATABASE_POOL_SIZE=30
DATABASE_MAX_OVERFLOW=20
```

---

## 测试策略

**当前状态：** 项目暂无自动化测试

**测试方式：**
- 手动测试：通过前端界面进行功能测试
- API 测试：使用 Swagger UI (`http://localhost:8000/docs`) 进行 API 测试
- 健康检查：`GET /health` 和 `GET /health/db-sessions` 端点

**建议补充：**
- 后端单元测试（pytest）
- 前端组件测试（Jest + React Testing Library）
- E2E 测试（Playwright）
- API 集成测试

---

## 编码规范

### Python（后端）

- **代码风格**: PEP 8
- **类型注解**: 使用 Python 类型提示
- **文档字符串**: 使用 Google 风格的 docstring
- **异步编程**: 使用 async/await，避免阻塞操作
- **数据库会话**: 使用依赖注入 `get_db()`，确保会话正确关闭
- **错误处理**: 使用 HTTPException 返回标准错误响应
- **日志记录**: 使用 `app.logger.get_logger(__name__)`

**示例：**
```python
from fastapi import APIRouter, Depends, HTTPException
from sqlalchemy.ext.asyncio import AsyncSession
from app.database import get_db
from app.logger import get_logger

logger = get_logger(__name__)
router = APIRouter()

@router.get("/example/{id}")
async def get_example(
    id: str,
    db: AsyncSession = Depends(get_db)
) -> dict:
    """获取示例数据

    Args:
        id: 示例ID
        db: 数据库会话

    Returns:
        示例数据字典

    Raises:
        HTTPException: 404 如果数据不存在
    """
    try:
        # 业务逻辑
        pass
    except Exception as e:
        logger.error(f"获取示例失败: {e}")
        raise HTTPException(status_code=500, detail="服务器错误")
```

### TypeScript（前端）

- **代码风格**: ESLint + TypeScript ESLint
- **组件**: 使用函数组件 + Hooks
- **状态管理**: Zustand（轻量级状态管理）
- **类型定义**: 统一在 `src/types/index.ts` 中定义
- **API 调用**: 使用 `src/services/api.ts` 中的封装方法
- **错误处理**: 使用 Ant Design 的 `message` 组件显示错误

**示例：**
```typescript
import { useState, useEffect } from 'react';
import { message } from 'antd';
import { projectApi } from '../services/api';
import type { Project } from '../types';

export default function ProjectList() {
  const [projects, setProjects] = useState<Project[]>([]);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    loadProjects();
  }, []);

  const loadProjects = async () => {
    try {
      setLoading(true);
      const data = await projectApi.getProjects();
      setProjects(data);
    } catch (error) {
      message.error('加载项目失败');
    } finally {
      setLoading(false);
    }
  };

  return (
    // JSX
  );
}
```

---

## AI 使用指引

### 与 AI 协作的最佳实践

1. **理解项目结构**：先阅读本文档和模块级 CLAUDE.md，了解项目架构
2. **查看现有代码**：参考现有 API 路由和服务的实现方式
3. **遵循编码规范**：保持代码风格一致
4. **数据库操作**：使用 SQLAlchemy ORM，避免原生 SQL
5. **错误处理**：统一使用 HTTPException 和日志记录
6. **API 设计**：遵循 RESTful 规范，使用合适的 HTTP 方法和状态码
7. **前端组件**：复用现有组件，保持 UI 一致性

### 常见任务

**添加新的 API 端点：**
1. 在 `backend/app/api/` 创建或修改路由文件
2. 在 `backend/app/models/` 定义数据模型（如需要）
3. 在 `backend/app/schemas/` 定义 Pydantic 模型
4. 在 `backend/app/services/` 实现业务逻辑
5. 在 `backend/app/main.py` 注册路由
6. 在 `frontend/src/services/api.ts` 添加 API 调用方法
7. 在前端页面中使用新的 API

**添加数据库表：**
1. 在 `backend/app/models/` 创建新的模型文件
2. 在 `backend/app/models/__init__.py` 导出模型
3. 创建 Alembic 迁移：`alembic revision --autogenerate -m "描述"`
4. 检查生成的迁移文件
5. 应用迁移：`alembic upgrade head`

**添加前端页面：**
1. 在 `frontend/src/pages/` 创建页面组件
2. 在 `frontend/src/App.tsx` 添加路由
3. 在 `frontend/src/services/api.ts` 添加 API 调用（如需要）
4. 使用 Ant Design 组件保持 UI 一致性

---

## 关键技术点

### 数据库连接池优化

项目针对高并发场景进行了连接池优化，支持 150-200 并发用户：

- **核心连接**: 50（从 30 提升）
- **溢出连接**: 30（从 20 提升）
- **总连接数**: 80
- **连接超时**: 90 秒
- **连接回收**: 1800 秒（30 分钟）
- **连接前检测**: 启用（pool_pre_ping）
- **LIFO 策略**: 启用（提高连接复用率）

### SSE 流式响应

项目使用 Server-Sent Events (SSE) 实现 AI 生成内容的实时流式传输：

- **后端**: 使用 `yield` 生成器返回流式数据
- **前端**: 使用 `src/utils/sseClient.ts` 封装的 SSE 客户端
- **进度反馈**: 实时显示 AI 生成进度和状态
- **错误处理**: 自动重连和错误提示

### MCP 插件系统

支持 Model Context Protocol (MCP) 插件，扩展 AI 能力：

- **插件管理**: `backend/app/mcp/` 目录
- **工具调用**: 支持动态加载和调用 MCP 工具
- **状态同步**: 自动同步插件状态
- **配置管理**: 通过数据库存储插件配置

### 多用户数据隔离

PostgreSQL 模式下，所有用户共享同一个数据库，通过 `user_id` 字段隔离数据：

- **会话管理**: 从 `request.state.user_id` 获取用户 ID
- **数据过滤**: 所有查询自动添加 `user_id` 过滤条件
- **权限控制**: 中间件验证用户身份和权限

---

## 常见问题

### 如何切换数据库？

项目支持 PostgreSQL 和 SQLite：

- **PostgreSQL**（推荐生产环境）：修改 `DATABASE_URL` 为 `postgresql+asyncpg://...`
- **SQLite**（开发环境）：修改 `DATABASE_URL` 为 `sqlite+aiosqlite:///./data/mumuai.db`

### 如何添加新的 AI 模型？

1. 在 `backend/app/services/ai_providers/` 创建新的 Provider 类
2. 继承 `BaseProvider` 并实现必要方法
3. 在 `backend/app/services/ai_service.py` 注册新的 Provider
4. 在前端 Settings 页面添加新模型的配置选项

### 如何调试 SSE 流式响应？

1. 打开浏览器开发者工具 -> Network 标签
2. 筛选 EventStream 类型的请求
3. 查看 Messages 标签查看实时数据流
4. 后端日志会记录详细的流式响应过程

### 数据库迁移失败怎么办？

```bash
# 查看当前迁移状态
alembic current

# 查看迁移历史
alembic history

# 回滚到上一个版本
alembic downgrade -1

# 重新应用迁移
alembic upgrade head
```

---

## 相关资源

- **项目仓库**: https://github.com/xiamuceer-j/MuMuAINovel
- **API 文档**: http://localhost:8000/docs (Swagger UI)
- **ReDoc**: http://localhost:8000/redoc
- **Linux DO 讨论**: https://linux.do/t/topic/1106333
- **许可证**: GNU General Public License v3.0

---

## 贡献指南

欢迎提交 Issue 和 Pull Request！

1. Fork 本项目
2. 创建特性分支 (`git checkout -b feature/AmazingFeature`)
3. 提交更改 (`git commit -m 'Add some AmazingFeature'`)
4. 推送到分支 (`git push origin feature/AmazingFeature`)
5. 提交 Pull Request

---

**最后更新**: 2026-02-21 23:14:25
**文档版本**: 1.0.0
**项目版本**: 1.3.5
