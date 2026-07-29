# Implementation Plan: Story Packet / Generation Intent 统一生成契约

## Phase A — Core contract

- [x] 新增 `generation_contract_service` 模块和 schema owner。
- [x] 定义 version、target、source、Story Packet、Generation Intent 和 snapshot 类型。
- [x] 新增 canonical JSON normalizer 与 SHA-256 digest owner。
- [x] 添加 serde round-trip、键顺序、嵌套 JSON、数组顺序、默认值和敏感字段测试。
- [x] 在 `services/mod.rs` 注册唯一 owner，不引入第二套 runtime。

## Phase B — Single chapter integration

- [x] 将单章 runtime 的局部 `build_single_generation_story_packet() -> Value` 迁移为共享 typed builder。
- [x] 用 typed `GenerationIntentKind::ChapterGenerate` 替换硬编码自由 JSON intent。
- [x] 在质量/候选执行兼容边界保留现有 flat JSON projection。
- [x] 将契约 snapshot 合并写入现有单章 runtime state。
- [x] 添加单章构建、override 优先级、历史 snapshot + continuity ledger 补齐测试。

## Phase A/B Evidence — 2026-07-15

- `generation_contract_service` owns the only versioned typed schema, canonical digest,
  runtime/history snapshot helpers and legacy projection adapters.
- Single-chapter background execution persists the typed contract into the existing task/runtime
  snapshot; stream execution reuses the same typed contract without creating a second persistence path.
- Legacy candidate-quality consumers receive the exact previous flat packet and
  `{"mode":"single_generation_active_route"}` projection; typed schema metadata and ledger wrapper
  identities are not exposed across the compatibility boundary.
- Focused verification passed:
  - `cargo test --manifest-path backend-rs/Cargo.toml generation_contract_service -- --nocapture`
    — 17 passed.
  - `cargo test --manifest-path backend-rs/Cargo.toml chapter_generation_runtime_service -- --nocapture`
    — 78 passed.
  - `cargo test --manifest-path backend-rs/Cargo.toml chapter_single_generation -- --nocapture`
    — 133 passed.
  - `cargo fmt --manifest-path backend-rs/Cargo.toml -- --check` — passed.
  - `cargo check --manifest-path backend-rs/Cargo.toml` — passed; remaining warnings belong to
    not-yet-integrated R4 Phase C-F surfaces or pre-existing regeneration/recovery owners.
  - Targeted `git diff --check` — passed.

## Phase C — Batch and restore integration

- [x] 批量创建时持久化稳定 Story Packet snapshot 和批次/章节 intent。
- [x] resume 优先恢复 typed snapshot，并验证 digest 不漂移。
- [x] 旧 snapshot 缺少新字段时继续 compat options fallback。
- [x] 验证 merge 不覆盖 progress、quality、candidate gateway 和现有 checkpoint 投影。

## Phase C Evidence — 2026-07-15

- Batch create persists one stable `ChapterBatch` Story Packet snapshot; every chapter attempt derives
  a distinct `ChapterGenerate` intent and digest while preserving the ordered batch target.
- Resume restores a valid typed snapshot without digest drift. Missing, legacy, unsupported-version and
  malformed snapshots continue through the existing compat-options fallback without panic.
- Runtime namespace merge preserves `progress`, `quality`, `candidate_gateway`, `checkpoint` and other
  existing fields. The DB-backed create fixture now creates the complete continuity-ledger read schema;
  no production migration or runtime fallback was added.
- Focused verification passed:
  - `chapter_generation_runtime_service` — 81 passed.
  - `chapter_batch_generation_runtime_state_service` — 135 passed.
  - `chapter_batch_generation_write_workflow_service` — 67 passed.
  - `chapter_batch_generation_resume_task_command_service` — 72 passed.
  - `generation_contract_service` — 17 passed.
  - Targeted resume, fallback, runtime merge and DB-backed create regression tests — passed.
  - `cargo check --manifest-path backend-rs/Cargo.toml` — passed; 19 existing or later-R4
    dead-code/unused warnings remain and were not hidden with broad suppressions.
  - `cargo fmt --manifest-path backend-rs/Cargo.toml -- --check` and targeted `git diff --check` — passed.
- No API DTO, response, SSE event, frontend store, database schema or migration changed in Phase C.

## Phase D — Regeneration integration

- [x] 为全章重生成建立兼容 adapter 和 `ChapterRegenerate` intent。
- [x] 为局部重生成建立 typed range/selection/preserve constraints 和
      `ChapterPartialRegenerate` intent。
- [x] 复用原 Prompt/Provider/stream workflow，保持 SSE 和错误码不变。
- [x] 添加全章、局部、非法范围和旧 DTO 等价测试。

## Phase D Evidence — 2026-07-15

- Full regeneration maps the legacy route request into the shared typed Story Packet and
  `ChapterRegenerate` intent. Partial regeneration maps its normalized selected text, typed range,
  `chapter_selection` target and preserve constraints into `ChapterPartialRegenerate` without
  introducing a second packet or runtime owner.
- Both adapters reuse the authoritative `load_generation_context` loader, compatibility options and
  request override owner. Provider, model, API key, `version_note` and `auto_apply` remain runtime-only
  and do not affect the stable contract digest.
- The full internal request `Default` now delegates to the route DTO default adapter, preserving
  `preserve_character_traits=true`, `preserve_structure=false` and `auto_apply=false`. Partial prompt
  construction and contract selection share the same final normalized selected text.
- Existing prompt assembly, provider dispatch, stream workflow, SSE events, API DTOs and error mapping
  remain unchanged. Non-chapter Story Packets and invalid partial ranges are rejected before dispatch.
- Focused verification passed — 101/101 total:
  - `chapter_regeneration_prepare_service` — 28 passed.
  - `chapter_regeneration_stream_workflow_service` — 14 passed.
  - `chapters_error_mapper` — 20 passed.
  - `chapter_regeneration_routes` — 22 passed.
  - `generation_contract_service` — 17 passed.
- `cargo check --manifest-path backend-rs/Cargo.toml` — passed; 20 existing or later-R4 warnings remain
  and were not hidden with broad suppressions.
- `cargo fmt --manifest-path backend-rs/Cargo.toml -- --check`, targeted `git diff --check`, UTF-8
  without BOM and LF-only checks — passed.
- No migration, database table/column, frontend store, API response, SSE event or error code changed.

## Phase E — Outline integration

- [x] 为 outline generate 和 continue/expand 建立 outline target adapter。
- [x] 统一 narrative/creative/story/quality override 归并规则。
- [x] 保留旧 provider/model 请求语义，不在 R4 实现角色模型策略。
- [x] 添加 new/continue/expand/batch expand 合同测试。

## Phase E Evidence — 2026-07-15

- `backend-rs/src/api/outlines/contract_prepare_owner.rs` owns the outline compatibility adapter for
  `New`, `Continue`, `Single` expand and `Batch` expand. Every route builds a typed outline target,
  Story Packet and Generation Intent before projecting resolved values into the existing execution path.
- Generate defaults merge in the fixed order `system defaults -> project defaults -> non-empty request
  overrides`; empty continue strings do not erase project facts. Single and batch expansion use the actual
  outline id, and batch construction produces an independent target and digest for every outline.
- Existing provider/model fields remain runtime-only inputs to the legacy AI execution path. They are not
  copied into Story Packet, Generation Intent or `input_digest`; no R5 role-routing policy was introduced.
- Existing API DTOs, responses, SSE behavior and outline execution signatures remain unchanged. No database
  schema, migration, frontend store or task payload was added.
- Focused verification passed:
  - `contract_prepare_owner` — 9 passed, including new/continue/single/batch contract cases.
  - `outlines` — 32 passed.
  - `generation_contract_service` — 17 passed.
  - `cargo check --manifest-path backend-rs/Cargo.toml` — passed.
  - `cargo fmt --manifest-path backend-rs/Cargo.toml -- --check`, targeted `git diff --check`, UTF-8
    without BOM and LF-only checks — passed.
- Validation logs are stored under
  `.trellis/tasks/07-14-story-packet-generation-intent/validation/phase-e-*.log`.

## Phase F — Review, repair and history

- [x] 让章节分析/质量上下文从共享 Story Packet projection 读取故事事实。
- [x] 用 `ChapterReview` / `ChapterRepair` intent 表达审校与修复，不复制 packet。
- [x] 在章节 generation history JSON payload 增加 optional contract snapshot 摘要。
- [x] 添加旧历史读取、新历史 round-trip 和无敏感字段测试。

## Phase F Evidence — 2026-07-15

- Chapter analysis and quality preparation now consume the shared Story Packet projection; review and repair
  use the typed `ChapterReview` / `ChapterRepair` intents while preserving the existing execution workflow.
- Generation history stores only the optional versioned contract summary and remains compatible with legacy
  payloads that do not contain the new field.
- Focused tests passed on the current worktree:
  - analysis runtime — 32 passed;
  - candidate executor — 4 passed;
  - contract preparation — 2 passed;
  - generation runtime — 83 passed;
  - history — 5 passed;
  - targeted repair — 7 passed.
- Evidence: `validation/phase-f-*.log`.

## Phase G — Compatibility and quality gate

- [x] 确认旧 API DTO、response、SSE event、Zustand/task store 行为未变。
- [x] 运行 `cargo fmt --manifest-path backend-rs/Cargo.toml -- --check`。
- [x] 运行 R4 focused Rust tests（generation contract、single、batch、restore、regeneration、outline、analysis、history）。
- [x] 运行 `cargo check --manifest-path backend-rs/Cargo.toml`。
- [x] 运行 `cargo test --manifest-path backend-rs/Cargo.toml --quiet`。
- [x] 运行 `npm run lint --prefix frontend` 和 `npm run build --prefix frontend`。
- [x] 若兼容面涉及页面请求，运行对应 Playwright targeted E2E。
- [x] 检查 UTF-8 无 BOM、LF、trailing whitespace 和 `git diff --check`。
- [x] 回填 PRD 验收、实现证据和优化路线，将下一主线切换到 R5/R6。

## Phase G Evidence — 2026-07-15

- Compatibility audit: PASS. R4 remains an internal Rust contract; no public API request/response fields,
  SSE event kinds, frontend request types, Zustand state, or task-store payloads were added or changed by R4.
  See `validation/phase-g-compatibility-audit.md`.
- Targeted Playwright E2E: N/A. R4 does not change a page request/response or frontend state contract; legacy
  DTO projection is covered by Rust route/adapter tests and frontend lint/build remain the cross-layer gate.
- Focused Rust suites passed on the current worktree:
  - generation contract — 17 passed;
  - single generation — 161 passed;
  - batch generation — 420 passed;
  - restore — 60 passed;
  - regeneration — 79 passed;
  - outlines — 32 passed;
  - analysis — 89 passed;
  - history — 74 passed.
- Final Rust gate passed: `cargo fmt --check`, `cargo check`, and 1689/1689 full tests.
- Final frontend gate passed: lint reported 0 errors / 33 existing warnings; production build completed in 5.42s.
- UTF-8 without BOM, LF-only, trailing-whitespace and targeted `git diff --check` audit passed after final
  documentation backfill. Evidence is stored in `validation/phase-g-r4-encoding-audit.log`.

## Risky Files and Rollback Points

- `backend-rs/src/services/chapter_generation_runtime_service/runtime_execution_owner.rs`：已有单章 packet
  和历史恢复逻辑；先保留兼容投影，再替换 owner。
- `backend-rs/src/services/chapter_generation_runtime_service/candidate_runtime_owner.rs`：候选质量输入广泛；
  不一次删除外层兼容字段。
- `backend-rs/src/services/chapter_batch_generation_runtime_state_service/`：恢复合同敏感；新字段只能命名空间
  merge，旧 snapshot fallback 必须先有测试。
- `backend-rs/src/api/outlines.rs`：当前已有未提交改动；编辑前逐块核对 diff，禁止覆盖 R3 兼容写入口。
- `backend-rs/src/services/chapter_regeneration_prepare_service/`：保持 stream workflow 和错误映射。
- `frontend/src/types/index.ts` 与各 generation store：默认不修改，确有新增可选 metadata 时才扩展。

## Start Review Gate

- [x] 用户已明确授权后续直接开发。
- [x] R4 范围、R5/R6 边界和无 migration 方案已由代码/文档调研确认。
- [x] 旧 API、SSE、恢复和前端兼容边界已记录。
- [x] 规划包含验证命令和逐入口回滚点。
