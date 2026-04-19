# 章节 API Gateway / Seam 分层说明

## 背景

为了降低 `backend/app/api/chapters.py` 的职责耦合，项目已逐步将章节 API 按 route 层、gateway/seam 层和 compat/service 层进行拆分。

本文档用于说明什么逻辑应该放在哪一层，以及后续如何继续将变更从 `chapters.py` 中拆离。

## 分层原则

1. route 层只负责 HTTP 入口、参数解析、权限校验和 response 组装
2. gateway / seam 层用于暴露稳定的 monkeypatch 入口与 facade
3. compat / service 层承载可复用的领域逻辑，避免 route 中堆积业务细节

## 当前结构

### 1. route 层

下列文件以 FastAPI route 为主：
- `backend/app/api/chapter_crud_routes.py`
- `backend/app/api/chapter_generation_routes.py`
- `backend/app/api/chapter_batch_generation_routes.py`
- `backend/app/api/chapter_quality_routes.py`
- `backend/app/api/chapter_analysis_routes.py`
- `backend/app/api/chapter_analysis_task_routes.py`
- `backend/app/api/chapter_partial_regeneration_routes.py`
- `backend/app/api/chapter_regeneration_routes.py`
- `backend/app/api/chapter_draft_routes.py`
- `backend/app/api/chapter_annotation_routes.py`
- `backend/app/api/chapter_expansion_plan_routes.py`

这些文件应优先保持轻量，主要处理路由声明、request/response schema 绑定与 dependency injection。

### 2. gateway / seam 层

`backend/app/api/chapters.py` 现在更接近 gateway / seam 聚合层，主要负责对外暴露调用入口与保留历史 seam。

常见 facade / seam 包括：
- batch generation facade
- single chapter generation facade
- candidate 选择 / rerank seam
- runtime/cache facade
- prompt/text facade

这一层的价值在于：
- 保留 `chapters_api.*` monkeypatch seam
- 让 route 层不直接依赖大量业务组装逻辑
- 让 wiring 口更集中、更容易迁移

### 3. compat / service 层

当前与 `chapters.py` 拆分相关的 compat/service 文件包括：
- `backend/app/services/chapter_candidate_entry_compat_service.py`
- `backend/app/services/chapter_candidate_executor_compat_service.py`
- `backend/app/services/chapter_generated_text_compat_service.py`
- `backend/app/services/chapter_prompt_quality_compat_service.py`
- `backend/app/services/task_workflow_runtime_compat_service.py`
- `backend/app/services/batch_generation_entry_compat_service.py`
- `backend/app/services/batch_generation_run_compat_service.py`
- `backend/app/services/project_quality_trend_compat_service.py`

这些 service 更适合承载：
- 章节生成、重写、批量生成的组装逻辑
- quality gate、runtime snapshot、prompt 与 artifact 协作
- 可单测试的纯逻辑单元

## 迁移策略

对 `chapters.py` 的后续调整建议按以下顺序进行：
1. 先把 route 依赖的业务装配移入 facade 或 wrapper
2. 再把稳定的领域逻辑下沉到 compat/service
3. 在测试和调用方未完全迁移前，保留 `chapters_api.*` seam
4. 最后再收缩 `chapters.py` 中仅作中转的 wrapper

## 测试要点

迁移过程中建议持续检查：
- `chapters_api.*` monkeypatch seam 是否仍可用
- `chapters.router` 对外路由是否仍稳定
- `_split_sentences` 、runtime snapshot sentinel 等历史辅助逻辑是否仍被正确维持
- candidate entry/runtime compat 类逻辑是否有单测试覆盖

## 风险与注意事项

1. 不要一次性删掉所有 wrapper，否则容易引发测试断裂
2. 不要让 route 文件再回流新的重业务逻辑
3. 与 batch generation、candidate 评估、quality gate 相关变更要同步回归

## 建议的下一步

1. 列出 `chapters.py` 中仍只做中转的入口
2. 为每个 facade 标注对应 service 归属
3. 在不破坏 seam 的前提下，继续把可纯化的逻辑移入 service
