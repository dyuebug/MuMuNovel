# API 子模块文档

[根目录](../../CLAUDE.md) > [backend](../CLAUDE.md) > **app/api**

---

## 变更记录

### 2026-04-20
- 新增 `app/api` 子模块文档
- 基于当前 FastAPI 路由注册情况整理分组
- 标注章节拆分路由、后台任务入口与协作边界

---

## 模块职责

`backend/app/api/` 负责 HTTP / SSE 接口暴露与请求编排：
- 定义 `APIRouter`
- 解析请求参数、认证态与依赖注入
- 调用 `services/` 完成业务逻辑
- 返回 JSON / SSE / 文件等响应
- 保持路由层薄，避免堆积复杂业务逻辑

---

## 真实入口关系

- 路由注册总入口：`backend/app/main.py`
- 通用辅助：`common.py`、`chapter_route_helpers.py`
- 认证入口：`auth.py`
- 设置入口：`settings.py`
- 后台任务入口：`background_tasks.py`
- 章节域是多文件拆分，不是单文件模块

---

## 文件分组

### 平台与账户
- `auth.py` - 本地登录、LinuxDO OAuth、会话 Cookie
- `users.py` - 用户接口
- `admin.py` - 管理员接口
- `settings.py` - 用户设置、AI 模型、连接测试、环境默认值
- `mcp_plugins.py` - MCP 插件管理
- `changelog.py` - 更新日志

### 项目与内容管理
- `projects.py`
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
- `book_import.py`
- `polish.py`

### 工作流与生成
- `wizard_stream.py` - 智能向导 SSE
- `inspiration.py` - 灵感模式、恢复、研究透传
- `background_tasks.py` - 长任务创建、查询、恢复、去重

### 章节域拆分路由
- `chapters.py`
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
- `chapter_route_helpers.py`

说明：章节相关改动必须先确认自己落点属于 CRUD、生成、批量生成、分析还是兼容/辅助层，避免改错入口。

---

## 常见依赖模式

### 数据库与认证
常见依赖：
- `Depends(get_db)`
- `Request`
- `require_authenticated_user_id()`
- `verify_project_access()`
- `load_accessible_chapter_or_404()`

### 服务层调用
路由通常只做三件事：
1. 校验用户与资源访问权限
2. 组装 service 所需参数
3. 把 service 结果转成响应模型或 SSE 输出

---

## 关键事实

- `background_tasks.py` 使用独立 `APIRouter(prefix="/background-tasks")`
- `auth.py` 使用 `APIRouter(prefix="/auth")`
- 章节 CRUD 在 `chapter_crud_routes.py` 中已有完整增删改查实现
- `settings.py` 不只处理 UI 设置，也参与 AIService 配置与默认值读取
- 有些路由文件本身已自带 `/api` 前缀，有些由 `main.py` 统一加 `prefix="/api"`；改前先核对注册方式

---

## 开发约定

- 新端点优先追加到语义最接近的现有文件，除非已形成独立子域
- 共享校验逻辑优先抽到 `common.py` 或 `chapter_route_helpers.py`
- 不要在路由层复制服务逻辑；发现重复应回收进 `services/`
- 变更路由前缀、请求模型或返回字段时，必须追踪前端 `src/services/api.ts` 与相关 E2E
- SSE / 后台任务接口改动时，必须同步检查任务恢复与进度消费链路

---

## 风险与注意事项

- 章节路由文件数量多，命名相近，误改概率高
- 部分接口承担“兼容旧前端调用”的职责，删除前必须确认调用方已迁移
- 后台任务接口与 Settings/AIService 紧耦合，配置字段变更会影响创建与恢复流程
- 用户隔离依赖 `request.state.user_id` 相关辅助函数，绕过它们容易引入越权问题

---

## 推荐阅读

1. `backend/app/main.py`
2. `backend/app/api/common.py`
3. `backend/app/api/chapter_route_helpers.py`
4. `backend/app/api/background_tasks.py`
5. `backend/app/api/settings.py`
6. 当前任务相关 route 文件

---

**最后更新**: 2026-04-20
**模块版本**: 1.3.9
