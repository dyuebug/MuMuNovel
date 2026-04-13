# 12 - 小说质量运行时架构（Runtime Quality Architecture）

## 文档目标

这份文档聚焦“章节生成在运行时如何保证质量”，回答三个问题：

1. 当前 MuMuNovel 已经落地了哪些质量护栏。
2. 为什么这些护栏能提升长篇小说生成的稳定性与成稿质量。
3. 下一阶段应该怎样继续把“能生成”升级为“稳定生成优质内容”。

---

## 一句话结论

MuMuNovel 当前已经从“生成完立即保存”升级为“先生成候选稿 → 做质量判定 → 通过后才保存正文”。

这意味着系统不再把低质量候选稿直接写进章节与历史记录，而是把它们作为分析输入，交给后续修复链路或人工审阅链路处理。这个变化，是提升长篇小说稳定性的关键分水岭。

---

## 当前运行时链路

### 1. 输入归并层

运行时会先把以下信息归并进生成上下文：

- 项目默认创作参数
- 请求级覆盖参数
- 写作风格与质量画像
- 故事修复建议（story repair payload）
- 章节上下文、伏笔提醒、人物与职业信息

当前这部分逻辑已经在章节生成、批量生成和重生成中逐步统一，但还没有完全收口为一个全局唯一的输入契约对象。

**当前价值**：
- 降低前后端字段漂移
- 降低不同入口生成风格不一致
- 为后续统一 `StoryPacket` 铺路
- 为后续把运行时质量契约整体收口进 `StoryPacket` 做准备

---

### 2. Candidate Draft 层

章节生成现在分成两个阶段：

1. 生成 candidate：模型先产出候选正文
2. 决定是否 apply：只有质量门禁放行，才会真正写入 `Chapter.content`

当前已实现的关键规则：

- 单章流式生成：质量门禁未通过时，不保存正文
- 批量/后台生成：质量门禁未通过时，不保存正文
- 质量门禁阻断时，candidate 只作为分析输入，不作为正式成稿

**当前价值**：
- 避免低质量文本污染章节正文
- 避免项目总字数、章节状态与正文内容失真
- 避免把失败样本误当成“已完成章节”继续向后传播

---

### 3. Quality Gate 层

当前章节质量门禁已经具备基础验收能力，核心会检查：

- overall score
- 冲突推进
- 世界规则落地
- 大纲贴合度
- 对话自然度
- 开篇钩子
- 回报兑现
- 章末牵引 / cliffhanger
- 节奏分布

门禁结果会进入三种动作之一：

- `continue`：允许保存正文
- `retry`：允许自动修复后重试
- `manual_review`：阻断保存，进入人工介入或后续分析

**当前价值**：
- 把模型输出质量转化为程序化决策
- 把“重试”从随机重复生成升级为有 repair guidance 的受控重试
- 为后续质量趋势统计提供统一出口

---

### 4. Persistence 层

当前持久化策略已经从“先保存再补救”改成“通过验收才落正式数据”。

#### 当前正式保存的数据

只有在 `content_applied=True` 时才会写入：

- `Chapter.content`
- `Chapter.word_count`
- `Chapter.status = completed`
- `Project.current_words`
- 正文型 `GenerationHistory`
- 自动埋入的伏笔结果

#### 当前不会保存的数据

当质量门禁要求 follow-up 时，不再保存：

- candidate 正文到 `Chapter.content`
- candidate 正文到正文型 `GenerationHistory`
- 已完成章节状态
- 已增长的项目字数

**当前价值**：
- 数据库里的“正式章节”与“已验收章节”语义一致
- `GenerationHistory` 不再混入未通过门禁的正文候选稿
- 后续统计、导出、再编辑不会误读失败草稿

---

### 5. Analysis / Repair 层

当 `quality_gate` 返回 `retry` 或 `manual_review` 时，系统不会丢弃 candidate，而是把它作为分析链路的输入：

- `chapter_content_override`
- `chapter_word_count_override`

这让分析器看到的是刚刚生成但尚未保存的候选稿，而不是旧的章节正文。

**当前价值**：
- 分析结论更贴近真实失败样本
- repair guidance 可以直接针对最新 candidate 生成
- 背景任务、批量任务、单章流式任务的 follow-up 行为开始趋于一致

---

### 6. History / Audit 层

当前 `GenerationHistory` 已经更严格地区分：

- 正式成稿历史：只有 apply 后才写
- 检查器 / 修订器 / 分析链路历史：继续按各自职责记录

这意味着 `GenerationHistory` 更接近“已接受输出历史”，而不是“所有候选尝试的垃圾桶”。

**当前价值**：
- 历史记录更适合做复盘与回归分析
- 下游 UI、质量趋势与导出逻辑更容易保持一致
- 减少把失败 candidate 误当成优质样本继续学习或展示

---

## Candidate 链路分层（2026-04）

本轮重构后，candidate 相关逻辑已经从 `chapters.py` 逐步下沉到多个 service，API 层更多只承担组装与兼容职责。

### 已下沉的阶段职责

- `chapter_candidate_generation_service`：candidate 池生成与 rerank retry
- `chapter_candidate_word_budget_repair_service`：字数 repair candidate
- `chapter_candidate_targeted_final_repair_service`：定向质量 repair candidate
- `chapter_candidate_finalize_service`：winner 决策、metadata 附着、最终 runtime state 收口
- `chapter_candidate_executor_service`：串联整个 candidate workflow
- `chapter_candidate_runtime_state_service` / `view_service` / `result_service` / `classification_service` / `event_service`：提供运行时状态、只读视图、结果规范化、repair 分类与 event builder
- `chapter_candidate_record_service`：负责单个 candidate 记录的清洗、quality gate plan 规范化与 selection metadata 二次装配

### `chapters.py` 仍保留的本地 hook

- `_collect_generation_candidate_output(...)`：紧贴 `AIService.generate_text_stream(...)` 的 chunk 循环与流式 runtime state 更新
- `_resolve_generation_attempt_labels(...)`：保留 generation path / attempt kind 命名语义
- `_sync_generation_runtime_state(...)`：已退化为 `chapter_candidate_runtime_state_service` 的兼容 wrapper
- `_build_generation_candidate_record(...)`：已退化为 `chapter_candidate_record_service` 的兼容 wrapper
- `_get_chapter_candidate_executor_dependencies()`：保留 default wiring 入口
- `_generate_best_ranked_candidate(...)`：保留旧签名兼容，内部转调 executor workflow

### 下一步优化方向

- `_collect_generation_candidate_output(...)` 仍贴近 API：若单章流式和批量流式 contract 完全统一，可考虑继续下沉
- `chapters.py` 当前主要承担兼容 facade / monkeypatch seam：后续应优先把新增逻辑继续下沉到 service，而不是按“文件内未使用”机械删除现有 wrapper
- candidate 仍以运行时对象为主：若后续要做失败样本平台与 repair 统计，可考虑 candidate / repair attempt 生命周期记录

---
