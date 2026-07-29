# 全流程后台异步任务统一化实施计划

## Steps

1. 在 Rust `background_tasks` 中允许 Inspiration 无项目任务，并新增 task type 分发。
2. 复用 `InspirationService` 执行三个动作，完成后写入原响应 JSON。
3. 更新前端 `BackgroundTaskType` union，新增 Inspiration task type。
4. 在 `inspirationApi` 增加后台任务封装，内部创建任务并等待完成。
5. 将 `Inspiration.tsx` 的生成选项、反馈优化、确认阶段智能补全调用切到后台封装。
6. 在 Rust regeneration workflow 中新增局部重写后台任务执行入口。
7. 在 `background_tasks` 中新增 `chapter_partial_regenerate` 分发。
8. 在前端 `chapterPartialRegenerationApi` 增加后台任务封装，并切换 `PartialRegenerateModal`。
9. 在 Rust `background_tasks` 中增加拆书导入 apply/retry wrapper，并允许无项目后台任务。
10. 将 `BookImport.tsx` 切换到后台任务封装，保留失败步骤重试与完成后跳转。
11. 将 Wizard 世界观重生成切换到既有 `world_regenerate` 后台任务。
12. 将 feature command 中的大纲/角色生成切换到既有 `outline_generate` / `character_generate` 后台任务。
13. 新增 AI 去味 `polish_text` / `polish_batch` 后台任务封装。
14. 在 Rust `background_tasks` 中新增捕获型 SSE 进度桥接，并接入 wizard/world/book-import 复用 SSE 的后台任务。
15. 新增 `chapter_regenerate` 后台任务，复用整章重生成 Rust workflow，并将 `ChapterRegenerationModal` 从直连 SSE 切到后台任务。
16. 运行 focused Rust 检查和 TypeScript 构建检查。

## Validation

```powershell
cargo check --manifest-path "backend-rs/Cargo.toml"
npm exec tsc -b
```

## Notes

- `spawn_channel_progress_bridge()` 只负责运行中 `progress` fanout，不接管任务终态。
- 终态仍沿用 `sync_channel_state_to_task()` 读取最终 result，再由 `complete_task()` 或 `fail_task()` 发布。
- 这样可以复用既有 wizard/book-import SSE 进度事件，同时避免为后台任务复制业务流程。
- `chapter_regenerate` 后台任务直接复用 full regeneration prepare/candidate/finalize/persistence owner，旧 `/regenerate-stream` 仍保留兼容。

## Rollback

前端可回退到原 `inspirationApi.generateOptions/refineOptions/quickGenerate` 同步方法；后端原同步/SSE 路由不删除。
## Completion Record (2026-07-12)

### Contract hardening

- Extracted `task_type_allows_empty_project()` from the task creation path so
  global-task admission has one testable owner.
- Added positive and negative project-scope contract tests for background task
  types.
- Updated three stale Rust contract tests to match the current recovery-owner,
  execution-owner, and manual-review lifecycle contracts.

### Final validation

```text
cargo fmt --manifest-path backend-rs/Cargo.toml -- --check  PASS
cargo test --manifest-path backend-rs/Cargo.toml api::background_tasks::tests  16/16 PASS
cargo test --manifest-path backend-rs/Cargo.toml  1524/1524 PASS
cargo check --manifest-path backend-rs/Cargo.toml  PASS
npm run build --prefix frontend  PASS
```

### Remaining non-blocking warnings

- Existing Rust dead-code warnings remain.
- Existing Vite circular chunk warning remains:
  `vendor-utils -> vendor-react -> vendor-utils`.
- No Git commit or branch operation was performed.
## Revalidation Record (2026-07-14)

Validated against the current mixed worktree after later background-task
reliability changes:

```text
cargo fmt --manifest-path backend-rs/Cargo.toml -- --check  PASS
cargo test --manifest-path backend-rs/Cargo.toml api::background_tasks::tests  25/25 PASS
cargo test --manifest-path backend-rs/Cargo.toml  1613/1613 PASS
cargo check --manifest-path backend-rs/Cargo.toml  PASS
npm run build --prefix frontend  PASS
npm run lint --prefix frontend  PASS (0 errors, 33 existing warnings)
```

The task remains functionally complete. Trellis archive/commit bookkeeping is
deferred because the worktree contains unrelated parallel changes and Git
operations require explicit confirmation.
