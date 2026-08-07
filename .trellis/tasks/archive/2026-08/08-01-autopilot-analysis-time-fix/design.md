# 自动创作质量门、失败终态与时间契约设计

## 1. 设计目标

本设计同时修复六个相互关联但所有权不同的问题：

1. 单章候选质量门错误地把 `applicable=false` 的指标作为 0 分弱项。
2. 章节生成、分析或返修在质量重试耗尽后可能进入没有可操作候选的 `waiting_human`。
3. 章节分析/返修 Provider 错误只留下聚合码，无法区分超时、限流、上游不可用和响应无效。
4. Autopilot API 把 UTC 语义的 `NaiveDateTime` 输出为无时区字符串，前端因此少显示 8 小时。
5. ChapterGenerate 预算内质量重试只保留 digest/摘要，丢弃上一候选正文，下一次尝试无法消费质量反馈形成定向改写闭环。
6. Provider 瞬时故障没有持久化 `not_before`，同一 Run 在连续调度或进程重启后可能立即重试并形成请求突刺。

产品决定是：保留最终人工复核候选。质量重试耗尽时，只有完整候选已经通过
原子事务持久化后才能进入 `waiting_human`；用户主动配置的高风险确认等真实
人工门保持不变。Provider 瞬时故障仅在预算内自动重试；认证/配置、上下文、
响应结构错误不可重试，瞬时故障预算耗尽后也进入无候选 `waiting_human`，仅允许
Retry/Repair/Stop，不允许 Accept。预算内 ChapterGenerate 质量重试必须持久化并
消费上一完整候选；预算内 Provider 重试必须持久化 capped `next_attempt_at`，由
连续调度和启动恢复共同遵守。

## 2. 所有权与修改边界

| 责任 | 所有者 | 设计变更 |
| --- | --- | --- |
| 候选质量指标计算 | `single_generation_candidate_quality_owner.rs` | 保持现有 `details.*.applicable` 生产逻辑 |
| 质量摘要、修复指引和质量门 | `story_repair_quality_context_owner.rs` | 在共享指标收集入口过滤不适用指标 |
| 章节生成终态 | `novel_autopilot/chapter_adapter.rs` + Repository | 质量耗尽复用原子人工候选事务 |
| 章节生成质量反馈闭环 | `chapter_adapter.rs` + `chapter_repository.rs` + generation service | 预算内保存并消费作用域正确的完整 retry evidence |
| 章节分析终态 | `chapter_analysis_adapter.rs` + `chapter_analysis_repository.rs` | 有有效分析结果时人工复核，无结果时不伪造候选 |
| 章节返修终态和重试证据 | `chapter_repair_adapter.rs` + `chapter_repair_repository.rs` | 保留中间重试证据和最终人工候选 |
| 后台任务终态 | `novel_autopilot/coordinator.rs` + `api/background_tasks.rs` | 明确区分 waiting candidate 与 provider failure |
| Provider 失败诊断 | 分析/返修 generation service 与 Adapter | 产生 allowlist 稳定类别和结构化日志 |
| Provider 持久化退避 | Adapter/Repository + coordinator/API startup reconciliation | 持久化并执行跨重启 `not_before`，成功或离开自动重试状态时清理 |
| Run schema | SeaORM model + Rust migration metadata + PostgreSQL Alembic/frozen migrator model | 新增 nullable UTC `next_attempt_at` 并保持 fresh revision chain/upgrade/downgrade 一致；冻结 initial baseline 不回写 |
| 时间 API 契约 | `api/novel_autopilot_runs.rs` | UTC `NaiveDateTime` 输出 RFC 3339 `Z` |
| 页面显示 | `NovelAutopilotWorkbench.tsx` | 按 Run 状态显示正确失败文案，继续使用浏览器本地时区 |

不新增数据库表；允许且只新增 `novel_autopilot_runs.next_attempt_at` nullable 列。
该 schema 扩展不回填、不平移历史时间值，也不修改与本任务无关的人工门。

## 3. 质量门修复

### 3.1 当前断裂

```text
评分器
  details.outline_alignment.applicable = false
  outline_alignment_rate = 0
  overall_score 计算时跳过该指标
        |
        v
质量上下文归一化
  collect_quality_metric_signals() 只读取扁平 outline_alignment_rate
        |
        v
错误结果
  failed_metrics 包含“大纲贴合=0”
  weakest_metric = 大纲贴合
  weak_metric_count 增加并触发 auto_repair
```

### 3.2 修复契约

`QualityMetricDescriptor` 增加与 `details` 子对象对应的 `detail_key`。例如：

```text
outline_alignment_rate -> details.outline_alignment
rule_grounding_hit_rate -> details.rule_grounding
conflict_chain_hit_rate -> details.conflict_chain
```

`collect_quality_metric_signals()` 在创建 `QualityMetricSignal` 前执行一次判断：

- `details.<detail_key>.applicable == false`：跳过该指标。
- `details`、子对象或 `applicable` 缺失：按 `true` 处理，兼容历史指标。
- `applicable` 类型错误：不静默解释为 `false`，按历史兼容路径参与计算。
- 不删除原始 `details` 和扁平指标；只改变派生的 repair guidance / quality gate。

因为修复位于共享收集入口，以下派生结果会同时一致：

- `failed_metrics`
- `weakest_metric_*`
- `weak_metric_count`
- `focus_areas`
- `repair_targets`
- `auto_repair` / `allow_save` 决策

整体分数继续由现有 `applicable_quality_overall()` 负责，不重复实现过滤逻辑。

### 3.3 边界用例

- 仅一个指标不适用，其余全部达标：不适用指标不触发返修。
- 多个指标不适用：最弱项只能来自适用指标。
- 所有描述指标不适用：修复指引返回“指标不足”边界，不 panic。
- 历史 payload 没有 `details`：行为与当前版本一致。
- 第二次候选存在其他真实弱项：过滤不适用项后仍按真实总分/弱项判定。

## 4. 可操作人工复核与无候选故障

### 4.1 状态语义

```text
尝试未耗尽
Step failed(error_code) -> Run running -> 调度下一次尝试

质量尝试耗尽且有完整候选
Candidate waiting_human(content + digest + safe quality facts)
          +
Step terminal(manual_review + error_code + candidate id)
          +
Run waiting_human(last_error_code, active task cleared)
          +
Background Task completed(dispatch_status = waiting_human)

Provider/配置/上下文/响应故障且无候选
Step failed(stable no-candidate error code)
          +
Run waiting_human(last_error_code, active task cleared)
          +
Background Task exposes the same safe provider diagnostic
```

无候选人工处理保留 Retry/Repair/Stop，用于用户调整配置或指导后继续；不得提供
Accept，也不得显示“候选已保存”。Provider 的 timeout、429、5xx 和 unknown
瞬时故障仅在预算内自动重试；认证/配置、上下文和响应结构错误不可重试，立即进入
无候选 `waiting_human`。

以下场景进入可操作人工复核：

- `chapter_generate` 已生成完整候选，但质量重试预算耗尽。
- `chapter_repair` 已生成完整返修候选，但质量重试预算耗尽。
- `chapter_analyze` 已生成有效分析结果，但质量决策需要人工确认。

以下场景不得伪造人工候选：

- Provider 请求失败、超时、限流或上游不可用，未产生完整结果。
- Provider 返回无法解析或结构无效的结果，无法构造安全候选。
- 执行配置或业务事实无效，模型调用前已经失败。

以下场景不受影响：

- 预算内自动重试。
- 用户主动配置的 `high_risk_only`、`every_n_chapters` 等真实确认点。
- 其他确实持久化了可操作候选的既有人工决策流程。
- 成本预算等本任务未要求改变的 fail-closed 策略。

### 4.2 原子持久化

不得沿用 `complete_step()` 后再单独 `transition_owned()` 的两次提交方式。
质量耗尽应复用现有 `persist_chapter_manual_review_candidate()` 所有权，在一个
事务内完成：

1. 重新校验 user、Run version/epoch/status、Step identity/attempt/status 和 task fence。
2. 校验候选正文非空、word count、完整状态、digest 和质量信息。
3. 候选写入 `chapter_draft_attempts`，ID 与 terminal Step ID 一致，state 为
   `waiting_human`，source 为 `novel_book_autopilot`。
4. Step 写稳定 `error_code`、`manual_review`、digest 和 `completed_at`。
5. Run 写 `waiting_human`、同一 `last_error_code`、清空 `current_step` 和
   `active_background_task_id`、写 `updated_at`、递增 version。
6. 任一 CAS、候选插入或 Step 更新失败时回滚全部更新。

Provider/解析失败继续使用不含候选的失败事务，并与候选事务分开，防止一个
`waiting_human: bool` 同时表达“有候选待复核”和“无内容但等待配置调整”。

### 4.3 最终候选与中间重试证据

- `chapter_generate` 预算内质量失败：把完整正文和受限质量信息保存为内部 retry evidence，下一次 ChapterGenerate 只消费作用域匹配的最新证据。
- `chapter_generate` 最终未通过候选：调用既有人工候选持久化契约，保存完整正文、digest、word count 和受限质量信息。
- `chapter_repair` 预算内失败：继续保存受 Run/epoch/source digest/analysis ID 约束的完整重试证据，供下一次自动返修消费。
- `chapter_repair` 最终耗尽：调用 `persist_manual_review_candidate()`，把最后一次完整返修结果转换为人工候选。
- Provider 失败没有完整候选：不得伪造候选或 digest。
- Accept 使用候选 ID、Run version/epoch、Step identity 和章节快照 CAS；成功后提交正文并继续运行。
- Retry/Repair/Stop 继续使用现有人工决定路由和版本围栏。

### 4.4 ChapterGenerate 质量反馈闭环

ChapterGenerate retry evidence 继续复用 `chapter_draft_attempts` 的完整候选所有权，
但必须用独立 source/state 与 `waiting_human` 人工候选区分，不能让内部重试证据获得
Accept 能力。候选正文通过现有完整正文抽取契约保存；Run/Step/Task/SSE/日志只允许
输出 candidate ID/digest、attempt、质量决定、失败指标键和有界修复方向。

每条 evidence 至少持久化并校验以下作用域：

```text
run_id
run_epoch
chapter_id + chapter_number
source_content_digest       # 生成开始前的章节业务快照；无正文时使用明确 empty digest
candidate_content_digest    # 必须等于完整候选正文的 digest
step_attempt
```

预算内质量失败在一个 Repository 事务中完成：

1. 以 user、Run version/epoch/status、Step identity/type/attempt、task fence 和章节快照 CAS 重新校验所有权。
2. 校验候选非空、完整正文 digest、word count、质量决定和有界质量摘要。
3. 插入以 terminal Step ID 为 ID 的 retry evidence，正文只进入 `chapter_draft_attempts`。
4. 终结当前 Step，写相同 candidate digest、质量决定和稳定 retry reason。
5. 更新 Run 的质量失败计数/version，清理当前 Step/task fence，保留自动重试状态。
6. 任一候选插入、Step 更新、Run CAS 或章节快照校验失败时回滚全部写入。

下一次 ChapterGenerate 在调用 Provider 前，按同一 `run_id/run_epoch/chapter/source digest`
查询 attempt 小于当前 attempt 的最新 evidence，再验证 candidate digest 与完整正文一致。
匹配时把上一候选正文和 allowlist 质量反馈/修复方向送入受控生成输入，形成
“候选 -> 质量反馈 -> 定向改写”；缺失证据允许按首轮生成，存在但跨作用域、损坏或
digest 不一致的证据必须拒绝或忽略并记录稳定安全码，不能静默串用其他 Run/章节内容。

`POST /api/projects/{project_id}/novel-autopilot-runs/{run_id}/decision` 收到
`decision=accept` 时必须先读取最新 Step 和 waiting candidate。存在候选时进入既有
原子 Accept 事务；不存在候选时，仅 Run 与最新 Step 均无错误的周期性人工门可继续。
若没有最新 Step，或 Run/Step 任一带错误码，API 同步返回
`409 human_decision_candidate_unavailable`。该预检必须发生在写 guidance、递增 Run
version 或创建后台任务之前；协调器还应依据最新 Step 的错误码做最终 fail-closed
检查，防止绕过 API 的构造请求获得接受能力。

这与 `.trellis/spec/backend/durable-novel-autopilot.md` 中“耗尽后保存人工候选并进入 `waiting_human`”的既有契约一致；实现重点是让当前绕过候选持久化的
`finish_quality_retry()` 耗尽分支回到该契约。

## 5. 后台任务与业务状态投影

质量候选持久化成功后继续使用 `NovelAutopilotTickOutcome::AwaitingHuman`，其结果至少包含：

```text
task_result:
  run_id
  run_status = waiting_human
  step_id
  step_type
  attempt
  reason_code
  candidate_id
  quality_diagnostics # allowlist only
```

`background_tasks.rs` 完成当前 tick 时：

- Task 可以保持 `completed`，因为该 tick 已成功持久化人工候选，但 message 必须明确为“候选已保存，等待人工复核”。
- `result.dispatch_status` 为 `waiting_human`，包含 candidate ID 和稳定原因码。
- Run `last_error_code`、Step `error_code` 和 task result 的 reason code 一致。
- SSE 终态 `data` 只携带候选引用和脱敏诊断，不携带正文。
- Provider/解析无候选故障走独立投影，不能复用“候选已保存”文案。

这样可以区分“成功完成一个候选并等待人工”和“模型调用失败且没有候选”，
同时解释后台 Task `completed` 与业务 Run `waiting_human` 的不同所有权。

## 6. Provider 失败诊断契约

### 6.1 稳定分类

在分析/返修 generation service 边界定义共享的安全诊断值对象，不把原始
`String` 直接传到日志、数据库或 API。建议字段：

```text
schema_version = novel-autopilot-failure-diagnostic/v1
source_code     = invalid_input | context_error | analysis_not_found |
                  generation_error | invalid_result
category        = timeout | rate_limited | upstream_unavailable |
                  authentication_or_configuration | response_invalid |
                  context_invalid | unknown
provider?       = allowlisted provider identifier
model?          = configured model identifier, bounded length
http_status?    = integer 400..599
retryable       = boolean
```

分类规则：

- 优先使用已有 typed error variant 和明确 HTTP 状态。
- 只有底层仍为字符串时，原文仅在内存中经过固定关键词/状态码分类器，不持久化原文。
- 无法安全分类时使用 `unknown`，不得退回记录完整异常。
- Provider/model 优先来自已解析执行配置，不从 Prompt 或响应正文猜测。
- Provider/model 字符串做长度限制和字符白名单；API Key、URL query、Authorization、正文和 Prompt 永不进入对象。

### 6.2 稳定错误码

Run `last_error_code`、Step `error_code` 和 Task `error` 使用同一个码：

- 可安全分类时使用更具体的稳定码，例如
  `chapter_analysis_provider_timeout`、
  `chapter_analysis_provider_upstream_unavailable`、
  `chapter_repair_provider_rate_limited`、
  `chapter_repair_result_invalid`。
- 无法分类时保留现有兼容码
  `chapter_analysis_provider_failed` / `chapter_repair_provider_failed`。
- 质量耗尽使用明确的 attempts-exhausted 码，不误标为 Provider 失败。

日志事件保留 Run、Step、attempt、chapter、background task、provider、model、
category 和可选 HTTP 状态，不记录原始异常字符串。

### 6.3 Provider 持久化退避与 `not_before`

#### Schema contract

`novel_autopilot_runs` 新增：

```text
next_attempt_at TIMESTAMP WITHOUT TIME ZONE NULL  # UTC semantics
```

- `NULL` 表示没有持久化时间门，可以立即参与正常调度。
- upgrade 只添加 nullable 列，不设置 server default、不回填既有行。
- downgrade 删除该列；回滚时先回滚读取该字段的新二进制，再执行 downgrade。
- fresh 完整 revision chain、SeaORM model、Rust migration metadata、PostgreSQL Alembic
  revision 和冻结 Python migrator model 必须得到相同类型与 nullability；历史 initial
  baseline 保持 frozen，不声明本列。
- 不修改既有 `created_at`/`updated_at` 等历史时间值，也不把 `next_attempt_at`
  暴露为客户端可写字段。

#### Delay calculation

只对诊断为 retryable 的 Provider timeout、rate limit、upstream unavailable 和兼容
unknown 类别计算退避；认证/配置、上下文或响应结构错误继续直接进入无候选人工处理。

```text
hint_delay = parse Retry-After delta-seconds or HTTP-date relative to now
local_floor = capped_exponential(step_attempt/provider_failure_count)
provider_floor = valid hint_delay ? clamp(hint_delay, min_delay, cap) : 0
base_delay = max(local_floor, provider_floor)
jitter_seed = digest(run_id, run_epoch, step_key, step_attempt, reason_code)
jitter = deterministic non-negative bounded value derived from jitter_seed
final_delay = min(cap, base_delay + jitter)
next_attempt_at = now_utc + final_delay
```

有效 `Retry-After` 作为本地指数退避的 floor；较短的 Provider 提示不会缩短本地
退避，较长的提示会把基础等待提升到经 cap 归一化后的值。无效、负数或无法解析的
提示不持久化原文，只回退本地策略。所有路径共享一个明确 cap，超过 cap 的 hint 或
jitter 都被截断。jitter 不使用随机进程状态、API Key、Prompt 或响应正文，因此同一
失败事实在进程重启前后计算一致。

当前实现已在 Provider HTTP transport boundary 从 response header 解析并规范化 typed
`retry_after_seconds`，并经 generation error、失败诊断和章节分析/返修 Adapter 透传到
Repository 退避计算。解析支持 delta-seconds 和 HTTP-date，禁止从错误字符串、URL 或
响应正文猜测 `Retry-After`。该端到端链路已有自动化回归；部署后的
OpenAI-compatible HTTP transport、完整工作流和跨重启运行行为也已通过 deterministic
Provider smoke 验证。部署 smoke 未故意注入第三方 Provider 的 `Retry-After` 故障响应，
因此 header 失败分支仍以聚焦自动化测试作为权威证据，不能扩张为外部网关实测结论。

#### Durable scheduling and clearing

- Provider 失败事务在终结 Step、增加失败计数、更新 Run version/状态并清理当前 task
  fence的同时写入 `next_attempt_at`；部分写入必须回滚。
- 同一后台任务的连续 tick、API dispatch retry、孤儿 queued Run 修复和 startup
  reconciliation 都把 `next_attempt_at` 作为 `not_before`。`now < next_attempt_at`
  时允许通过 Run version/epoch/active-task CAS 创建并绑定至多一个持久化 pending Task；
  Task payload 保存同一 UTC `not_before`，到期前不得标记 running、claim Step 或调用
  Provider。进程重启后按数据库中的原 due 重建 pending Task，不重新计算 delay；到期
  后 claim 层继续使用数据库围栏，多实例竞争只能有一个执行者获胜。
- Provider-backed Step 成功时清空；进入任何 `waiting_human`/manual review 时清空；
  pause、cancel/stop 时清空。resume 只恢复正常可调度状态，不复用已经清除的旧退避。
- 迟到旧 task 只能在匹配 Run version/epoch/active task fence 时更新或清理该字段，不能
  覆盖较新失败写入的时间。
- `next_attempt_at` 是内部持久化调度字段，本任务不要求新增 public API 字段。pending
  Task payload 可以携带同一 `not_before`，但调度和 claim 判断必须以数据库值与
  Run/Task fence 为准，不能依赖浏览器定时器或仅在 Task payload/result 中保存 delay。

## 7. 脱敏质量诊断

人工候选及无候选故障诊断允许保留以下字段：

- `overall_score`
- `quality_decision`
- `quality_gate_action`
- `failed_metrics[].key/label/value/threshold/gap`
- 受长度和条数限制的 `repair_targets` / `focus_areas`
- `result_digest`

候选正文只能保存在 `chapter_draft_attempts` 并通过受权限和 Run/Step 作用域约束
的候选 API 使用；Run/Step、后台任务结果、日志和 SSE 不得携带正文或正文预览。
所有位置都不得保存完整 Prompt、reasoning、Provider 响应、原始异常或 API Key。
诊断构造函数应从既有质量对象显式挑选字段，不能复制整个 JSON 后删除少数字段。

## 8. UTC API 时间契约

数据库现状保持：UTC 语义的 PostgreSQL `timestamp without time zone` 和 Rust
`NaiveDateTime`。

API 边界增加两个局部帮助函数：

```text
utc_rfc3339(NaiveDateTime) -> "2026-08-01T05:34:58.865557Z"
optional_utc_rfc3339(Option<NaiveDateTime>) -> string | null
```

`run_view()` 和 `step_view()` 的以下字段统一使用帮助函数：

- Run：`created_at`、`updated_at`、`started_at`、`paused_at`、`completed_at`
- Step：`created_at`、`updated_at`、`started_at`、`completed_at`

前端继续使用 `new Date(value)` 和 `toLocaleString('zh-CN')`，浏览器会把 UTC
转换为本地时区。禁止在前端追加 `Z`、手工加 8 小时或对旧字符串做猜测性修补，
否则会产生重复偏移。

`formatRuntime()` 同样消费带 `Z` 的开始/完成时间，持续时间计算与显示时区无关。

## 9. 前端行为

- 有候选的 `waiting_human` 明确显示“候选已保存，等待人工复核”和稳定原因码。
- Provider 无候选故障明确显示 Provider 类别，不能声称已有候选可接受。
- 新增的稳定错误码进入集中映射；未知码仍显示原始错误码。
- 保留现有 Accept/Retry/Repair/Stop 人工操作，并且只在可用候选/合法状态下启用。
- 时间线保持现有列布局，只修复数据契约和断言。

## 10. 兼容性、迁移和回滚

- 新增一条向后兼容的 schema migration，为 `novel_autopilot_runs` 增加 nullable
  `next_attempt_at`；既有 Run 升级后为 `NULL`，仍可立即参与旧行为。
- upgrade/downgrade 与 fresh 完整 revision catalog 同步；历史 ee0a initial baseline 保持
  frozen，不回写新列。fresh 数据库先由后续 revision 创建 Autopilot 表，再由本 revision
  执行 `ADD COLUMN/INDEX`。部署顺序为先 migration 后新二进制，回滚顺序为先旧二进制
  后 downgrade。
- 已经持久化为 `waiting_human` 的历史 Run 不自动改写，避免无审计的数据变更。
- 旧无时区 API 值只存在于旧响应，数据库值不变；部署后新响应带 `Z`。
- API 字段名和前端 TypeScript 类型不变，时间字符串从 ISO local-like 收紧为 RFC 3339 UTC。
- 新 Provider 诊断码是向后兼容扩展，未知码已有回退显示。
- 人工候选沿用现有表和 API，不需要数据库迁移。
- ChapterGenerate retry evidence 复用现有 `chapter_draft_attempts`，不新增正文列；如反馈
  消费出现回归，可停止读取新 evidence，但不得删除已持久化候选或放宽作用域校验。
- 如持久化退避出现回归，可先回滚应用读取/写入，再 downgrade 删除 nullable 列；
  downgrade 会丢失尚未到期的 `not_before`，因此回滚窗口内必须限制立即重试突刺。

## 11. 测试矩阵

### Rust 单元/Repository/API

- `applicable=false` 指标不进入失败项、最弱项、弱项计数和修复方向。
- 缺少 `details` 的历史指标保持当前行为。
- 质量和 Provider 预算内失败仍调度重试。
- 最终质量耗尽原子写入人工候选、Run `waiting_human` 和可关联 Step。
- 最终章节生成/返修候选可通过 Accept/Retry/Repair/Stop 操作；预算内重试证据仍可被下一次消费。
- ChapterGenerate 预算内质量失败原子保存完整 retry evidence；下一 attempt 只消费作用域与 digest 全部匹配的最新记录，并确实把上一候选和安全反馈送入生成输入。
- ChapterGenerate evidence 写入失败、章节快照变化、Run/epoch/task fence 变化时，candidate/Step/Run 全部回滚；正文不会出现在 Run/Step/Task/SSE/日志。
- Provider、配置、上下文或响应无效的无候选故障不创建 `chapter_draft_attempts`；不可重试或预算耗尽时将 Run 投影为 `waiting_human`，且仅允许 Retry/Repair/Stop。
- 构造无候选 `accept` 请求时 API 返回 `409 human_decision_candidate_unavailable`，Run
  保持 `waiting_human` 且 version、guidance 和后台任务均不变化；协调器直接收到同类
  decision 时也必须 fail-closed。
- Repository CAS、candidate insert 或 Step update 失败时事务完整回滚。
- Provider 分类覆盖 timeout、429、503、invalid result 和 unknown，并验证原文/密钥不出现在诊断。
- typed Provider boundary 对 `Retry-After` delta-seconds/HTTP-date 的有效、无效、负数和超 cap 用例；验证规范化 hint 的端到端透传、有效提示优先，以及本地 fallback/cap 和 deterministic jitter 使用固定时钟与稳定 seed 可复现。纯函数仅以 `Some(seconds)` 测试不等同于 header 边界已接入。
- `next_attempt_at` 未到期时连续调度、API 重派和 startup reconciliation 最多绑定一个携带原 due 的 pending Task，该 Task 不进入 running、不 claim Step、不调用 Provider；重启按同一 due 重建，到期后多实例 claim 只有一个 DB CAS 获胜。
- success、waiting_human/manual、pause、cancel/stop 清空退避；旧 epoch/task 的迟到完成不能清除新值。
- migration upgrade 后既有行为 `NULL`、downgrade 可逆、fresh 完整 revision chain 与 Rust/Python model metadata 完全一致，并断言 frozen initial baseline 未包含 `next_attempt_at`，避免重复 DDL。
- `run_view()` / `step_view()` 所有非空时间以 `Z` 结尾，空值仍为 `null`。

### Frontend Playwright

- 在 `Asia/Shanghai` 时区下，API `2026-08-01T05:34:58Z` 显示为
  `2026/8/1 13:34:58`。
- 创建、更新和 Step 时间遵循同一偏移；不会重复加 8 小时。
- 固定开始/结束时间的运行时长正确。
- 有候选的 `waiting_human` Run 显示候选已保存，人工决策控件可用。
- Provider 无候选故障不显示“候选已保存”或可接受候选的误导文案。

### 安全检查

- 测试构造包含 API Key、完整 Prompt、章节正文和原始 Provider 响应的错误，断言
  Task result、API、SSE 和日志诊断对象均不含这些内容。
- UTF-8 无 BOM、格式、Clippy、前端 lint/build 和聚焦 Playwright 通过。
