# Design: 章节自动返修质量重试收敛

## Problem Statement

当前 durable Novel Autopilot 的章节返修会从已接受章节和对应分析生成候选。候选若再次得到 `retry`，适配器仅持久化通用错误码和计数，候选正文、digest、质量指标及质量消息全部丢失。下一次尝试再次从同一已接受正文和同一分析开始，无法利用上一轮反馈收敛；预算耗尽时也没有把最后一个有效候选交给人工复核。

## Design Goals

- 保持正式章节只在质量通过时原子更新。
- 在现有数据结构内耐久保存质量失败候选，不新增迁移。
- 后续返修只消费同一 Autopilot run/epoch、同一源正文和同一分析产生的最近失败候选。
- 让时间线和任务结果暴露安全的失败摘要，而不是只有通用错误码。
- 保持现有重试预算和并发 fence 语义。

## Architecture And Ownership

### Candidate Generation

`chapter_repair_generation_service` 继续负责加载权威章节、匹配 `source_content_digest` 的分析和生成返修候选。新增一个受作用域约束的最近失败候选读取步骤：

- 只查询 `chapter_draft_attempts.source = novel_autopilot_chapter_repair`；
- 校验 `repair_payload` 中的 `run_id`、`run_epoch`、`source_content_digest`、`analysis_id`；
- 使用现有 `extract_candidate_draft_full_content` 读取完整候选；
- 将该候选作为下一次返修基线，并把最近质量消息/指标合并到原分析返修目标；
- 任一作用域或完整性校验失败时，安全回退到已接受章节和已提交分析。

生成服务仍返回 typed `ChapterRepairCandidate`，但补充用于失败持久化的最小字段：候选 digest、质量指标、质量消息及构建 draft attempt 所需的信息。

### Failure Persistence

`chapter_repair_repository` 扩展质量失败提交契约，在同一数据库事务中：

1. 校验 run/version/epoch/step/background task/chapter snapshot fence；
2. 插入现有 `chapter_draft_attempts` 失败候选行；
3. 将 step 标记为失败，写入 `result_digest`、质量决策和错误码；
4. 更新 run 的质量失败计数、当前步骤和终态路由。

候选行使用：

- `source = novel_autopilot_chapter_repair`；
- `attempt_state = retry`；
- `batch_task_id = null`，避免违反批量任务外键；
- `repair_payload` 保存 Autopilot 作用域、源正文 digest、分析 ID、完整候选和安全质量上下文；
- 不保存 prompt、API key、Authorization、模型 endpoint 或供应商原始载荷。

### Retry And Manual Review Routing

- 预算内 `retry`/`auto_repair`：保存失败候选并返回 `retry_scheduled`。
- 候选质量动作直接为 `manual_review`：沿用现有人工复核候选持久化路径。
- `retry` 到达预算上限：保存最后候选为人工复核候选，返回 `chapter_repair_manual_review`，不再创建额外返修步骤。
- `continue`/`allow_save`：沿用现有原子章节提交路径，并清零连续质量失败计数。
- provider/cancel/stale/business-data-changed：保持现有行为，不伪造候选证据。

## Data Flow

```text
accepted chapter + committed analysis
  -> load scoped latest retry draft (optional)
  -> merge latest quality feedback into repair targets
  -> generate one bounded candidate
  -> accept: atomic chapter commit
  -> retry with budget: atomic draft evidence + failed step commit
  -> retry exhausted/manual review: persist reviewable candidate + WaitingHuman
```

## Compatibility

- 不新增或修改数据库表结构。
- 不改变公开 HTTP API 的必填字段。
- 任务结果只新增可选诊断字段，现有消费者可忽略。
- 旧的、无 Autopilot 作用域信息的 draft attempt 不会被自动消费。
- 已通过章节、普通单章生成和批量生成路径不受影响。

## Security And Privacy

- 只持久化章节候选及规范化质量上下文，这些本就是现有 candidate draft 业务数据。
- 禁止把 prompt、角色模型策略原文、provider payload 或凭据写入 `repair_payload`。
- 最近候选读取必须同时匹配章节、run、epoch、源正文 digest 和分析 ID，防止跨运行污染。

## Operational And Rollback Considerations

- 若候选证据插入失败，整个质量失败事务回滚，避免 step 已终止但候选缺失。
- 若最近候选损坏或不完整，记录安全告警并回退到已接受正文，不阻塞恢复。
- 回滚可限定在返修生成服务、适配器、返修仓储和对应测试；无 schema rollback。

## Alternatives Rejected

- 仅调整错误码：不能让重试收敛，也不能保留候选证据。
- 直接把失败候选写入正式章节：破坏质量门禁与并发安全。
- 新增专用表：现有 `chapter_draft_attempts` 已满足需求，违反 KISS/YAGNI。
- 使用 `batch_task_id` 存 run ID：该字段有 `batch_generation_tasks` 外键，语义和约束均不兼容。
