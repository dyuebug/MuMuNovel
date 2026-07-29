# Existing Capability Map

## Conclusion

MuMuNovel 已具备自动成书约 70%–80% 的底层业务能力，但缺少小说级 Durable Orchestrator。当前 R7 `novel_autopilot` 只能执行一次已确认 Tool 调用，不能被扩展解释为无人值守多步骤循环。

## Existing owners to reuse

| Capability | Existing owner/evidence | Autopilot use |
| --- | --- | --- |
| Novel workflow | `backend-rs/src/services/novel_workflow_service.rs` | CAS 推进 foundation/world/outline/writing/reviewing/polishing/completed |
| R7 direct Tool | `backend-rs/src/services/autopilot_coordinator_service.rs` | 保持旧契约，不用于 durable loop |
| R7 API/audit | `backend-rs/src/api/autopilot.rs`, `autopilot_invocation_audit_service.rs` | 历史审计保持独立 |
| Wizard planning | `backend-rs/src/services/wizard_service.rs`, `backend-rs/src/api/wizard.rs` | 复用 world/career/character/outline generation |
| Organizations | `backend-rs/src/api/organizations.rs` | 抽取/复用生成 owner |
| Outline | `backend-rs/src/api/outlines.rs` | 复用 generate/expand request executors |
| Chapter generation | `chapter_single_generation_*`, `chapter_batch_generation_*` | 复用 Generation Contract、checkpoint、resume、retry |
| Quality gate | `chapter_generation_runtime_service`, `chapter_*_runtime_state_service` | 复用 accept/auto_repair/retry terminal semantics |
| Cooperative cancellation | `backend-rs/src/tasks`, `background_tasks.rs` | 每步提交前检查并废弃迟到结果 |
| Polish | `backend-rs/src/api/polish.rs` | 复用 text/batch polish service owner |
| Export | `backend-rs/src/api/projects.rs` | 复用 ProjectExportContext/serialization |
| Runtime metrics | frontend project metrics feature + backend runtime metrics | 展示 Run/章节/预算指标 |
| Model output | `useBackgroundTaskOutputStream` + existing output panel | 可选显示 content/reasoning；不持久化 reasoning |

## Hard compatibility boundary

`.trellis/spec/backend/autopilot-invocation-audit.md` 明确规定：

- `novel_autopilot` 是 `NonResumable`；
- 无 pause/resume/steer/checkpoint/retry/replay；
- 只允许一次人工确认后的 direct business Tool；
- audit 不保存原始 arguments、Prompt、reasoning 或 raw error。

因此新能力必须使用独立 task type（设计名 `novel_book_autopilot`）和独立 Run/Step 表，不修改旧 task type 的语义。

## Database pattern

- 生产 schema 由 `backend/alembic/postgres/versions/` 管理。
- 当前已审计模板：`20260716_2200_autopilot_invocation_audit.py`。
- Rust 同时维护 SeaORM model 与 `schema_migration_metadata_service.rs` migration catalog/head tests。
- 不得依赖启动时 runtime schema sync 修复生产表。

## Main gaps

1. Durable Run/Step persistence and restart recovery.
2. Pure phase router and idempotent step protocol.
3. Empty-project planning adapters across all prerequisite domains.
4. Per-chapter loop with quality decision and bounded repair/retry.
5. Budget, time, token and human-gate controls.
6. Full-book review/polish/completion/export closure.
7. Frontend workbench with persisted state and optional live model channels.

## Reference comparison with ainovel-cli

`ainovel-cli` uses a durable flow router and chapter commit checkpoint:

- `internal/host/flow/router.go`
- `internal/tools/novel_context_builders.go`
- `internal/tools/commit_chapter.go`
- `internal/tools/save_review.go`

MuMuNovel should reproduce the orchestration semantics, not copy Go implementation details or introduce a second content store.
