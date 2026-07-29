# Role Model Policy 与 Generation Execution Audit

## Goal

实现优化路线 R5：在 R4 `GenerationIntentKind` 之上建立唯一的 planner / writer /
reviewer 角色模型策略，保持默认设置简单和旧 route override 行为不变，并让每次核心生成执行的
requested / resolved / actual Provider、model、fallback 分类和策略版本可追溯。

## User Value

- 用户无需为三个角色重复配置即可继续使用现有全局 Provider/model。
- 高级用户可以按角色覆盖 Provider/model，而旧页面和旧 API 的显式 override 仍然优先。
- 生成历史能够解释“请求了什么、策略解析成什么、最终实际执行了什么以及为何 fallback”。
- 模型策略变化不会改变同一 Story Packet / Generation Intent 的 R4 `input_digest`。

## Confirmed Facts

- R4 已完成并冻结 `GenerationIntentKind`、Story Packet、Generation Intent 和 canonical
  `input_digest`；Provider/model/fallback 是 runtime-only，不属于摘要输入。
- `SettingsService::build_ai_config` 当前支持 provider/model/temperature override，但没有角色策略，
  且 `AIConfig.backup_urls` 尚未从已保存设置恢复。
- `settings.preferences` 已保存 versioned API presets 与 web research，现有 helper 按顶层 key 合并，
  可以承载无 migration 的 `role_model_policy` 子文档。
- `AIService` 已有模型 fallback，OpenAI client 已有 endpoint attempt diagnostics；成功模型 fallback
  目前不会返回给调用者，transport diagnostics 还包含完整 URL，不能直接写入 history。
- 生成链路包含 non-stream 与 stream 路径，旧 `AIService` 方法签名已有多个非 R5 调用者。

## Requirements

### R1. Canonical role mapping

- 新增唯一 `GenerationRole` owner，固定映射：
  - `OutlineGenerate`、`OutlineExpand` → `planner`
  - `ChapterGenerate`、`BatchChapterGenerate`、`ChapterRegenerate`、
    `ChapterPartialRegenerate`、`ChapterRepair` → `writer`
  - `ChapterReview` → `reviewer`
- route、runtime 与 history 不得各自复制字符串匹配。

### R2. Versioned role policy

- 定义 `role-model-policy/v1` schema，角色 override 只允许非敏感执行选择字段。
- 默认策略为空角色 override，全部继承全局 settings。
- 持久化复用 `settings.preferences.role_model_policy`，不新增数据库 migration。
- policy canonical digest 仅覆盖规范化后的策略选择，不包含 API Key、Authorization、完整 endpoint
  URL、Prompt、生成内容或时间戳。
- 非法 schema、未知角色、空 provider/model、对象类型错误必须显式拒绝或安全降级到默认策略，
  不得 panic。

### R3. Resolution precedence

Provider 和 model 分字段解析，兼容优先级固定为：

```text
旧 route 显式 override
  > role override
  > global settings
  > provider runtime default
```

- provider/model 的 requested、resolved 值及来源必须可区分。
- 当 provider 改变且没有更高优先级 model override 时，不得错误继承旧 provider 的模型；应使用
  目标 provider 的角色/global 合法模型或 runtime default。
- 现有无角色调用者继续走旧 `build_ai_config` 行为。

### R4. Tracked AI execution

- 保留旧 `generate_text*` / stream API 兼容方法。
- 新增 tracked non-stream 与 tracked stream 边界，至少记录：
  - actual provider/model
  - model fallback 是否发生及脱敏原因分类
  - endpoint failover 是否发生、attempt/failover 数量和 endpoint role/index
- model fallback、endpoint failover、Rust/Python candidate executor fallback 必须是不同枚举类别。
- tracked metadata 不得暴露 API Key、Authorization、完整 endpoint URL、Prompt 或响应正文。

### R5. Generation execution audit

- 将 role、policy schema version、policy digest、requested/resolved/actual Provider/model、解析来源、
  fallback 分类和脱敏摘要合并为 `generation-execution-audit/v1`。
- 单章、批量、重生成、大纲和 review/repair 入口应逐步通过同一 owner 构建审计，不新增第二套
  Generation Intent 或任务状态。
- 章节生成 history payload 使用 additive JSON key 保存摘要；旧记录缺少该 key 时继续可读。
- R5 不创建 R6 business checkpoint，也不把 audit 当作 checkpoint。

### R6. Compatibility and security

- 不删除或重命名现有公开 API 字段，不新增 SSE event kind。
- 旧 route 显式 provider/model override 的优先级和行为保持不变。
- R4 `input_digest` 在策略、Provider、model、endpoint 或 fallback 改变时保持不变。
- 所有 persisted/logged audit 值必须通过 allowlist 构造；不得从任意 diagnostics JSON 直接透传。
- 文件保持 UTF-8 无 BOM、LF-only、无 trailing whitespace。

## Acceptance Criteria

- [x] 所有 `GenerationIntentKind` 都由单一 owner 映射为预期角色，并有穷举测试。
- [x] `role-model-policy/v1` 能从空、合法和非法 preferences 构建安全策略；canonical digest 稳定。
- [x] 解析优先级 `route > role > global > default` 有 provider/model 分字段参数化测试。
- [x] provider 切换不会继承不兼容的旧 provider model，并有回归测试。
- [x] `AIService` 旧方法行为兼容；tracked non-stream 能记录成功、模型 fallback 和失败分类。
- [x] tracked stream 能在终态提供实际模型和脱敏 endpoint failover 摘要，且不新增 SSE event kind。
- [x] generation audit allowlist 中不存在 API Key、Authorization、Prompt、正文或完整 endpoint URL。
- [x] 核心生成 history additive 保存 `generation_execution_audit`，旧 payload 缺字段时兼容读取。
- [x] 改变 role policy 或实际 Provider/model/fallback 不改变同一 R4 contract 的 `input_digest`。
- [x] focused tests、`cargo fmt --check`、`cargo check`、完整 Rust tests 通过；若前端契约未变则记录 E2E N/A 依据。
- [x] 优化路线回填 R5 完成证据并把下一主线切换到 R6。

Acceptance evidence (2026-07-15):

- Role Policy、provider/model 分字段解析、tracked non-stream/stream、model/endpoint/candidate
  fallback 分类及 typed audit allowlist 均已通过 focused tests。
- single、batch create/resume、regeneration、Wizard Outline、Batch Outline、ChapterReview/Repair
  已按各自返回边界接入有序 `generation_execution_audit`。
- 核心章节生成 history 执行 additive durable persistence；Review/Regeneration audit 使用 additive
  result boundary。ChapterReview 后台 wrapper 没有独立 durable audit 字段，R5 不新增 checkpoint、
  migration 或数据库字段，也不将审计写入业务正文/分析字段。
- `cargo fmt --check`、`cargo check`、完整 Rust tests 1736/1736 通过；frontend 契约未变，
  lint/build/E2E 为 N/A。

## Out of Scope

- R6 versioned business checkpoint、revision、幂等恢复与数据库 ledger。
- G1-Cancel cooperative cancellation。
- R7 Autopilot、Agent/Coordinator、Tool 编排和无人值守整书生成。
- 动态 benchmark 路由、成本预算、智能模型推荐、自动 Prompt 策略。
- 新增生产 migration、执行 production downgrade 或修改生产环境。
- 前端角色策略编辑 UI；R5 首先提供后端兼容存储、解析和审计合同。

## Risks

- AI client、generation runtime 和 history 文件存在其他任务的未提交修改，必须逐文件审查 diff，
  使用窄接线并禁止覆盖无关变更。
- stream 成功诊断目前不完整；实现必须保证终态 trace 可用，同时避免把完整 URL带入审计。
- preferences 是共享 JSON；所有写入 helper 必须保留 `api_presets`、`web_research` 与未知顶层 key。
