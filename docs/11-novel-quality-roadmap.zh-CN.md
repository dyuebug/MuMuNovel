# 11 - 小说生成质量优化路线图（中文版）

> 对应英文完整版：[`11-novel-quality-roadmap.md`](./11-novel-quality-roadmap.md)

## 目标

这份路线图面向 MuMuNovel 的“优质小说生成”主链路。
目标不是继续堆叠更多 prompt，而是把系统升级为可持续迭代的创作闭环：

1. 生成前，能明确本章真正要完成的叙事任务。
2. 生成中，能稳定继承项目默认偏好、章节阶段和质量策略。
3. 生成后，能通过可量化的质量门禁决定是否需要修复。
4. 多轮生成后，能把人物、设定、关系、伏笔和节奏经验沉淀为长期记忆。

---

## 当前已经具备的基础

### 1. 项目级默认生成偏好已具备统一解析入口
- 已能统一处理 `creative_mode`、`story_focus`、`plot_stage`、`story_creation_brief`、`quality_preset`、`quality_notes`。
- 已解决空字符串污染默认值的问题。
- 参考：`backend/app/services/project_generation_defaults.py:24`

### 2. 后端已有结构化故事指导对象
- `StoryGenerationGuidance` 已能表达创作模式、结构侧重、剧情阶段、创作摘要和质量偏好。
- 说明系统已经从“散乱参数”进化为“结构化生成指导”。
- 参考：`backend/app/services/chapter_quality_context_service.py:27`

### 3. Prompt 层已经支持质量块拼装
- Prompt 组装已经支持创作模式、质量预设、创作总控摘要和修复目标等 block。
- 重生成链路也已支持 `prompt_quality_kwargs`。
- 参考：`backend/app/services/prompt_service.py:396`
- 参考：`backend/app/services/prompt_service.py:6573`

### 4. API 主链路已打通“故事指导 -> Prompt 质量参数 -> 重生成”
- 章节重生成已经会解析故事指导并构造 prompt 质量参数。
- 说明“项目默认值 / 临时覆盖值 / 修复建议”已经能在执行链路中汇合。
- 参考：`backend/app/api/chapters.py:6286`
- 参考：`backend/app/api/chapters.py:6321`

### 5. 前端重生成弹窗已暴露关键质量控制项
- 现在用户可以显式设置创作模式、结构侧重点、剧情阶段、创作总控摘要、质量预设和额外质量要求。
- 这为后续做“高质量重生成模板”打下了 UI 基础。
- 参考：`frontend/src/components/ChapterRegenerationModal.tsx:212`
- 参考：`frontend/src/components/ChapterRegenerationModal.tsx:513`

### 6. 记忆系统已经具备演进空间
- 当前记忆服务已能承载检索式上下文。
- 下一步可以继续演进为人物状态、世界规则、伏笔账本和一致性控制系统。
- 参考：`backend/app/services/memory_service.py:1080`

---

## 当前主要短板

### 短板 A：缺少统一的“创作包”对象
当前参数虽然已经不少，但仍分散在项目默认值、请求体、前端表单和修复建议里。
系统需要一个统一的 `StoryPacket`（或 `GenerationIntent`）对象，至少封装：
- 本章目标
- 本章冲突推进任务
- 本章人物关系变化
- 本章需要兑现/埋设的伏笔
- 本章禁止事项
- 本章质量优先级

### 短板 B：质量评审还没有成为硬性门禁
当前 repair guidance 更像“生成后给建议”。
更理想的闭环应该是：
- 先生成草稿
- 再运行自动验收
- 若不达标则选择修复策略
- 最终只保存达到最低质量门槛的版本

### 短板 C：记忆层还没有成为强一致性约束
优质长篇小说的核心不是字数多，而是人物、动机、规则、因果和回报链保持一致。
建议把记忆分层，而不是把所有上下文平铺进 prompt：
- 世界规则层
- 人物状态层
- 关系变化层
- 伏笔账本层
- 近期章节局部上下文层

### 短板 D：缺少长篇连载级别的运营指标
当前优化仍偏单章视角。长篇小说还需要跟踪：
- 近 10 章主线推进密度
- 人物成长是否停滞
- 伏笔挂账是否积压
- 情绪曲线是否过于单调
- 节奏是否连续失衡

---

## 推荐架构方向

### Layer 1：Project Defaults Layer
- 负责项目级创作偏好和默认质量策略。
- 继续作为统一解析入口。

### Layer 2：Story Packet Layer
- 负责表达“本次生成到底要完成什么”。
- 应成为大纲生成、章节生成、批量生成、重生成的共同输入对象。

### Layer 3：Prompt Assembly Layer
- 负责按模型能力、质量预设、字数目标和修复意图拼装 prompt block。
- 继续保持模块化，避免巨型字符串拼接。

### Layer 4：Quality Gate Layer
- 负责自动验收、评分、失败归类、修复策略选择。
- 这是从“能生成文本”升级到“稳定生成合格章节”的关键层。

### Layer 5：Memory & Consistency Layer
- 负责跨章节一致性、设定追踪、人物状态同步和伏笔管理。
- 应提供分层检索，而不是一股脑塞给模型。

### Layer 6：Feedback Loop Layer
- 负责把人工修订、失败样本和修复结果反哺到默认值、质量规则和 prompt 策略中。
- 让系统从“能写”进化为“越写越稳”。

---

## 分阶段路线图

### Phase 1：统一生成契约（P0）
目标：
- 让大纲生成、章节生成、批量生成、重生成统一到同一套输入契约

建议动作：
- 新增统一 `StoryPacket` / `GenerationIntent`
- 将项目默认值、请求覆盖值和修复建议统一归并到该对象
- 统一前后端字段命名
- 为该对象增加快照日志，便于回溯失败样本

### Phase 2：建立质量门禁（P0）
目标：
- 从“生成完就保存”升级为“通过验收才保存”

建议动作：
- 定义章节最低验收维度：冲突推进、规则落地、大纲贴合、对话自然度、钩子、回报、章末牵引
- 将评分结果按“可自动修复 / 需人工介入 / 允许保存”分层
- 对低分维度自动绑定 repair focus area
- 优先做局部修复，避免整章重写导致风格漂移

### Phase 3：升级记忆系统为一致性系统（P1）
目标：
- 把人物、设定、伏笔和关系变化变成强约束，而不是可选补充

建议动作：
- 将记忆拆分为：世界规则、人物状态、关系状态、伏笔账本、近期章节上下文
- 每章完成后自动抽取结构化状态变更
- 重生成前自动对比“当前章节计划”和“既有状态”
- 为伏笔增加 seeded / pending / paid off / stale 之类的状态机

### Phase 4：构建长篇节奏控制（P1）
目标：
- 不只优化单章，而是优化整本书的节奏与回报结构

建议动作：
- 增加卷级节奏计划
- 追踪近 5~10 章的主线推进密度、情绪波谷波峰、回报兑现率
- 在生成前提示是否出现连续铺垫过长或冲突不足
- 在大纲生成阶段约束章节功能分工

### Phase 5：建立实验与反馈平台（P2）
目标：
- 把质量优化从“靠感觉调 prompt”升级为“有证据地迭代”

建议动作：
- 为不同 `quality_preset`、`creative_mode` 建立 A/B 输出样本库
- 记录人工修改最多的句段类型
- 给失败案例打标签：节奏拖沓、解释过多、钩子不足、OOC、伏笔未兑现等
- 为质量规则和 prompt block 生成回归报告

---

## 未来两周建议优先级

### 第一优先级
- 抽象统一 `StoryPacket`
- 让章节生成、重生成和大纲生成都接到同一输入契约
- 为质量评分增加“失败维度 -> 修复策略”映射表

### 第二优先级
- 给记忆系统增加人物状态快照与伏笔账本
- 增加章节保存前的一致性风险提示

### 第三优先级
- 增加卷级节奏计划
- 输出基础质量趋势 API，供前端可视化使用

---

## 建议监控指标

### 生成质量指标
- 平均章节总分
- 低于验收阈值章节占比
- 自动重生成触发率
- 重生成后二次通过率

### 连载稳定性指标
- 近 10 章主线推进密度
- 未兑现伏笔数量
- 人物状态冲突次数
- 大纲偏移次数

### 产能指标
- 单章平均生成耗时
- 平均重生成次数
- 生成后人工修订字数占比

---

## 建议的下一批工程任务

1. 新增统一 `StoryPacket` schema，并让章节生成与重生成共用。
2. 给 `chapter_quality_context_service` 增加“验收失败 -> 修复策略”映射。
3. 为 `memory_service` 增加人物状态与伏笔状态的结构化写入接口。
4. 为大纲和章节生成增加卷级节奏计划输入。
5. 输出章节质量趋势 API，供前端质量面板使用。

---

## 关键参考

- `backend/app/services/project_generation_defaults.py:24`
- `backend/app/services/chapter_quality_context_service.py:27`
- `backend/app/services/prompt_service.py:396`
- `backend/app/services/prompt_service.py:6573`
- `backend/app/api/outlines.py:1824`
- `backend/app/api/chapters.py:6286`
- `backend/app/services/memory_service.py:1080`
- `frontend/src/components/ChapterRegenerationModal.tsx:212`
- `frontend/src/components/ChapterRegenerationModal.tsx:513`