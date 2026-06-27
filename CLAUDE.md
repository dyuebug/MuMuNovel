# MuMuNovel - AI 智能小说创作助手

> 面向长篇小说创作的 AI 协作平台，当前生产运行形态是
> **Rust backend + Nginx gateway + PostgreSQL + Rust db-migrator**。

## 变更记录

### 2026-06-25

- 根级上下文切换到 Rust-owned runtime。
- 删除旧 FastAPI runtime、`backend/app`、`uvicorn` 与 Swagger/ReDoc 说明。
- 明确 Python 当前只保留冻结的迁移 source-map、Alembic metadata 与测试支撑边界。

---

## 项目目的

MuMuNovel 把小说创作工作流拆成可逐步完成的 AI 能力：

- 项目创建与智能向导
- 世界观、角色、职业体系与大纲生成
- 章节写作、重写、分析、批量生成与后台任务恢复
- 灵感模式与生成前网络检索透传
- 伏笔、写作风格、记忆与提示词工坊
- 多模型接入：OpenAI 兼容接口、Anthropic、Gemini、MCP 工具链

---

## 当前架构

```text
MuMuNovel/
├── backend-rs/              # Rust 生产后端：API、SSE、任务、数据库访问、静态资源回退
│   ├── src/main.rs          # Rust 服务入口
│   ├── src/api/             # Axum API 路由
│   ├── src/services/        # 业务 owner 与迁移后的运行语义
│   └── Cargo.toml
├── backend/                 # 冻结的 Python 迁移 source-map、Alembic metadata、工具与测试
│   ├── migrator_app/        # Alembic metadata owner，不是生产 runtime
│   ├── alembic/             # PostgreSQL migrations；SQLite 为 legacy/manual profile
│   ├── scripts/             # DB migration scripts
│   ├── tools/               # 迁移/检查工具
│   └── tests/               # migrator/tool/test-support 测试
├── frontend/                # React + TypeScript 前端
│   ├── src/main.tsx
│   ├── src/App.tsx
│   ├── src/components/
│   ├── src/pages/
│   └── src/services/
├── deploy/                  # Nginx、gateway smoke、部署探针
├── docker-compose.yml       # Rust runtime compose
└── docker-compose.strangler.yml
```

---

## 模块索引

| 模块 | 语言 | 职责 | 主入口 | 文档 |
|---|---|---|---|---|
| `backend-rs/` | Rust | 生产 API、SSE、后台任务、数据库访问、静态资源回退 | `backend-rs/src/main.rs` | `backend-rs/CLAUDE.md` |
| `backend/` | Python | 冻结的迁移 source-map、Alembic metadata、迁移工具与测试支持 | frozen source-map / test-support | `backend/CLAUDE.md` |
| `frontend/` | TypeScript | React UI、路由、主题、SSE 消费、E2E | `frontend/src/main.tsx` | `frontend/CLAUDE.md` |
| `deploy/` | Shell/PowerShell/Python | Nginx gateway、strangler probes、部署脚本 | `deploy-strangler.ps1` | `deploy/strangler-gateway-probes.json` |

---

## 运行与开发

### Strangler 部署

```powershell
.\deploy-strangler.bat
```

常用参数：

```powershell
.\deploy-strangler.bat -NoCache
.\deploy-strangler.bat -FullRestart
.\deploy-strangler.bat -SkipFrontendBuild
.\deploy-strangler.bat -NonInteractive
```

### Rust 后端检查

```powershell
cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "E:/Code/ProjectsCode/WorkSpace/Codex/NovelAi/MuMuNovel/.codex-targets/story-continuity-ledger-owner"
```

### Frozen Python source-map / 工具检查

```powershell
python -X utf8 "backend/tools/check_alembic_revision_health.py"
python -X utf8 "backend/tools/check_text_encoding_health.py"
python -X utf8 -m pytest "backend/tests/test_tools/test_alembic_versioning.py" -q
```

### 前端

```powershell
cd frontend
npm install
npm run dev
npm run build
```

### Compose 验证

```powershell
docker compose -f "docker-compose.yml" config --services
docker compose -f "docker-compose.strangler.yml" config --services
python -X utf8 "backend/tools/run_strangler_gateway_smoke.py" --manifest "deploy/strangler-gateway-probes.json" --validate-manifest-only
```

当前 compose runtime 应只包含：

```text
postgres
db-migrator
rust-backend
nginx
```

---

## 健康检查

Gateway 默认地址：

- `GET http://localhost:8005/health`
- `GET http://localhost:8005/livez`
- `GET http://localhost:8005/readyz`
- `GET http://localhost:8005/health/db-sessions`

Rust backend 容器内部服务端口为 `8001`。不再使用长期运行的 Python HTTP backend。

---

## API 模型获取当前状态

`fetch-models` 已由 Rust settings API 接管：

- Rust route 常量：`backend-rs/src/api/settings.rs`
- Gateway 访问路径：`POST /api/settings/fetch-models`
- Rust 内部 route：`/settings/fetch-models`
- 前端调用：`frontend/src/services/modules/settings.ts`
- UI 组件：`frontend/src/components/ModelInputWithFetch.tsx`
- Settings 页面集成：`frontend/src/components/SettingsCurrentTab.tsx`

旧的 Python `backend/app/api/settings.py`、`backend/app/schemas/settings.py`
和 `backend/test_fetch_models.py` 已不再是当前实现来源。

---

## 编码与协作约定

### Rust runtime

- route handler 保持 transport-oriented，业务语义进入 `backend-rs/src/services/`。
- 章节生成、批量生成、重写、分析、后台任务等迁移工作按 owner/package 收口，不再用微小 Python cleanup 作为主要迁移进度。
- 修改 route、SSE、task lifecycle、checkpoint、provider default、rollback/source-map evidence 时，必须补对应 focused tests 或 smoke/manifest 验证。

### Frozen Python migration source-map

- `backend/migrator_app/` 只保留 Alembic metadata source-map 和测试支撑。
- 生产迁移执行器现在是 Rust `db-migrator`，不要恢复 `backend/app` 或 `app.*` production imports。
- `backend/alembic/postgres/versions/` 仍是历史 source-map 与测试/检查输入，不是生产 deploy entry。
- `backend/scripts/migrate.py` 已删除；手动 Alembic revision 维护直接使用 `alembic -c alembic-postgres.ini revision ...`。
- SQLite Alembic profile 是 legacy/manual，不是 production migrator 默认路径。

### Frontend

- 页面走 `frontend/src/pages/`，复用 UI 放 `frontend/src/components/`。
- API/SSE 客户端优先复用 `frontend/src/services/`。
- Settings 中 API Key、Base URL、生成前网络检索等设置应保存到现有 settings/preset 数据流，不新增平行状态源。

---

## 当前风险与注意事项

- Python runtime 已退出生产 HTTP 路径，但 Python source-map / test-support 仍保留，因此不能宣称 Python-zero。
- 根目录文档只记录当前操作入口；具体 owner 以 `backend-rs/src/api/*`、`backend-rs/src/services/*`、`backend/CLAUDE.md` 和 Trellis task checkpoint 为准。
- 大量工作区变更可能来自并行迁移，禁止随意 revert 未确认来源的修改。
- 前端构建产物和 Nginx gateway 由部署流程管理，不要用旧 Python static serving 假设判断当前架构。

---

## 推荐阅读顺序

1. `.trellis/tasks/05-18-backend-chapter-generation-refactor-followup/implement.md`
2. `backend-rs/src/main.rs`
3. `backend-rs/src/api/settings.rs`
4. `backend-rs/src/api/chapter_generation_routes.rs`
5. `backend/CLAUDE.md`
6. `frontend/CLAUDE.md`
7. `frontend/src/App.tsx`

---

## 相关资源

- 项目仓库：`https://github.com/dyuebug/MuMuNovel`
- Gateway 健康检查：`http://localhost:8005/health`
- Linux DO 讨论帖：`https://linux.do/t/topic/1106333`

---

**最后更新**: 2026-06-25
**文档版本**: 2.0.0
**项目版本**: Rust-owned runtime closeout
