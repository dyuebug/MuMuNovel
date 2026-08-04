# 自动创作质量门、失败终态与时间契约设计

## 1. 设计目标

本设计同时修复四个相互关联但所有权不同的问题：

1. 单章候选质量门错误地把 `applicable=false` 的指标作为 0 分弱项。
2. 章节生成、分析或返修在质量重试耗尽后可能进入没有可操作候选的 `waiting_human`。
3. 章节分析/返修 Provider 错误只留下聚合码，无法区分超时、限流、上游不可用和响应无效。
4. Autopilot API 把 UTC 语义的 `NaiveDateTime` 输出为无时区字符串，前端因此少显示 8 小时。

产品决定是：保留最终人工复核候选。质量重试耗尽时，只有完整候选已经通过
原子事务持久化后才能进入 `waiting_human`；用户主动配置的高风险确认等真实
人工门保持不变。Provider 瞬时故障仅在预算内自动重试；认证/配置、上下文、
响应结构错误不可重试，瞬时故障预算耗尽后也进入无候选 `waiting_human`，仅允许
Retry/Repair/Stop，不允许 Accept。

## 2. 所有权与修改边界

| 责任 | 所有者 | 设计变更 |
| --- | --- | --- |
| 候选质量指标计算 | `single_generation_candidate_quality_owner.rs` | 保持现有 `details.*.applicable` 生产逻辑 |
| 质量摘要、修复指引和质量门 | `story_repair_quality_context_owner.rs` | 在共享指标收集入口过滤不适用指标 |
| 章节生成终态 | `novel_autopilot/chapter_adapter.rs` + Repository | 质量耗尽复用原子人工候选事务 |
| 章节分析终态 | `chapter_analysis_adapter.rs` + `chapter_analysis_repository.rs` | 有有效分析结果时人工复核，无结果时不伪造候选 |
| 章节返修终态和重试证据 | `chapter_repair_adapter.rs` + `chapter_repair_repository.rs` | 保留中间重试证据和最终人工候选 |
| 后台任务终态 | `novel_autopilot/coordinator.rs` + `api/background_tasks.rs` | 明确区分 waiting candidate 与 provider failure |
| Provider 失败诊断 | 分析/返修 generation service 与 Adapter | 产生 allowlist 稳定类别和结构化日志 |
| 时间 API 契约 | `api/novel_autopilot_runs.rs` | UTC `NaiveDateTime` 输出 RFC 3339 `Z` |
| 页面显示 | `NovelAutopilotWorkbench.tsx` | 按 Run 状态显示正确失败文案，继续使用浏览器本地时区 |

不新增数据库表或列，不迁移历史时间，不修改与本任务无关的人工门。

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

- `chapter_generate` 最终未通过候选：调用既有人工候选持久化契约，保存完整正文、digest、word count 和受限质量信息。
- `chapter_repair` 预算内失败：继续保存受 Run/epoch/source digest/analysis ID 约束的完整重试证据，供下一次自动返修消费。
- `chapter_repair` 最终耗尽：调用 `persist_manual_review_candidate()`，把最后一次完整返修结果转换为人工候选。
- Provider 失败没有完整候选：不得伪造候选或 digest。
- Accept 使用候选 ID、Run version/epoch、Step identity 和章节快照 CAS；成功后提交正文并继续运行。
- Retry/Repair/Stop 继续使用现有人工决定路由和版本围栏。

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

- 不做数据库迁移；历史 Run/Step 仍可读取。
- 已经持久化为 `waiting_human` 的历史 Run 不自动改写，避免无审计的数据变更。
- 旧无时区 API 值只存在于旧响应，数据库值不变；部署后新响应带 `Z`。
- API 字段名和前端 TypeScript 类型不变，时间字符串从 ISO local-like 收紧为 RFC 3339 UTC。
- 新 Provider 诊断码是向后兼容扩展，未知码已有回退显示。
- 人工候选沿用现有表和 API，不需要数据库迁移。
- 如候选耗尽路由出现回归，可回滚到已有 direct-manual-review 候选路径，不改变表结构。

## 11. 测试矩阵

### Rust 单元/Repository/API

- `applicable=false` 指标不进入失败项、最弱项、弱项计数和修复方向。
- 缺少 `details` 的历史指标保持当前行为。
- 质量和 Provider 预算内失败仍调度重试。
- 最终质量耗尽原子写入人工候选、Run `waiting_human` 和可关联 Step。
- 最终章节生成/返修候选可通过 Accept/Retry/Repair/Stop 操作；预算内重试证据仍可被下一次消费。
- Provider、配置、上下文或响应无效的无候选故障不创建 `chapter_draft_attempts`；不可重试或预算耗尽时将 Run 投影为 `waiting_human`，且仅允许 Retry/Repair/Stop。
- 构造无候选 `accept` 请求时 API 返回 `409 human_decision_candidate_unavailable`，Run
  保持 `waiting_human` 且 version、guidance 和后台任务均不变化；协调器直接收到同类
  decision 时也必须 fail-closed。
- Repository CAS、candidate insert 或 Step update 失败时事务完整回滚。
- Provider 分类覆盖 timeout、429、503、invalid result 和 unknown，并验证原文/密钥不出现在诊断。
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
