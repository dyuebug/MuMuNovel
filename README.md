# MuMuNovel 📚✨

<div align="center">

![Version](https://img.shields.io/badge/version-1.3.9-blue.svg)
![Python](https://img.shields.io/badge/python-3.11-blue.svg)
![FastAPI](https://img.shields.io/badge/FastAPI-0.109.0-green.svg)
![React](https://img.shields.io/badge/react-18.3.1-blue.svg)
![License](https://img.shields.io/badge/license-GPL%20v3-blue.svg)

**基于 AI 的智能小说创作助手**

[特性](#-特性) • [快速开始](#-快速开始) • [配置说明](#%EF%B8%8F-配置说明) • [项目结构](#-项目结构)

</div>

---

<div align="center">

## 💬 加入交流群

欢迎扫码加入 QQ 交流群，一起交流 AI 小说创作心得、反馈问题、获取最新动态！

<img src="frontend/public/qq.jpg" alt="QQ交流群二维码" width="300" />

</div>

---

<div align="center">

## 💖 支持项目

如果这个项目对你有帮助，欢迎通过以下方式支持开发：

**[☕ 请我喝杯咖啡](https://mumuverse.space:1588/)**

### 🎁 赞助专属权益

| 权益 | 说明 |
|------|------|
| 📋 **优先需求响应** | 您的功能需求和问题反馈将获得优先处理 |
| 🚀 **Windows一键启动** | 获取免安装EXE程序，双击即可使用 |
| 💬 **专属技术支持** | 加入赞助者内部群，获得远程协助和配置指导 |

### ☕ 赞助金额

| 金额 | 描述 |
|------|------|
| ¥5 | 🌶️ 一包辣条 |
| ¥10 | 🍱 一顿拼好饭 |
| ¥20 | 🧋 一杯咖啡 |
| ¥50 | 🍖 一次烧烤 |
| ¥99 | 🍲 一顿海底捞 |

您的支持是我持续开发的动力！🙏

</div>

---

## ✨ 特性

- 🤖 **多 AI 模型** - 支持 OpenAI、Gemini、Claude 等主流模型
- 📝 **智能向导** - AI 自动生成大纲、角色和世界观
- 👥 **角色管理** - 人物关系、组织架构可视化管理
- 📖 **章节编辑** - 支持创建、编辑、重新生成和润色
- 🌐 **世界观设定** - 构建完整的故事背景
- 🔐 **多种登录** - LinuxDO OAuth 或本地账户登录
- 💾 **PostgreSQL** - 生产级数据库，多用户数据隔离
- 🐳 **Docker 部署** - 一键启动，开箱即用

## 📸 项目预览

<details>

<summary>多图预警</summary>

<div align="center">

### 登录界面
![登录界面](images/1.png)

![登录界面](images/1-1.png)

### 主界面
![主界面](images/2.png)

![主界面（暗色）](images/2-1.png)

### 项目管理
![项目管理](images/3.png)

![项目管理](images/3-1.png)

### 赞助我 💖
![赞助我](images/4.png)

![赞助我](images/4-1.png)

</div>

</details>

## 📋 TODO List

### ✅ 已完成功能

- [x] **灵感模式** - 创作灵感和点子生成
- [x] **自定义写作风格** - 支持自定义 AI 写作风格
- [x] **数据导入导出** - 项目数据的导入导出
- [x] **Prompt 调整界面** - 可视化编辑 Prompt 模板
- [x] **章节字数限制** - 用户可设置生成字数
- [x] **思维链与章节关系图谱** - 可视化章节逻辑关系
- [x] **根据分析一键重写** - 根据分析建议重新生成
- [x] **Linux DO 自动创建账号** - OAuth 登录自动生成账号
- [x] **职业等级体系** - 自定义职业和等级系统，支持修仙境界、魔法等级等多种体系
- [x] **角色/组织卡片导入导出** - 单独导出角色和组织卡片，支持跨项目数据共享
- [x] **伏笔管理** - 智能追踪剧情伏笔，提醒未回收线索，可视化伏笔时间线
- [x] **提示词工坊** - 社区驱动的 Prompt 模板分享平台，一键导入优质提示词
- [x] **拆书功能** - 目前呼声比较高的功能，一键拆书，给当年的ta一个圆满的结局

### 📝 规划中功能

......

> 💡 欢迎提交 Issue 或 Pull Request！

## 💻 硬件配置要求

### 最低配置（个人使用/开发环境）

| 组件 | 要求 |
|------|------|
| **CPU** | 2 核 |
| **内存** | 2 GB RAM |
| **存储** | 10 GB 可用空间 |
| **网络** | 稳定互联网连接（用于调用 AI API） |

### 推荐配置（小型团队/生产环境）

| 组件 | 要求 |
|------|------|
| **CPU** | 4 核 |
| **内存** | 8 GB RAM |
| **存储** | 20 GB SSD |
| **网络** | 稳定互联网连接 |

### 高并发配置（80-150 用户）

| 组件 | 要求 |
|------|------|
| **CPU** | 8 核 |
| **内存** | 16 GB RAM |
| **存储** | 50 GB+ SSD |
| **网络** | 高带宽连接 |

> **📌 说明**
> - **Embedding 模型**：约 400 MB 磁盘空间，运行时加载到内存
> - **PostgreSQL**：默认配置使用 256 MB shared_buffers，1 GB effective_cache_size
> - **Docker 部署**：建议预留额外 1-2 GB 内存给容器运行时
> - 本项目主要依赖外部 AI API（OpenAI/Claude/Gemini），不需要本地 GPU

## 🧩 前端服务层约定

前端服务层当前采用模块化结构：

- `frontend/src/services/core/httpClient.ts`：唯一真实 HTTP 客户端实现
- `frontend/src/services/modules/*.ts`：按业务域拆分的 API 实现
- `frontend/src/services/modularApi.ts`：推荐的前端服务聚合入口
- `frontend/src/services/api.ts`：仅保留历史导入路径与默认 `api` 转发的兼容门面

开发时默认约定：

- 新运行时代码优先从 `frontend/src/services/modularApi.ts` 导入
- 只有在需要强聚焦、按域直取时，才直接从 `frontend/src/services/modules/*` 导入
- 不再为新代码增加对 `frontend/src/services/api.ts` 的依赖
- `frontend/eslint.config.js` 已限制新增运行时代码继续导入 `services/api.ts`
- 详细约定见 `docs/architecture/frontend-service-layer-conventions.zh-CN.md`
## 🚀 快速开始

### 前置要求

- Docker 和 Docker Compose
- 至少一个 AI 服务的 API Key（OpenAI/Gemini/Claude）

### Docker Compose 部署（推荐）

```bash
# 1. 克隆项目
git clone https://github.com/dyuebug/MuMuNovel.git
cd MuMuNovel

# 2. 配置环境变量（必需）
cp backend/.env.example .env
# 编辑 .env 文件，填入必要配置（API Key、数据库密码等）

# 3. 确保文件准备完整
# ⚠️ 重要：确保以下文件存在
# - .env（配置文件，必需挂载到容器）
# - backend/scripts/init_postgres.sql（数据库初始化脚本）

# 4. 启动 Rust runtime + Rust db-migrator + Nginx gateway
docker compose -f docker-compose.strangler.yml up -d --build

# 5. 访问应用
# 打开浏览器访问 http://localhost:8005
```

> **📌 注意事项**
>
> 1. **`.env` 文件挂载**: `docker-compose.yml` 会自动将 `.env` 挂载到容器，确保文件存在
> 2. **数据库初始化**: `init_postgres.sql` 会在首次启动时自动执行，安装必要的PostgreSQL扩展
> 3. **自行构建**: 如需从源码构建，请先下载 embedding 模型文件（[加群获取](frontend/public/qq.jpg)）

### 使用 Docker Hub 镜像（旧单容器镜像，不再推荐）

当前生产入口已经迁移到 Rust runtime + Rust db-migrator + Nginx gateway。
旧 Docker Hub 单容器镜像示例仍保留作历史参考，但它不是当前推荐部署路径。
新部署请优先使用仓库内 `docker-compose.strangler.yml` 或 `deploy-strangler.bat`。

```bash
# 旧镜像路径：仅作历史参考，不作为当前 Rust runtime 部署入口
docker pull mumujie/mumuainovel:latest

# 2. 创建 docker-compose.yml（点击下方展开查看完整配置）
```

当前仓库已内置 Rust runtime Compose 配置：`docker-compose.strangler.yml`。
不要复制旧 Python 单容器 compose 示例；它会重新引入退役的 `app.main`
启动路径。

```bash
# 3. 启动当前 Rust runtime 栈
docker compose -f docker-compose.strangler.yml up -d --build

# 4. 查看日志
docker compose -f docker-compose.strangler.yml logs -f

# 5. 更新到最新版本
git pull
docker compose -f docker-compose.strangler.yml up -d --build
```

> **💡 提示**: Docker Hub 镜像已包含所有依赖和模型文件，无需额外下载

### 本地开发 / 从源码构建

#### 前置准备

```bash
# ⚠️ 重要：如果从源码构建，需要先下载 embedding 模型文件
# 模型文件较大（约 400MB），需放置到以下目录：
# backend/embedding/models--sentence-transformers--paraphrase-multilingual-MiniLM-L12-v2/
#
# 📥 获取方式：
# - 加入项目 QQ 群或 Linux DO 讨论区获取下载链接
# - 群号：见项目主页
# - Linux DO：https://linux.do/t/topic/1100112
```

#### 后端 / 生产 runtime

```bash
# 当前生产后端是 Rust，不再通过 uvicorn/app.main 启动 Python runtime。
# Windows 推荐：
deploy-strangler.bat -NoPause

# 或直接使用 Compose：
docker compose -f docker-compose.strangler.yml up -d --build
```

#### 前端

```bash
cd frontend
npm install
npm run dev  # 开发模式
npm run build  # 生产构建
```


#### 编码体检（可选）

```bash
python backend/tools/check_text_encoding_health.py              # ????????????
python backend/tools/check_text_encoding_health.py --include-docs  # ???? docs/ ??
```
## ⚙️ 配置说明

### 必需配置

创建 `.env` 文件：

```bash
# PostgreSQL 数据库（必需）
DATABASE_URL=postgresql+asyncpg://mumuai:your_password@postgres:5432/mumuai_novel
POSTGRES_PASSWORD=your_secure_password

# AI 服务
OPENAI_API_KEY=your_openai_key
OPENAI_BASE_URL=https://api.openai.com/v1
DEFAULT_AI_PROVIDER=openai
DEFAULT_MODEL=gpt-4o-mini

# 本地账户登录
LOCAL_AUTH_ENABLED=true
LOCAL_AUTH_USERNAME=admin
LOCAL_AUTH_PASSWORD=your_password
```

### 可选配置

```bash
# LinuxDO OAuth
LINUXDO_CLIENT_ID=your_client_id
LINUXDO_CLIENT_SECRET=your_client_secret
LINUXDO_REDIRECT_URI=http://localhost:8005/api/auth/callback

# PostgreSQL 连接池（高并发优化）
DATABASE_POOL_SIZE=30
DATABASE_MAX_OVERFLOW=20
```

### 中转 API 配置

支持所有 OpenAI 兼容格式的中转服务：

```bash
# New API 示例
OPENAI_API_KEY=sk-xxxxxxxx
OPENAI_BASE_URL=https://api.new-api.com/v1

# 其他中转服务
OPENAI_BASE_URL=https://your-proxy-service.com/v1
```

## 🐳 Docker 部署详情

### 服务架构

- **postgres**: PostgreSQL 18 数据库
  - 端口: 5432
  - 数据持久化: `postgres_data` volume
  - 初始化脚本: `backend/scripts/init_postgres.sql`（自动挂载）
  - 优化配置: 支持 80-150 并发用户

- **db-migrator**: Rust 一次性数据库迁移服务
  - 命令: `migration-executor`
  - 职责: 显式执行 PostgreSQL schema 迁移

- **rust-backend**: Rust 生产后端
  - 端口: 容器内 `8001`
  - 职责: API、SSE、后台任务、静态资源回退
  - 健康检查: `http://localhost:8001/health`

- **nginx**: 统一入口
  - 端口: `8005` -> `80`
  - 职责: 对外网关，转发到 Rust backend

### 重要文件说明

| 文件 | 说明 | 是否必需 |
|------|------|---------|
| `.env` | 环境配置（API Key、数据库密码等） | ✅ 必需 |
| `docker-compose.yml` | 服务编排配置 | ✅ 必需 |
| `backend/scripts/init_postgres.sql` | PostgreSQL 扩展安装脚本 | ✅ 自动挂载 |
| `backend/embedding/models--*/` | Embedding 模型文件 | ⚠️ 自建需要 |

> **注意**: 使用 Docker Hub 镜像时，模型文件已包含在镜像中，无需额外下载

### 常用命令

```bash
# 启动服务
docker compose -f docker-compose.strangler.yml up -d --build

# 查看状态
docker compose -f docker-compose.strangler.yml ps

# 查看日志
docker compose -f docker-compose.strangler.yml logs -f

# 停止服务
docker compose -f docker-compose.strangler.yml down

# 重启服务
docker compose -f docker-compose.strangler.yml restart

# 查看资源使用
docker stats
```

### 数据持久化

- `./postgres_data` - PostgreSQL 数据库文件
- `./logs` - 应用日志文件

### 端口配置

修改 `docker-compose.yml` 中的端口映射：

```yaml
ports:
  - "8800:80"  # 宿主机:Nginx 容器
```

## 📁 项目结构

```
MuMuNovel/
├── backend-rs/              # Rust 生产后端：API、SSE、任务、数据库访问、静态资源回退
├── backend/                 # Python 迁移/测试/运维支撑，不再是生产 runtime
│   ├── alembic/             # PostgreSQL Alembic source-map
│   ├── migrator_app/        # 冻结的迁移 metadata 包
│   ├── scripts/             # 部署 / 数据库脚本
│   ├── tests/               # Python 回归测试支撑
│   └── tools/               # 诊断、smoke、编码体检工具
├── frontend/               # 前端应用
│   ├── src/
│   │   ├── pages/         # 页面组件
│   │   ├── components/    # 通用组件
│   │   ├── services/      # API 服务
│   │   └── store/         # 状态管理
│   └── package.json
├── docker-compose.strangler.yml # Rust runtime Compose 配置
├── docker-compose.yml           # 当前同样指向 Rust runtime 栈
├── deploy-strangler.bat         # Windows 一键部署入口
└── README.md
```

## 🛠️ 技术栈

**后端**: Rust/Axum • PostgreSQL • SeaORM/SQLx • OpenAI/Claude/Gemini SDK

**前端**: React 18 • TypeScript • Ant Design • Zustand • Vite

## 📖 使用指南

1. **登录系统** - 使用本地账户或 LinuxDO 账户
2. **创建项目** - 选择"使用向导创建"
3. **AI 生成** - 输入基本信息，AI 自动生成大纲和角色
4. **编辑完善** - 管理角色关系，生成和编辑章节

### API 文档

当前生产入口是 Rust + Nginx：`http://localhost:8005`。
旧 FastAPI Swagger/ReDoc 入口已随 Python runtime 退役，不作为当前部署能力。

## 🤝 贡献

欢迎提交 Issue 和 Pull Request！

1. Fork 本项目
2. 创建特性分支 (`git checkout -b feature/AmazingFeature`)
3. 提交更改 (`git commit -m 'Add some AmazingFeature'`)
4. 推送到分支 (`git push origin feature/AmazingFeature`)
5. 提交 Pull Request

### 贡献者

感谢所有为本项目做出贡献的开发者！

<a href="https://github.com/dyuebug/MuMuNovel/graphs/contributors">
  <img src="https://contrib.rocks/image?repo=dyuebug/MuMuNovel" />
</a>

## 📝 许可证

本项目采用 [GNU General Public License v3.0](LICENSE)

**GPL v3 意味着：**
- ✅ 可自由使用、修改和分发
- ✅ 可用于商业目的
- 📝 必须开源修改版本
- 📝 必须保留原作者版权
- 📝 衍生作品必须使用 GPL v3 协议

## 🙏 致谢

- [FastAPI](https://fastapi.tiangolo.com/) - Python Web 框架
- [React](https://react.dev/) - 前端框架
- [Ant Design](https://ant.design/) - UI 组件库
- [PostgreSQL](https://www.postgresql.org/) - 数据库

## 📧 联系方式

- 提交 [Issue](https://github.com/dyuebug/MuMuNovel/issues)
- Linux DO [讨论](https://linux.do/t/topic/1106333)
- 加入QQ群 [QQ群](frontend/public/qq.jpg)
- 加入WX群 [WX群](frontend/public/WX.png)

---

<div align="center">

**如果这个项目对你有帮助，请给个 ⭐️ Star！**

Made with ❤️

</div>

## Star History

<a href="https://www.star-history.com/#dyuebug/MuMuNovel&type=date&legend=top-left">
 <picture>
   <source media="(prefers-color-scheme: dark)" srcset="https://api.star-history.com/svg?repos=dyuebug/MuMuNovel&type=date&theme=dark&legend=top-left" />
   <source media="(prefers-color-scheme: light)" srcset="https://api.star-history.com/svg?repos=dyuebug/MuMuNovel&type=date&legend=top-left" />
   <img alt="Star History Chart" src="https://api.star-history.com/svg?repos=dyuebug/MuMuNovel&type=date&legend=top-left" />
 </picture>
</a>

## History

![Alt](https://repobeats.axiom.co/api/embed/ee7141a5f269c64759302e067abe23b46796bafe.svg "Repobeats analytics image")


### 开发联调快速回归

本地后端、前端代理和 Docker 应用都启动后，可以运行下面的 PowerShell 脚本做最小 smoke check：

```powershell
powershell -ExecutionPolicy Bypass -File .\check-auth-flow.ps1
```

脚本会默认读取根目录 `.env` 中的 `APP_PORT`、`DOCKER_APP_PORT`、`LOCAL_AUTH_USERNAME` 和 `LOCAL_AUTH_PASSWORD`，依次校验：

- `backend /readyz`
- `backend /api/auth/config`
- `backend` 登录 / 鉴权 / 刷新 / 项目列表 / 退出
- `frontend` 代理登录链路
- `docker app /readyz`

如果前端 dev server 或 Docker 应用暂时没有启动，可以按需跳过：

```powershell
powershell -ExecutionPolicy Bypass -File .\check-auth-flow.ps1 -SkipFrontend
powershell -ExecutionPolicy Bypass -File .\check-auth-flow.ps1 -SkipDockerReadyz
```
