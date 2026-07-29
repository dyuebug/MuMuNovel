# Implementation Plan: Role Model Policy 与 Generation Execution Audit

## Preconditions

- [x] R4 Story Packet / Generation Intent 已完成并冻结 `input_digest` runtime-only 边界。
- [x] R5 路线范围、优先级和 G1 验收目标已从优化路线确认。
- [x] 当前工作区存在跨任务混合修改，禁止 reset、覆盖无关 diff、commit 或 push。
- [x] R5 采用 complex Trellis 任务，规划产物在实现前完成。

## Phase A — Canonical Policy Owner

- [x] 新增 `role_model_policy_service` schema owner：`GenerationRole`、policy v1、selection source。
- [x] 实现 `GenerationIntentKind -> GenerationRole` 穷举映射。
- [x] 实现 preferences parse/merge，保留 `api_presets`、`web_research` 与未知顶层 key。
- [x] 实现 policy canonical normalization 与 SHA-256 digest，拒绝敏感/未知选择字段。
- [x] 实现 provider/model 分字段优先级 resolver 和 provider-switch model 防串用规则。
- [x] 添加空策略、合法/非法策略、digest 稳定性及优先级参数化测试。

Validation:

```powershell
cargo test --manifest-path backend-rs/Cargo.toml role_model_policy -- --nocapture
cargo fmt --manifest-path backend-rs/Cargo.toml -- --check
cargo check --manifest-path backend-rs/Cargo.toml
```

Evidence (2026-07-15):

- `role_model_policy` focused tests：9 passed / 0 failed。
- `cargo fmt --check`：PASS。
- `cargo check`：PASS；保留工作区既有 warning，Phase A owner 会在 Phase B 接入生产路径。
- UTF-8 无 BOM、LF-only、无 trailing whitespace：PASS。
- 日志：`validation/phase-a-role-model-policy-test.log`、`validation/phase-a-cargo-fmt-check.log`、
  `validation/phase-a-cargo-check.log`、`validation/phase-a-encoding-audit.log`。

## Phase B — Settings Integration

- [x] 将 AI config 构造共用逻辑抽成最小私有 owner，不改变旧 `build_ai_config` 签名。
- [x] 新增 role-aware config builder，返回 `AIConfig + ResolvedRoleModelPolicyV1`。
- [x] 从 settings 恢复 role policy；默认空策略继承全局 Provider/model。
- [x] 仅在 role-aware path 恢复 backup URLs / fallback policy，避免无关调用者行为漂移。
- [x] 添加旧 builder 兼容、role override、route override 和 provider default 测试。

Validation:

```powershell
cargo test --manifest-path backend-rs/Cargo.toml settings_service -- --nocapture
cargo test --manifest-path backend-rs/Cargo.toml chapter_generation_execution_contract_service -- --nocapture
```

Evidence (2026-07-15):

- `SettingsService::build_ai_config` 保持原签名、原模型解析和 `backup_urls = []` 兼容行为。
- 新增 `RoleAwareAIConfig` 与 `build_role_aware_ai_config`，从 preferences 恢复策略并返回解析结果。
- role-aware 路径恢复 `api_backup_urls`；`fallback_strategy=auto` 映射为允许模型 fallback，其他值失败关闭。
- Settings service tests：17 passed / 0 failed；其中新增 Phase B 测试 5 passed / 0 failed。
- role policy regression：9 passed / 0 failed；generation config bridge regression：12 passed / 0 failed。
- `cargo fmt --check`、`cargo check`：PASS；未接线 owner 的 dead-code warning 留待 Phase C/D 消除。
- 日志：`validation/phase-b-settings-service-tests.log`、
  `validation/phase-b-role-model-policy-regression.log`、
  `validation/phase-b-generation-config-contract-regression.log`、
  `validation/phase-b-cargo-fmt-check.log`、`validation/phase-b-cargo-check.log`。

## Phase C — Tracked AI Execution

- [x] 新增 typed `AIExecutionTraceV1`、fallback taxonomy 和 endpoint allowlist summary。
- [x] 新增 tracked non-stream API，记录 primary success、model fallback success/failure。
- [x] 新增 tracked stream completion trace，不改变既有 `AIStreamChunk` 外部语义。
- [x] 从 OpenAI diagnostics 提取脱敏 endpoint 摘要，禁止 raw diagnostics 持久化。
- [x] 保留 Anthropic/Gemini/旧 AIService 方法兼容。
- [x] 添加 secret/URL/prompt/content 不泄漏的序列化与回归测试。

Validation:

```powershell
cargo test --manifest-path backend-rs/Cargo.toml ai::execution_trace -- --nocapture
cargo test --manifest-path backend-rs/Cargo.toml ai::service -- --nocapture
cargo test --manifest-path backend-rs/Cargo.toml ai::clients::openai -- --nocapture
cargo fmt --manifest-path backend-rs/Cargo.toml -- --check
cargo check --manifest-path backend-rs/Cargo.toml
```

Evidence (2026-07-15):

- `AIExecutionTraceV1`、tracked non-stream/stream 与旧 API 兼容路径完成，Phase C 清单 6/6。
- `ai::execution_trace`：3 passed / 0 failed；`ai::service`：6 passed / 0 failed。
- `ai::clients::openai` regression：5 passed / 0 failed；focused tests 合计 14 passed / 0 failed。
- `cargo fmt --check`、`cargo check`：PASS；48 个 warning 属于工作区既有或 Phase D 尚未接线 owner。
- targeted diff check、UTF-8 无 BOM、LF-only、无 trailing whitespace：PASS。
- 日志：`validation/phase-c-execution-trace-tests.log`、
  `validation/phase-c-ai-service-tests.log`、`validation/phase-c-openai-regression.log`、
  `validation/phase-c-cargo-fmt-check.log`、`validation/phase-c-cargo-check.log`、
  `validation/phase-c-targeted-diff-check.log`、`validation/phase-c-encoding-audit.log`、
  `validation/phase-c-quality-review.log`。

## Phase D — Generation Audit and History

- [x] 新增 `generation_execution_audit_service` typed builder/read/merge owner。
- [x] 单章生成 prepared config 携带 role policy resolution，不复制 R4 contract owner。
- [x] single generation direct candidate 返回 tracked execution，并写入 additive history key。
- [x] 批量、恢复、重生成、大纲、review/repair 按既有契约逐入口接线。
- [x] candidate executor fallback 由 gateway 显式分类，与 model/endpoint fallback 分离。
- [x] 旧 history 缺 audit、未知 schema、R4 summary 共存有兼容测试。

Validation:

```powershell
cargo test --manifest-path backend-rs/Cargo.toml generation_execution_audit -- --nocapture
cargo test --manifest-path backend-rs/Cargo.toml single_generation -- --nocapture
cargo test --manifest-path backend-rs/Cargo.toml batch_generation -- --nocapture
cargo test --manifest-path backend-rs/Cargo.toml regeneration -- --nocapture
cargo test --manifest-path backend-rs/Cargo.toml outlines -- --nocapture
cargo test --manifest-path backend-rs/Cargo.toml analysis -- --nocapture
cargo test --manifest-path backend-rs/Cargo.toml history -- --nocapture
```

Phase D progress evidence (2026-07-15):

- single generation、typed audit、history compatibility 与 candidate fallback taxonomy 已完成；
  focused tests 分别为 161、5、77 项通过。
- batch create/resume 已使用 `BatchChapterGenerate` role-aware config，role policy context
  贯穿 execution input、runtime session、chapter attempt 与既有 tracked audit/history owner。
- batch focused tests：execution contract 12、runtime 135、write workflow 67、resume 77 项通过；
  `cargo check`、targeted rustfmt/diff check 均通过。
- batch 证据：`validation/phase-d-batch-execution-contract-tests.log`、
  `validation/phase-d-batch-runtime-tests.log`、
  `validation/phase-d-batch-write-workflow-tests.log`、
  `validation/phase-d-batch-resume-tests.log`、
  `validation/phase-d-batch-cargo-check.log`、
  `validation/phase-d-batch-rustfmt-check.log`。
- regeneration focused tests 80/80；单次、批量、恢复与重生成均沿用正式 role-aware、
  tracked execution 与 additive history 链。
- Wizard Outline Generate 的主调用、空响应重试、JSON 解析重试均使用独立 tracked execution，
  结果按调用顺序 additive 返回 `generation_execution_audit`；wizard tests 14/14。
- Batch Outline Expand 为每个 outline 保留独立有序 audit 数组，不跨 outline 压平执行；
  `outline_execution_audit` 3/3、`api::outlines` 35/35。
- ChapterReview/Repair 使用同一 role-aware/tracked owner 构建审计。ChapterReview 审计位于
  analysis result boundary；后台 wrapper 会丢弃增强返回值，且既有数据库无通用 audit/metadata 字段。
  R5 不新增 migration，也不污染 `analysis_report`、suggestions 等业务字段，因此不宣称该入口拥有
  独立 durable audit record。核心章节生成 history 仍执行 additive durable persistence。

## Phase E — Compatibility and Security Audit

- [x] 证明 policy/provider/model/fallback 变化不改变 R4 `input_digest`。
- [x] 审计公开 API/SSE：不删除字段，不新增事件 kind；前端无契约变化，E2E N/A。
- [x] 审计 audit/history/log 中不存在 API Key、Authorization、Prompt、正文或完整 endpoint URL。
- [x] 审查当前共享文件 diff，确认未覆盖其他任务的 proxy bypass、R4 runtime/history 等变更。
- [x] 更新 PRD AC、实现证据和优化路线，将下一主线切换到 R6。

Evidence (2026-07-15):

- runtime-only digest 专项测试 1/1 通过：policy/provider/model/fallback 均不进入 R4
  `input_digest`；日志 `validation/phase-e-input-digest-runtime-only-test.log`。
- canonical audit 仅由 typed allowlist 构建，不保存 Prompt、正文、API key、Authorization 或完整 URL。
- 未新增 migration、SSE event kind，未删除旧 `AIService` API 或旧公开响应字段；多次 execution
  始终使用有序数组。
- R5 未修改 frontend 类型、页面或事件契约，因此 frontend lint/build 与 E2E 均为 N/A；
  工作区中的 frontend diff 属于其他并行任务，不纳入 R5 变更范围。

## Phase F — Full Quality Gate

- [x] `cargo fmt --manifest-path backend-rs/Cargo.toml -- --check`
- [x] `cargo check --manifest-path backend-rs/Cargo.toml`
- [x] `cargo test --manifest-path backend-rs/Cargo.toml --quiet`：1736 passed / 0 failed。
- [x] frontend 类型/页面/事件契约未变化，lint/build 与 E2E 记录为 N/A。
- [x] UTF-8 无 BOM、LF-only、trailing whitespace 与 targeted `git diff --check` 全通过。
- [x] Trellis quality check 与自评完成；按授权保持任务目录，不 archive、不 commit、不 push。

Evidence (2026-07-15):

- `validation/phase-f-cargo-fmt-check.log`：PASS。
- `validation/phase-f-cargo-check.log`：PASS，仅保留共享工作区既有 unused/dead-code warnings。
- `validation/phase-f-cargo-test-quiet.log`：1736 passed / 0 failed。
- 统一 focused tests：`generation_execution_audit`、`role_model_policy`、
  `chapter_analysis_runtime_service`、`outline_execution_audit`、`api::outlines`、
  `wizard_service`、`regeneration` 全部通过。
- 17 个 R5 定向 Rust 文件通过 UTF-8 无 BOM、LF-only、无 trailing whitespace 与
  targeted `git diff --check`。

## Risky Shared Files

```text
backend-rs/src/ai/clients/openai.rs
backend-rs/src/ai/clients/anthropic.rs
backend-rs/src/ai/clients/gemini.rs
backend-rs/src/services/settings_service.rs
backend-rs/src/services/chapter_generation_runtime_service/runtime_execution_owner.rs
backend-rs/src/services/chapter_generation_history_persistence_service.rs
backend-rs/src/services/mod.rs
```

每次修改前先读取当前 diff；优先新增 sibling owner 和窄接线，禁止整文件覆盖或批量格式化无关代码。

## Rollback Boundary

- Phase A/B 可移除 role-aware builder，旧 `build_ai_config` 继续工作。
- Phase C 可移除 tracked 方法，旧 AIService API 继续工作。
- Phase D 可停止写入 additive history key；旧记录和 R4 summary 无需迁移。
- 不执行数据库 migration、production downgrade、Git reset、commit 或 push。
