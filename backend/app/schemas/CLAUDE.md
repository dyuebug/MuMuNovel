# Schemas 子模块文档

[根目录](../../CLAUDE.md) > [backend](../CLAUDE.md) > **app/schemas**

---

## 变更记录

### 2026-04-20
- 新增 `app/schemas` 子模块文档
- 基于当前 Pydantic schema 文件、导入边界与 API 调用关系整理分层
- 标注章节生成请求、设置配置、质量载荷与导入导出模型的职责边界

---

## 模块职责

`backend/app/schemas/` 定义后端 API 与服务层之间的 Pydantic 数据模型，负责：
- 约束请求体与响应体结构
- 为路由层提供输入校验与输出序列化
- 沉淀章节生成、设置、质量、导入导出等领域的共享数据契约
- 承接部分归一化与反序列化逻辑

原则：schema 聚焦“数据契约”，不要把业务流程、数据库读写或复杂编排塞进这里。

---

## 真实入口关系

- 路由总入口：`backend/app/main.py`
- 典型路由使用：
  - `backend/app/api/settings.py` → `SettingsCreate` / `SettingsUpdate` / `SettingsResponse`
  - `backend/app/api/chapter_generation_routes.py` → `ChapterGenerateRequest`
  - `backend/app/api/prompt_workshop.py` → `ImportRequest` / `PromptSubmissionCreate` / `ReviewRequest`
- 服务层与 schema 的交汇点主要在：
  - 路由入参校验
  - ORM → 响应模型序列化
  - 质量/修复 payload 的结构化归一化

说明：`schemas/` 不是单纯“接口 DTO 目录”，其中还包含质量载荷标准化与导入导出数据契约。

---

## 当前 schema 分组

### 1. 核心内容实体
- `project.py`
- `chapter.py`
- `outline.py`
- `character.py`
- `relationship.py`
- `career.py`
- `foreshadow.py`
- `writing_style.py`
- `prompt_template.py`
- `mcp_plugin.py`

关键点：
- 这组文件定义常规 CRUD 与列表响应模型，是 API 的基础数据契约
- `chapter.py` 不只含基础章节模型，还承载生成、批量生成、分析状态、质量趋势等复合响应结构
- 领域主实体的 schema 往往同时被路由和服务返回值复用

### 2. 章节生成、重写与生成偏好
- `chapter.py`
- `regeneration.py`
- `generation_preferences.py`
- `generation_payload.py`
- `polish.py`

关键点：
- `ChapterGenerateRequest`、`BatchGenerateRequest` 位于 `chapter.py`，而不是拆到独立 generation 文件
- `generation_preferences.py` 提供 `CreativeModeValue`、`StoryFocusValue`、`PlotStageValue`、`QualityPresetValue` 等 Literal 类型，以及统一归一化函数
- `chapter.py` 通过 `field_validator` 复用 `normalize_optional_choice()`、`normalize_optional_text()`，把枚举化偏好与自由文本标准化
- 生成相关 schema 与章节路由、批量生成、重写链路强耦合，字段调整必须同步前后端

### 3. 设置与平台配置
- `settings.py`
- `mcp_plugin.py`

关键点：
- `settings.py` 同时覆盖用户设置、API 端点配置、预设管理与部分 JSON 反序列化逻辑
- `SettingsResponse` 内部会把数据库中的 `api_backup_urls` JSON 字符串转回 `List[str]`
- 设置 schema 直接服务 `backend/app/api/settings.py`，影响 AIService 配置读取、连接测试与前端设置页

### 4. 质量、修复与运行态载荷
- `quality.py`
- `regeneration.py`
- `chapter.py`

关键点：
- `quality.py` 是高复杂度 schema 文件，定义 `StoryRepairGuidance`、`StoryQualityGateDecision`、`ChapterQualityMetricsSummary`、`ActiveStoryRepairPayload` 等嵌套载荷
- `quality.py` 还提供 `normalize_story_*` 系列函数，把映射结构安全转换为强类型模型
- 这组 schema 不只是接口出参，也承担服务层内部“结构化质量载荷”的统一边界
- `chapter.py` 引用了 `quality.py` 中多个模型，用于项目章节质量趋势与分析状态输出

### 5. 导入导出与拆书相关
- `import_export.py`
- `book_import.py`

关键点：
- `import_export.py` 覆盖项目导出的大量嵌套结构，如章节、角色、大纲、关系、组织、记忆、剧情分析、写作风格等
- 这类 schema 更接近“文件交换契约”，字段通常需要兼顾历史兼容与跨实体关联
- 一些导出字段并非数据库原字段，而是为导入重建关系服务，例如 `outline_title`、`character_name`、`style_name`

### 6. 提示词工坊与外部协作
- `prompt_workshop.py`
- `prompt_template.py`

关键点：
- `prompt_workshop.py` 明确区分导入、下载、投稿、审核、管理员创建/更新、条目响应、投稿响应等多类契约
- 工坊相关 schema 同时服务本地实例与云端/代理调用场景，字段设计要兼顾实例标识与用户标识

---

## 导出与复用现状

- `backend/app/schemas/__init__.py` 当前几乎为空，不承担统一导出职责
- 大多数路由直接从具体文件导入 schema，例如：
  - `app.schemas.chapter import ChapterGenerateRequest`
  - `app.schemas.settings import SettingsResponse`
  - `app.schemas.prompt_workshop import PromptSubmissionCreate`
- 因此新增 schema 后，通常不需要同步维护集中导出，但要注意跨文件导入路径是否稳定

---

## 关键事实

- `chapter.py` 是 schema 层的核心聚合文件之一，不仅含 CRUD，还含生成、批量生成、分析与质量趋势结构
- `settings.py` 不只是用户设置表单模型，还承担端点预设与部分反序列化逻辑
- `quality.py` 本质上是“高嵌套运行态载荷契约层”，影响章节分析、修复建议、质量门禁与趋势展示
- `generation_preferences.py` 是多个生成请求共享的偏好类型与归一化基础设施
- `schemas/__init__.py` 当前不是统一出口，阅读调用链时应优先看具体 schema 文件

---

## 开发约定

- 改 schema 前先追踪：API route → service → frontend types / 请求体 → 测试
- 若新增请求字段，必须明确默认值、可空语义、前后端兼容策略
- 若 schema 被多个生成链路共用，优先抽到共享文件而不是复制字段
- 在 schema 层允许做轻量归一化、反序列化和字段约束，但不要放业务判断或数据库访问
- 修改 `quality.py`、`chapter.py`、`settings.py` 这类高耦合 schema 时，必须联查相关 API 与前端消费方

---

## 风险与注意事项

- `chapter.py`、`quality.py`、`settings.py` 的变更影响面很大，容易引发 API/前端/测试连锁回归
- 一些导出导入 schema 为兼容历史数据保留了非最简字段，不要轻易按“看起来冗余”删除
- 质量载荷模型嵌套层级深，字段改名可能破坏服务层标准化函数与前端展示逻辑
- `schemas/__init__.py` 不提供统一出口，盲目改导入路径容易造成循环依赖或遗漏更新

---

## 推荐阅读

1. `backend/app/schemas/chapter.py`
2. `backend/app/schemas/quality.py`
3. `backend/app/schemas/settings.py`
4. `backend/app/schemas/generation_preferences.py`
5. `backend/app/schemas/import_export.py`
6. `backend/app/api/chapter_generation_routes.py`

---

**最后更新**: 2026-04-20
**模块版本**: 1.3.9
