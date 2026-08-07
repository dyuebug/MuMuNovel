# 修复自动创作章节分析失败与时间显示错误

## Goal

让整本小说自动创作正确区分质量门失败与 Provider 失败；质量重试耗尽且已有完整候选时，持久化可接受、返修或重试的人工复核候选，避免进入没有内容可操作的人工状态；预算内的 ChapterGenerate 质量重试必须保存并消费上一候选及其安全质量反馈，Provider 瞬时故障必须使用可跨重启恢复的持久化退避；同时保留可诊断、已脱敏的失败原因，并统一运行详情与步骤时间线的时间语义，确保“更新时间”等字段在用户界面中显示为正确时间。

## Confirmed Facts

- 用户报告自动创作仍会出现章节分析失败，并进入人工处理。
- 最新运行 `21c2d40f-cec1-4f60-af77-cec3f2ae9a64` 实际失败在 `chapter_generate`，并没有创建 `chapter_analyze` Step；界面现象需要按真实 Step 类型解释，不能只按用户可见标签判断。
- 最新运行的第 1 章生成连续三次产生不同 `result_digest`：前两次为 `chapter_quality_retry`，第三次为 `chapter_generation_attempts_exhausted`；`consecutive_provider_failures=0`，可排除本次为 Provider 调用失败。
- 第一次候选的质量总分为 `86.1`，但质量门把 `outline_alignment.applicable=false`、`skipped_reason=no_outline_anchors` 的“大纲贴合=0”继续计入失败项，形成确定性的假失败信号。
- 第二次候选总分为 `66.1`，存在其他真实弱项，但“不适用的大纲贴合”仍被重复计入失败项。
- 第三次耗尽后，后台任务结果仅保留 `chapter_generation_attempts_exhausted`，丢失最终候选的质量指标；Run 已为 `waiting_human`，但 `last_error_code` 为空。
- `chapter_generation_attempts_exhausted` 路径没有把最终候选写入 `chapter_draft_attempts`，因此“等待人工处理”没有可供复核的持久化候选。
- 产品决策已调整：保留人工复核。最后一次完整候选未通过质量门时，必须保存该候选，并提供 Accept/Retry/Repair/Stop 操作。
- 质量耗尽进入 `waiting_human` 的前提是已经原子持久化可操作候选；不得再出现 Run 等待人工但 `chapter_draft_attempts` 没有对应候选的状态。
- 中间自动返修尝试继续保存受 Run/epoch/章节摘要约束的重试草稿，最终候选通过既有人工候选契约持久化。
- ChapterGenerate 的预算内质量重试当前只在 Step/Task 中保留候选 digest 和安全质量摘要，候选正文没有持久化，下一次生成无法以同一候选为基线形成“候选 -> 质量反馈 -> 定向改写”的闭环。
- 用户主动配置的高风险确认等真实人工门保持不变。
- 已核验的相邻故障中，同一章节返修步骤连续三次返回聚合错误 `chapter_repair_provider_failed`，随后因 `max_step_attempts=3` 进入 `waiting_human`。
- 当前章节分析/返修 Adapter 只记录稳定错误码，未持久化底层 Provider、HTTP 状态、模型、响应解析或候选执行错误摘要。
- 可重试 Provider 故障当前只返回 `retry_scheduled`，没有持久化 `not_before` 时间；进程重启、任务恢复或启动协调可能立即重试并形成请求突刺。
- Provider transport boundary 已从 HTTP response header 解析并规范化可选
  `retry_after_seconds`，章节分析/返修链路保留 typed hint；delta-seconds 和
  HTTP-date 均受统一 cap，缺失、非法或过期值回退到本地 capped backoff。
- `novel_autopilot_runs` 当前没有 `next_attempt_at`；持久化退避需要新增一个 UTC 语义、nullable、无默认回填的列，并同步所有受支持的 schema owner。
- 后台编排任务可以显示 `completed`，而业务 Run 已是 `waiting_human`，任务记录的 `error` 仍为空。
- PostgreSQL 的 Autopilot 时间字段使用 UTC 语义的 `timestamp without time zone`；Rust API 直接序列化 `NaiveDateTime`，产生不带 `Z`/offset 的字符串；前端再用 `new Date(value).toLocaleString('zh-CN')`，导致北京时间固定少 8 小时。
- 当前工作区包含大量与本任务无关的未提交内容，实施时必须保留并绕开这些内容。

## Requirements

- 定位最新章节分析失败的完整调用链和真实失败类别，不能仅依据聚合错误码猜测。
- 质量门不得把 `applicable=false` 的指标作为失败项、最弱项、弱项计数或自动返修触发条件。
- 自动生成质量重试耗尽且候选完整时，必须将候选正文、digest 和受限质量信息写入 `chapter_draft_attempts`，但不得写入 Run/Step 或后台任务快照。
- ChapterGenerate 预算内质量重试同样必须把完整候选正文作为内部 retry evidence 写入 `chapter_draft_attempts`；Run、Step、Task result、日志和 SSE 只能携带 candidate ID/digest、attempt、质量决定、失败指标键和精简修复方向等 allowlist 摘要。
- ChapterGenerate retry evidence 的作用域至少包含 `run_id`、`run_epoch`、chapter identity、`source_content_digest`、`candidate_content_digest` 和 `step_attempt`；下一次尝试只可消费最新、完整、digest 自洽且全部作用域匹配的证据，过期、跨 Run/epoch/章节或损坏证据必须拒绝或忽略。
- 写入 ChapterGenerate retry evidence、终结当前 Step、更新 Run 质量失败计数/版本并释放当前 task fence 必须属于同一事务；任一写入或 CAS 失败时不得留下孤立候选或部分状态。
- Run、Step 和后台任务进入人工复核时必须表达同一稳定原因，并能关联同一个候选 ID，避免 Run `last_error_code` 为空或人工决定找不到候选。
- 对章节分析和章节返修失败使用一致的、可脱敏的诊断契约，至少能够关联 Run、Step、attempt、background task、Provider、模型和失败类别。
- 不在日志、数据库或 API 中暴露 API Key、完整 Prompt、章节正文或未经限制的上游响应体。
- 后台任务状态、业务 Run 状态和步骤时间线不得产生误导性冲突；质量耗尽时应明确表达“候选已保存，等待人工复核”，Provider 无候选故障不得使用相同文案。
- 明确定义运行时间字段的 API 时间语义，并让前端按该契约显示创建时间、更新时间、开始时间和完成时间。
- Autopilot API 必须把数据库中的 UTC `NaiveDateTime` 输出为带 `Z` 或 `+00:00` 的 RFC 3339 字符串；前端继续按浏览器本地时区显示。
- 保持现有 API 字段和已有自动创作运行数据向后兼容；新增诊断字段应为可选字段或通过兼容迁移提供默认值。
- 为 Provider 失败、解析失败、重试耗尽和时区转换补充自动化测试。
- 对可重试 Provider 故障计算有上限的退避时间：有效 `Retry-After` 优先于本地指数退避，所有来源均受统一 cap 限制；缺失或无效提示回退到本地策略，并使用基于稳定作用域的 deterministic jitter，重启后同一失败事实得到相同延迟。
- 在 `novel_autopilot_runs.next_attempt_at` 持久化最终 UTC `not_before`；后台连续调度、API 重派和启动恢复在 `now < next_attempt_at` 时允许通过 CAS 创建并绑定至多一个携带同一 `not_before` 的持久化 pending Task，但该 Task 到期前不得标记 running、claim Step 或调用 Provider；重启后按数据库中的同一 due 重建 pending Task，不得重新计算并提前退避时间。
- `next_attempt_at` 在 Provider 成功、进入人工处理、暂停、取消/停止时原子清空；恢复运行本身不得复用已经清除的旧退避，迟到任务也不得清除较新 epoch/version 的时间。
- 为 `next_attempt_at` 提供独立 nullable schema upgrade/downgrade；冻结的 initial baseline 保持不变，fresh 数据库必须通过完整 revision catalog 先创建 Autopilot 表，再由新 revision 添加列和索引。既有行保持 `NULL`，不重写历史 Run/Step 时间值。
- 仅保存 allowlist 诊断字段：聚合错误码、稳定子类别、Provider/模型标识、可选 HTTP 状态、attempt、评分摘要、失败指标键和精简修复方向；不得保存底层错误原文。

## Acceptance Criteria

- [x] 可通过最新失败 Run 的时间线和服务记录确定失败类别；若上游只返回通用错误，也要明确记录该证据边界。
- [x] 章节分析失败不再只暴露 `chapter_analysis_provider_failed`，同时提供安全的稳定子错误码或诊断摘要。
- [x] 章节返修失败保留同等粒度的诊断信息，避免两个 Adapter 再次出现观测差异。
- [x] `applicable=false` 的质量指标不会出现在 `failed_metrics`，不会成为 `weakest_metric`，也不会单独触发 `auto_repair`。
- [x] 第三次质量重试耗尽后，在同一事务中创建一条完整、作用域正确的人工候选，并让 Run 进入 `waiting_human`；任一写入失败时全部回滚。
- [x] 人工候选可通过现有 Accept/Retry/Repair/Stop API 操作；Accept 使用章节快照 CAS，过期或缺失候选必须拒绝而不能部分提交。
- [x] Run `last_error_code`、Step `error_code`、后台任务结果和候选 ID 能够互相关联，页面明确显示“候选已保存，等待人工复核”。
- [x] Provider 连续失败达到配置上限，或配置/上下文/响应无效且不可重试时，不伪造候选，并保留脱敏的稳定失败子类别；Run 进入 `waiting_human`，仅允许 Retry/Repair/Stop，不允许 Accept。
- [x] 中间自动返修重试仍能消费上一次受作用域约束的重试证据，最终人工候选也来自最后一次完整结果。
- [x] 用户主动配置的高风险确认仍按既有逻辑工作，不受失败终态变更影响。
- [x] 创建时间、更新时间、开始时间和完成时间在同一页面遵循同一时区规则，且不会出现固定 8 小时偏移或重复偏移。
- [x] 既有 UTC 语义的无时区数据库记录通过 API 统一输出为 RFC 3339 UTC；本任务不要求迁移或重写历史时间值。
- [x] Rust 相关单元/集成测试、前端类型检查及聚焦时间线测试通过。
- [x] 日志和 API 响应经过敏感信息检查，不包含 API Key、完整 Prompt 或章节正文。
- [x] ChapterGenerate 第一次质量失败后原子保存带完整正文的 retry evidence；第二次尝试只加载同一 `run_id/run_epoch/chapter/source_content_digest/candidate_content_digest/step_attempt` 链上的最新证据，并把安全质量反馈用于定向重写而不是无上下文重新生成。
- [x] ChapterGenerate retry evidence 的正文只存在于 `chapter_draft_attempts`；Run/Step/Task/SSE/日志的回归扫描仅看到 allowlist 摘要，且事务失败不会留下候选、计数或 Step 的部分提交。
- [x] timeout、429、5xx 等可重试 Provider 故障在章节分析/返修入口持久化 capped `next_attempt_at`；当前使用带 deterministic jitter 的本地退避，数据库 due 是跨重启恢复的唯一时间门。
- [x] typed Provider transport boundary 解析并透传有效 `Retry-After`（delta-seconds 或 HTTP-date），使其优先于本地退避且受统一 cap 限制；解析只读取 HTTP response header，不从错误字符串、URL 或响应正文猜测。
- [x] `now < next_attempt_at` 时常规调度和启动恢复绑定带同一 due 的 pending Task，到期前不标记 running；到期后 claim 层数据库围栏拒绝提前执行并清空已到期 due。
- [x] Provider 成功、`waiting_human`、pause、cancel/stop 都会清空 `next_attempt_at`，version/epoch/task fence 阻止迟到旧任务覆盖新状态。
- [x] PostgreSQL upgrade/downgrade、Rust migration metadata、SeaORM model、冻结 migrator model 和 fresh 完整 revision chain 对 nullable `next_attempt_at` 一致；冻结 initial baseline 未回写该列，既有行升级后为 `NULL`，时间字段无数据平移。
- [x] 补齐两个并发调度器只绑定一个 pending Task、startup reconciliation 按原 due 重建、due 前不进入 running/claim/Provider 执行链以及 success/manual/pause/cancel/stop 清理路径的跨层回归。

## Out of Scope

- 不在缺少证据时更换用户的 Provider、模型、API Key 或网关地址。
- 不以提高重试次数掩盖确定性失败。
- 不重构与 Novel Autopilot、生成执行审计和时间线无关的模块。
- 不删除人工门，也不改变用户主动选择的 `high_risk_only`、`every_n_chapters` 等确认策略。
- 不迁移、重写或人为平移历史数据库时间值。
- 不把本任务扩展为所有后台任务或所有 Provider 客户端的全局重试框架；持久化退避只覆盖 Durable Novel Autopilot 的 Provider 重试入口和重派路径。

## Resolved Decisions

- 保留人工复核机制。Provider 瞬时故障只在预算内自动重试；认证/配置、上下文、响应结构错误立即进入无候选 `waiting_human`，Provider 瞬时故障耗尽预算后也进入无候选 `waiting_human`。
- 无候选人工处理仅提供 Retry/Repair/Stop，不得显示 Accept，也不得使用“候选已保存”文案；有完整质量候选的 `waiting_human` 继续提供 Accept/Retry/Repair/Stop。
- 允许为 `novel_autopilot_runs` 新增 nullable `next_attempt_at` 列；这是一项向后兼容的 schema 扩展，不改变“不平移历史 UTC 时间值”的既有决定。
- ChapterGenerate 与 ChapterRepair 都必须在预算内保留可作用域校验的完整 retry evidence；前者用于生成反馈闭环，后者继续用于返修基线。两者都不能把正文复制到编排状态、任务结果或 SSE。
- Provider `Retry-After` 已在 typed Provider boundary 解析、规范化并透传；有效 hint
  是本地退避的 floor，缺失或无效值使用 capped fallback + deterministic jitter。
  最终 `not_before` 持久化后成为调度与恢复的唯一时间门。

## Notes

- 本任务跨越 PostgreSQL、Rust 服务、后台任务快照和 React 时间线，按复杂任务处理，规划阶段必须补充 `design.md` 与 `implement.md`。
