# Models 子模块文档

[根目录](../../CLAUDE.md) > [backend](../CLAUDE.md) > **app/models**

---

## 变更记录

### 2026-04-20
- 新增 `app/models` 子模块文档
- 基于当前 SQLAlchemy 模型导出与文件结构整理分组
- 标注用户隔离、任务模型与工坊模型的注意事项

---

## 模块职责

`backend/app/models/` 定义后端核心 SQLAlchemy ORM 模型，负责：
- 持久化小说项目与内容实体
- 持久化用户、设置、MCP 插件与认证关联数据
- 持久化章节生成、分析、重写、批量任务等运行记录
- 为 API / services / Alembic 提供统一数据结构基础

---

## 真实入口关系

- 统一导出文件：`backend/app/models/__init__.py`
- 数据库基类：`app.database.Base`
- 路由与服务层通过这些模型进行查询、更新与事务提交
- Alembic 迁移应与模型变更保持同步

---

## 当前模型分组

### 1. 用户与平台配置
- `user.py` - `User`, `UserPassword`
- `settings.py` - `Settings`
- `mcp_plugin.py` - `MCPPlugin`

关键点：
- `Settings.user_id` 唯一，用于每个用户一份 AI 配置
- `UserPassword` 与本地登录流程有关，不能只看 `User`

### 2. 项目与内容主实体
- `project.py` - `Project`
- `outline.py` - `Outline`
- `chapter.py` - `Chapter`
- `character.py` - `Character`
- `relationship.py` - `CharacterRelationship`, `Organization`, `OrganizationMember`, `RelationshipType`
- `career.py` - `Career`, `CharacterCareer`
- `writing_style.py` - `WritingStyle`
- `project_default_style.py` - `ProjectDefaultStyle`
- `foreshadow.py` - `Foreshadow`

关键点：
- `Project.user_id` 是用户隔离关键字段
- `Chapter` 通过 `project_id` 归属项目，通过 `outline_id` 支持一对多大纲关系
- `Project.outline_mode` 受约束：`one-to-one` / `one-to-many`

### 3. 记忆与分析
- `memory.py` - `StoryMemory`, `PlotAnalysis`
- `generation_history.py` - `GenerationHistory`

关键点：
- 这组模型支撑长期记忆、剧情分析、生成历史追踪
- 一些“记忆能力”并不只依赖数据库，也依赖 Chroma 等外部存储，但数据库模型仍是元数据入口

### 4. 任务与运行态
- `analysis_task.py` - `AnalysisTask`
- `regeneration_task.py` - `RegenerationTask`
- `batch_generation_task.py` - `BatchGenerationTask`
- `batch_generation_snapshot.py` - `BatchGenerationSnapshot`
- `chapter_draft_attempt.py` - `ChapterDraftAttempt`

关键点：
- `BatchGenerationTask` 记录批量生成总体状态、失败章节、当前章节、重试次数
- `ChapterDraftAttempt` / `BatchGenerationSnapshot` 用于生成过程细粒度追踪
- 改这些模型时，要连带检查后台任务恢复与状态查询链路

### 5. 提示词资产
- `prompt_template.py` - `PromptTemplate`
- `prompt_workshop.py` - `PromptWorkshopItem`, `PromptSubmission`, `PromptWorkshopLike`

关键点：
- 工坊模型区分公开条目、待审核投稿、点赞记录三层
- 工坊相关用户标识常使用跨实例标识，不总是本地 `user_id`

---

## 导出清单现状

`__init__.py` 当前导出了以下主要对象：
- `Project`, `Outline`, `Chapter`, `Character`
- `CharacterRelationship`, `Organization`, `OrganizationMember`, `RelationshipType`
- `GenerationHistory`, `ChapterDraftAttempt`, `BatchGenerationSnapshot`
- `AnalysisTask`, `BatchGenerationTask`, `RegenerationTask`
- `Settings`, `StoryMemory`, `PlotAnalysis`
- `WritingStyle`, `ProjectDefaultStyle`, `MCPPlugin`
- `User`, `UserPassword`
- `Career`, `CharacterCareer`
- `PromptTemplate`, `Foreshadow`
- `PromptWorkshopItem`, `PromptSubmission`, `PromptWorkshopLike`

新增模型后，如果希望被外部统一导入，需同步更新 `__init__.py`。

---

## 关键事实

- 模型文件并不只覆盖“业务实体”，也承载大量任务运行态与配置态
- 用户隔离通常不是每张表都有 `user_id`，有些表通过 `project_id` 间接隔离
- `Settings`、`BatchGenerationTask`、`PromptWorkshop*` 都属于高耦合模型，字段变更影响面大
- `PromptWorkshopLike` 对 `(user_identifier, workshop_item_id)` 做了唯一索引

---

## 开发约定

- 改模型字段前，先追踪：API schema → service 查询/写入 → 测试 → Alembic
- 新增字段优先考虑默认值、空值兼容和历史数据迁移
- 用户相关数据必须明确隔离策略：直接 `user_id` 还是经 `project_id` 间接隔离
- 不要把纯运行时计算塞进模型；模型聚焦持久化结构
- 新模型若会被跨模块广泛导入，应补到 `__init__.py`

---

## 风险与注意事项

- 改 `Project`、`Chapter`、`Settings`、`BatchGenerationTask` 等核心模型会波及大量服务与测试
- 任务模型与后台恢复逻辑耦合，字段改名容易造成恢复失败或查询异常
- 工坊与本地用户体系不是完全同构，误把 `user_identifier` 当本地 `user_id` 会出错
- 迁移脚本目录分 PostgreSQL / SQLite 两套，结构变更后需同时关注

---

## 推荐阅读

1. `backend/app/models/__init__.py`
2. `backend/app/models/project.py`
3. `backend/app/models/chapter.py`
4. `backend/app/models/settings.py`
5. `backend/app/models/batch_generation_task.py`
6. `backend/app/models/prompt_workshop.py`

---

**最后更新**: 2026-04-20
**模块版本**: 1.3.9
