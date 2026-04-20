# Services 子模块文档

[根目录](../../CLAUDE.md) > [backend](../CLAUDE.md) > **app/services**

---

## 变更记录

### 2026-04-20
- 新增 `app/services` 子模块文档
- 按当前服务命名与调用方式梳理分层
- 标注章节生成、批量生成、兼容层与 AI 基础设施边界

---

## 模块职责

`backend/app/services/` 是后端业务逻辑主层，负责：
- AI 客户端与 Provider 抽象
- 章节生成、重写、分析、批量生成编排
- 后台任务状态持久化与流式事件消费
- 记忆、质量评估、剧情连续性、风格同步等核心业务
- 兼容层门面，支撑渐进式重构

原则：路由层只编排，复杂逻辑应下沉到这里。

---

## 主要分层

### 1. AI 基础设施层
- `ai_service.py` - 统一 AIService 门面，屏蔽 provider 差异，并支持自动 MCP 工具加载
- `ai_config.py` - AI 客户端配置
- `ai_clients/` - OpenAI / Anthropic / Gemini 原始客户端
- `ai_providers/` - Provider 适配层
- `mcp_tools_loader.py` - MCP 工具加载

### 2. 后台任务层
- `background_task_manager.py` - 任务记录、SSE 消费、磁盘持久化、checkpoint
- `background_task_wizard_executor.py` - 向导类后台任务执行
- `task_workflow_runtime_*` / `analysis_task_*` / `regeneration_task_*` - 任务运行态与状态查询

### 3. 章节生成域
核心命名族：
- `chapter_generation_*`
- `chapter_candidate_*`
- `chapter_generated_text_*`
- `chapter_regeneration_*`
- `chapter_analysis_*`
- `chapter_context_service.py`
- `chapter_quality_context_service.py`
- `chapter_web_research_service.py`
- `chapter_content_apply_service.py`

说明：这一层已经被拆成“入口 → wiring → runtime → candidate → finalize/repair”等多个小服务，改动前必须定位自己所处阶段。

### 4. 批量生成域
核心命名族：
- `batch_generation_create_service.py`
- `batch_generation_stream_service.py`
- `batch_generation_execution_service.py`
- `batch_generation_runtime_service.py`
- `batch_generation_workflow_service.py`
- `batch_generation_retry_service.py`
- `batch_generation_candidate_service.py`
- `batch_generation_chapter_*`
- `batch_generation_status_service.py`
- `batch_generation_resume_service.py`
- `batch_generation_orchestration_service.py`

说明：`batch_generation_execution_service.py` 现在更像 facade / re-export 门面，真正实现分散在多个 `batch_generation_*` 文件中。

### 5. 内容与设定域
- `memory_service.py`, `memory_ranking.py`
- `foreshadow_service.py`
- `plot_analyzer.py`, `plot_expansion_service.py`
- `project_continuity_ledger_service.py`
- `project_generation_defaults.py`
- `writing_style_sync_service.py`
- `prompt_service.py`, `prompt_template_sync_service.py`
- `book_import_service.py`, `txt_parser_service.py`
- `auto_character_service.py`, `auto_organization_service.py`

### 6. 质量与评估域
- `novel_quality_profile_service.py`
- `novel_quality_rules.py`
- `story_quality_repair_effectiveness_service.py`
- `story_runtime_serialization_service.py`
- `story_repair_payload_service.py`
- `project_quality_trend_service.py`
- `project_quality_trend_query_service.py`
- `project_quality_trend_snapshot_store.py`
- `outline_quality_summary_snapshot_store.py`
- `task_quality_snapshot_service.py`

### 7. 集成与外部能力
- `oauth_service.py`
- `workshop_client.py`
- `grok_search_adapter.py`
- `grok_search_embedded.py`
- `mcp_test_service.py`

### 8. 兼容层
常见命名：`*_compat_service.py`
例如：
- `chapter_prompt_quality_compat_service.py`
- `project_quality_trend_compat_service.py`
- `batch_generation_entry_compat_service.py`
- `batch_generation_run_compat_service.py`
- `chapter_candidate_executor_compat_service.py`
- `task_workflow_runtime_compat_service.py`

这些文件通常只是转发到新服务，不应继续堆积新业务逻辑，除非正在做过渡封装。

---

## 关键事实

- `AIService` 统一管理 provider 路由与自动 MCP 工具装配
- `background_task_manager.py` 会把任务运行态持久化到 `data/runtime/background_tasks.json`
- `memory_service.py` 仍保留全局实例 `memory_service`
- 章节与批量生成服务使用大量 facade / wiring / runtime / candidate / finalize 拆分
- 重构过程中存在大量 re-export 与 compat 文件；“文件名像入口”不代表“这里有真实实现”

---

## 开发约定

- 先找真正实现文件，再改 facade 或 compat 层
- 新逻辑优先落在职责明确的小服务，不要继续把大门面做胖
- 命名保持语义化：`*_entry_service`、`*_wiring_service`、`*_runtime_service`、`*_compat_service`
- 涉及流式任务、重试、候选稿、质量修复的改动，要连带检查上下游状态对象
- 若修改服务函数签名，必须追踪 API route、background task executor、tests 三端调用链

---

## 风险与注意事项

- `services/` 中文件数量多、名字相似，最常见问题是改到兼容层或 facade 而非真实实现
- 批量生成和单章生成共享很多状态模型与质量逻辑，局部改动容易产生回归
- 后台任务、SSE、恢复逻辑跨多个服务文件协同，单点修改必须补看全链路
- 全局实例服务（如 `memory_service`）和持久化管理器（如 `background_task_manager`）带状态，测试时要注意副作用

---

## 推荐阅读

1. `backend/app/services/ai_service.py`
2. `backend/app/services/background_task_manager.py`
3. `backend/app/services/chapter_generation_stream_entry_service.py`
4. `backend/app/services/batch_generation_execution_service.py`
5. `backend/app/services/chapter_prompt_quality_compat_service.py`
6. 当前任务相关命名族的真实实现文件

---

**最后更新**: 2026-04-20
**模块版本**: 1.3.9
