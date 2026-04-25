---
mode: plan
cwd: \\?\E:\Code\ProjectsCode\WorkSpace\Codex\NovelAi\MuMuNovel
task: Frontend and backend refactor replanning based on completed work
complexity: complex
planning_method: builtin
created_at: 2026-04-21T10:45:59+08:00
---

# Plan: Frontend and Backend Refactor Replan

## Goal

This document replaces the legacy roadmap header as the clean planning baseline.
The old roadmap still records iteration history, but future phase decisions should use this file.

## Completed Baseline

### Frontend

1. Floating index main path has been layered.
   - Shared utils exist for state, trigger props, lifecycle, and view helpers.
   - Shared hooks exist for bindings, lifecycle, state, and trigger props.
   - Most chapter-specific helpers and hooks are now compatibility aliases or pure re-exports.

2. `Chapters.tsx` page glue has been reduced.
   - Extracted `FloatingIndexPanelEntry`.
   - Extracted `ChapterReaderEntry`.
   - Extracted ChapterPlanEditorEntry.
   - Extracted SingleChapterGenerationOverlayEntry.
   - Extracted ChapterBatchProgressEntry.
   - The page now uses the shared floating index binding hook directly.

3. Service modularization has started.
   - `frontend/src/services/modularApi.ts` is the aggregate entry.
   - `frontend/src/services/core/` and `frontend/src/services/modules/` already exist.

4. Workflow and state extraction has started.
   - `frontend/src/store/chapterGenerationWorkflow.ts`
   - `frontend/src/store/projectSyncHelpers.ts`
   - `frontend/src/store/backgroundTaskSelectors.ts`

### Backend

1. Chapter-related backend routes already show split signals.
   - `backend/app/api/chapters.py` is still large.
   - `backend/app/api/chapter_analysis_task_routes.py` is already separated.

2. Service and compat layers are already present.
   - `chapters.py` delegates heavily to `app.services.*`.
   - Multiple `*_compat_service` modules still exist.

3. Validation tooling already exists.
   - `backend/tools/run_live_regression_retest.py`
   - `frontend/scripts/check-service-facade.mjs`

## Completion Estimate

### Frontend

Estimated progress for the current chapter-domain refactor track: 80% to 85%.

Done:
- Floating index layering is in a strong state.
- Nearby lazy modal and panel entry extraction is well underway.
- Service modular entry exists.
- Compatibility wrappers are close to stable end state.

Not done:
- `frontend/src/pages/Chapters.tsx` is still large.
- Generation, batch, story creation, snapshot, restore, and task flows still need more domain extraction.
- The `Chapters-*.js` chunk warning is still unresolved.

### Backend

Estimated progress for the chapter-mainline backend refactor track: 30% to 40%.

Done:
- Topic route splitting has started.
- Service and compat layers exist.
- Some chapter task routes are already separated.

Not done:
- `backend/app/api/chapters.py` remains too large.
- Compat services still indicate transition debt.
- Generation, candidate draft, quality feedback, and repair flows still need boundary cleanup.

## Phase Plan

## Phase A: Finish high-yield `Chapters.tsx` cleanup

Priority: Highest

Target:
- Keep shrinking page rendering glue and page-owned orchestration in `Chapters.tsx`.

Frontend tasks:
1. Continue extracting nearby lazy-render seams.
2. Keep moving generation workflow logic out of the page.
3. Prevent compatibility wrappers from gaining new responsibilities.

Exit criteria:
- `Chapters.tsx` loses another set of direct rendering blocks.
- Page code becomes more orchestration-oriented and less presentation-oriented.

## Phase B: Frontend chapter-domain modularization

Priority: Second

Target:
- Turn `Chapters.tsx` into a domain container page instead of a giant mixed page.

Frontend tasks:
1. Split by feature area.
   - Reader flow
   - Plan editor flow
   - Batch generation flow
   - Single generation flow
   - Story creation and snapshot flow
   - Analysis task flow
2. Introduce clearer domain folders.
3. Prepare natural boundaries for chunk splitting.

Exit criteria:
- `Chapters.tsx` becomes mostly a composition container.
- Domain UI, workflow, and helper boundaries are clear.

## Phase C: Backend chapter-mainline refactor

Priority: Parallel with frontend Phase A/B

Target:
- Reduce `backend/app/api/chapters.py` into a cleaner route plus orchestration structure.

Backend tasks:
1. Map `chapters.py` responsibilities.
   - CRUD
   - Generation
   - Candidate drafts
   - Quality feedback and repair
   - Runtime state sync
   - Research, memory, and foreshadow collaboration
2. Keep routing thin and move orchestration lower.
3. Build a compat-service replacement list.
4. Keep task routes separated instead of merging them back.

Exit criteria:
- `chapters.py` becomes smaller.
- Service boundaries are easier to explain.
- Compat-service debt starts shrinking.

## Phase D: Performance and validation

Priority: After structure stabilizes

Target:
- Convert structural cleanup into measurable maintainability and runtime wins.

Frontend tasks:
1. Start chunk-warning remediation for `Chapters-*.js`.
2. Check render hotspots and lazy-load cost.

Backend tasks:
1. Add stronger regression checks for chapter-generation critical paths.
2. Add timing and failure visibility for long chains.
3. Require regression coverage before deeper service rewrites.

## Recommended Order

1. Continue Phase A on the frontend.
2. Start backend responsibility mapping for `chapters.py`.
3. Move to Phase B after the page seam extraction reaches diminishing returns.
4. Enter Phase D only after structure becomes stable.

## Risks

- Do not delete compatibility alias files too early.
- Do not switch to chunk optimization before structural cleanup stabilizes.
- Do not split `backend/app/api/chapters.py` without a responsibility map first.
- Use this file as the clean planning baseline because the legacy roadmap header has encoding noise.

## Key References

- `frontend/src/pages/Chapters.tsx:1`
- `frontend/src/services/modularApi.ts:1`
- `frontend/src/store/chapterGenerationWorkflow.ts:1`
- `backend/app/api/chapters.py:1`
- `backend/app/api/chapter_analysis_task_routes.py:1`
- `frontend/src/components/FloatingIndexPanelEntry.tsx:15`
- `frontend/src/components/ChapterReaderEntry.tsx:12`
- `frontend/src/components/ChapterPlanEditorEntry.tsx:13`
- `frontend/src/components/SingleChapterGenerationOverlayEntry.tsx:8`
- `frontend/src/components/ChapterBatchProgressEntry.tsx:8`
- `frontend/src/components/ChapterListSection.tsx:39`
- `frontend/src/components/ChapterBasicModalEntry.tsx:21`
- `frontend/src/components/ChapterAnalysisEntry.tsx:10`
- `frontend/src/components/ChapterBatchGenerateModalEntry.tsx:9`
## Latest Update

Date: 2026-04-21

Frontend progress update:
- `Chapters.tsx` lost two more direct lazy-render seams.
- Single chapter generation overlay entry is now isolated.
- Batch generation progress entry is now isolated.
- Chapter list section is now isolated.
- Basic chapter modal entry is now isolated.
- Chapter analysis entry is now isolated.
- Batch generate modal entry is now isolated.
- `lint` and `build` both passed after the extraction.
- Bundle-report investigation confirmed that coarse local manualChunks rules are not a safe fix for the current Chapters warning and were reverted.

Current frontend recommendation:
1. Treat Phase A as close to completion.
2. Reassess whether `ChapterBatchGenerateModal` is still worth extracting.
3. If extraction value is low, move to Phase B modularization and Phase D chunk-warning remediation.

Current backend recommendation:
1. Keep backend implementation work paused until the next frontend transition point.
2. When frontend seam cleanup reaches diminishing returns, begin responsibility mapping for `backend/app/api/chapters.py`.
Frontend chunk remediation note:
- Current `Chapters-*.js` bundle is still around 607 kB before gzip-based warning comparison output.
- Broad local `manualChunks` for chapter UI/helpers caused oversized shared chunks because of transitive Ant Design dependencies.
- The safer next step is targeted lazy-boundary design, not coarse local grouping.
## Progress Refresh / 2026-04-21 / Iteration 111

Frontend progress update:
- `ChapterBatchGenerateModalEntry` now exposes a narrower page-facing API: `visible` plus a single typed `modalProps` contract.
- `ChapterBatchGenerateModalProps` is now reused directly by `Chapters.tsx`, so the page bundles the batch-generation modal contract before crossing the lazy-entry boundary.
- This completes the highest-value Phase A presentation cleanup around the remaining chapter modal seams and makes the next Phase B workflow extraction easier to stage.
- `lint` and `build` both passed after the props-boundary bundling change.
- The existing `Chapters-*.js` bundle warning remains, so chunk remediation should still wait until workflow/state boundaries become clearer.

Refreshed frontend recommendation:
1. Treat Phase A seam cleanup as complete for now.
2. Start Phase B with story-creation and batch-workflow state boundary extraction inside `Chapters.tsx`.
3. Delay deeper chunk-warning remediation until the new workflow boundaries reveal safer lazy-loading points.

Refreshed backend recommendation:
1. Keep backend implementation changes queued behind the next frontend Phase B checkpoint.
2. Once the frontend workflow extraction stabilizes, begin responsibility mapping for `backend/app/api/chapters.py`.

## Progress Refresh / 2026-04-21 / Iteration 112

Frontend progress update:
- Added a shared `story creation` derived-state helper for `Chapters.tsx` so single and batch flows now reuse the same prompt-state and customization-state calculation contract.
- `Chapters.tsx` no longer owns the full duplicated block for prompt resolution, customization flags, current draft building, and snapshot-save eligibility.
- This is the first concrete Phase B step: the page still owns orchestration, but part of the workflow-state contract is now explicit and reusable.
- `lint` and `build` both passed after the helper extraction.
- The existing `Chapters-*.js` bundle warning remains, so chunk remediation should still wait until more workflow/state glue has been extracted.

Refreshed frontend recommendation:
1. Continue Phase B with batch-generation request/polling glue extraction or story-creation snapshot orchestration extraction.
2. Prefer workflow-owned helpers/hooks over additional presentational entry wrappers.
3. Revisit chunk-warning remediation only after `Chapters.tsx` loses more orchestration/state density.

## Progress Refresh / 2026-04-21 / Iteration 113

Frontend progress update:
- Added a dedicated batch workflow helper so `Chapters.tsx` no longer directly owns the full start/cancel API orchestration for batch generation.
- The page now focuses on wiring state and callbacks, while task creation, task-meta persistence, optimistic progress setup, cancellation, and refresh sequencing are delegated to `chapterBatchGenerationWorkflowHelpers.ts`.
- This is the second concrete Phase B step: the batch-generation workflow contract is clearer, and the remaining page complexity is moving toward restore/polling and snapshot orchestration.
- `lint` and `build` both passed after the workflow helper extraction.
- The existing `Chapters-*.js` bundle warning remains, so chunk remediation should still wait until more workflow/state glue has been extracted.

Refreshed frontend recommendation:
1. Continue Phase B with story-creation snapshot orchestration extraction or batch restore/polling coordination extraction.
2. Keep pushing imperative workflow glue out of `Chapters.tsx` before revisiting chunk-remediation work.
3. Revisit deeper chunk splitting only after the page becomes a clearer composition container.

## Progress Refresh / 2026-04-21 / Iteration 114

Frontend progress update:
- Added a dedicated snapshot workflow helper so `Chapters.tsx` no longer directly owns the full save/apply/delete parameter assembly for story-creation snapshots.
- The page now focuses more on state wiring, while snapshot workflow orchestration is delegated to `chapterStoryCreationSnapshotWorkflowHelpers.ts`.
- This is the third concrete Phase B step: the remaining `Chapters.tsx` complexity is concentrating around restore/polling and auto-sync/persistence coordination.
- `lint` and `build` both passed after the snapshot workflow helper extraction.
- The existing `Chapters-*.js` bundle warning remains, so chunk remediation should still wait until more workflow/state glue has been extracted.

Refreshed frontend recommendation:
1. Continue Phase B with batch restore/polling coordination extraction or story-creation auto-sync/persistence coordination extraction.
2. Keep shrinking imperative workflow glue in `Chapters.tsx` before revisiting bundle-optimization work.
3. Revisit deeper chunk splitting only after the page becomes a clearer composition container.
## Progress Refresh / 2026-04-21 / Iteration 115

Frontend progress update:
- Added a dedicated batch restore/polling coordination helper so `Chapters.tsx` no longer directly owns the restore bootstrap and polling-interval wiring for batch generation.
- The page now delegates `checkAndRestoreBatchTask` and `startBatchPolling` to `chapterBatchGenerationCoordinationHelpers.ts`, while the narrower restore and polling primitives remain in their focused helper modules.
- This is the fourth concrete Phase B step: batch-generation workflow boundaries are clearer, and the remaining `Chapters.tsx` complexity is concentrating around story-creation auto-sync/persistence coordination and analysis-task restore/polling coordination.
- During validation, removed 27 irregular `U+3000` whitespace characters from `Chapters.tsx`; `lint` and `build` both passed after the cleanup.
- The existing `Chapters-*.js` bundle warning remains, so chunk remediation should still wait until more workflow/state glue has been extracted.

Refreshed frontend recommendation:
1. Continue Phase B with story-creation auto-sync/persistence coordination extraction.
2. After that, consider analysis-task restore/polling coordination or another workflow-owned hook/helper pass.
3. Revisit deeper chunk splitting only after `Chapters.tsx` becomes a clearer composition container.
## Progress Refresh / 2026-04-21 / Iteration 116

Frontend progress update:
- Added `chapterStoryCreationPersistenceCoordinationHelpers.ts` so `Chapters.tsx` no longer directly owns the full restore plus snapshot-load plus draft-persist orchestration for story-creation state.
- `chapterStoryCreationAutoSyncHelpers.ts` now exposes `syncStoryCreationAutoDrafts`, allowing single and batch story-creation auto-sync to cross the page boundary as two scope-level coordination calls instead of six granular effects.
- `Chapters.tsx` now delegates single and batch story-creation restore/persistence and auto-sync coordination through helper contracts, which makes the remaining page complexity concentrate more clearly around analysis-task coordination and other chapter-domain workflows.
- `lint` and `build` both passed after the extraction; during validation, an additional 32 irregular `U+3000` whitespace characters were removed from `Chapters.tsx`.
- The existing `Chapters-*.js` bundle warning remains, and the current build snapshot shows a noticeably larger `Chapters` chunk than the previous validation baseline, so chunk-remediation work should stay deferred until that delta is re-checked deliberately.

Refreshed frontend recommendation:
1. Continue Phase B with analysis-task restore/polling coordination extraction or another workflow-owned helper pass around task orchestration.
2. Before any chunk-tuning attempt, compare bundle snapshots across the last two Phase B iterations to confirm whether the current size jump is structural or incidental.
3. Keep using workflow-owned helpers/hooks instead of adding new presentation-only wrappers.
## Progress Refresh / 2026-04-21 / Iteration 117

Frontend progress update:
- Added `chapterAnalysisTaskCoordinationHelpers.ts` so `Chapters.tsx` no longer directly owns the full analysis-task load, start-polling, refresh, and modal-close retry coordination path.
- `Chapters.tsx` now delegates `loadAnalysisTasks`, `startPollingTask`, `refreshChapterAnalysisTask`, and `handleCloseAnalysis` through workflow-oriented helper contracts, which pushes another slice of analysis-task orchestration out of the page container.
- This continues Phase B by making analysis-task behavior more explicitly layered: polling primitives stay in focused helper modules, while page-level coordination now crosses a dedicated analysis coordination boundary.
- `lint` and `build` both passed after the extraction; during validation, another 73 irregular `U+3000` whitespace characters were removed from `Chapters.tsx`.
- The `Chapters-*.js` bundle warning remains, and the current build snapshot shows the `Chapters` chunk growing to roughly 2.8 MB before gzip reporting, so this should be treated as an active follow-up signal before continuing any performance-sensitive restructuring.

Refreshed frontend recommendation:
1. Before the next Phase B slice, inspect why the latest `Chapters` build artifact grew sharply relative to the recent baseline.
2. If the bundle delta is incidental, continue with the next workflow-owned extraction around project-switch initialization or another high-density chapter-domain seam.
3. Keep chunk-remediation work targeted and data-driven; do not jump to coarse `manualChunks` rules.

## Progress Refresh / 2026-04-21 / Iteration 118

Frontend progress update:
- Investigated the sudden `Chapters-*.js` size spike and confirmed it was not a normal structural side effect of the recent helper extractions.
- `frontend/src/pages/Chapters.tsx` contained six injected ultra-long garbled lines: two comment lines and four visible UI text lines in the page header action area.
- Replaced the corrupted comment/text payloads with normal short strings, which reduced `Chapters.tsx` from roughly 5.0 MB to about 92 KB.
- `lint` and `build` both passed after the cleanup, and the `Chapters` production chunk dropped from roughly 2.8 MB back to about 89 KB before gzip.
- This closes the active bundle-anomaly follow-up from Iteration 117 and confirms the recent Phase B helper extraction work itself was not the root cause of the chunk explosion.

Refreshed frontend recommendation:
1. Resume Phase B with the next workflow-owned extraction around project-switch initialization or another high-density page-orchestration seam in `Chapters.tsx`.
2. Keep bundle monitoring in place after each extraction, but treat abnormal chunk spikes as corruption/regression signals rather than proof that helper extraction is harmful.
3. Continue avoiding broad chunk-splitting changes until the remaining page orchestration has been reduced further.

## Progress Refresh / 2026-04-21 / Iteration 119

Frontend progress update:
- Added `chapterProjectCoordinationHelpers.ts` to centralize chapter-page project lifecycle orchestration that was still embedded in `Chapters.tsx`.
- `Chapters.tsx` now delegates the current-project switch initialization effect through `initializeChapterProjectWorkflow`, which makes the page-level bootstrap path around analysis cache restore, polling stop, chapter refresh bootstrap, writing-style load, analysis-task load, and batch-task restore more explicit.
- The page now also delegates current-project reload and delete-after-refresh coordination through `reloadChapterProjectWorkflow` and `deleteChapterWithRefreshWorkflow`, reducing another slice of imperative project refresh glue in the container.
- `lint` and `build` both passed after the extraction, and the `Chapters` production chunk remains in the normal range at roughly 89.7 KB before gzip, confirming the bundle is still healthy after Iteration 118 cleanup.
- This continues Phase B by moving another project-domain orchestration seam out of `Chapters.tsx` without changing user-visible behavior.

Refreshed frontend recommendation:
1. Continue Phase B with another workflow-owned extraction around chapter action dialog coordination, export/manual-create orchestration, or another remaining imperative seam in `Chapters.tsx`.
2. Keep validating bundle size after each iteration, but treat the current `Chapters` chunk size as back to baseline.
3. Preserve the current approach: extract orchestration/helpers first, and delay any broad chunk strategy changes until the page container is smaller and cleaner.

## Progress Refresh / 2026-04-21 / Iteration 120

Frontend progress update:
- Added `chapterActionDialogCoordinationHelpers.ts` so `Chapters.tsx` no longer directly owns the export-confirm modal flow, manual-create dialog bootstrap, or expansion-plan preview dialog bootstrap.
- The page now delegates `handleExport`, `showManualCreateChapterModal`, and `showExpansionPlanModal` through action/dialog coordination helpers, while the lower-level dialog implementations remain in `utils/chapterActionDialogs.tsx`.
- This removes the last direct `chapterApi` / `projectApi` service import usage from `Chapters.tsx`, which makes the page container more clearly focused on state wiring and chapter workflow composition.
- `lint` and `build` both passed after the extraction, and the `Chapters` production chunk remains healthy at roughly 91.1 KB before gzip.
- This continues Phase B by extracting another imperative UI-orchestration seam without changing visible behavior.

Refreshed frontend recommendation:
1. Continue Phase B with another workflow-owned extraction around chapter analysis batch-trigger orchestration or another remaining imperative seam in `Chapters.tsx`.
2. Treat `utils/chapterActionDialogs.tsx` as a later cleanup target, because it still contains mixed presentation/orchestration responsibility and visible encoding debt.
3. Keep bundle validation after each iteration; the current `Chapters` artifact remains near the healthy post-cleanup baseline.

## Progress Refresh / 2026-04-21 / Iteration 121

Frontend progress update:
- Cleaned the visible encoding debt in `utils/chapterActionDialogs.tsx` for the dialog flows touched by the new action coordination layer, replacing garbled fallback labels, button labels, titles, and error/success copy with readable strings.
- This keeps the newly extracted `chapterActionDialogCoordinationHelpers.ts` handoff readable and removes one immediate readability hazard from a shared dialog utility.
- `lint` and `build` both passed after the cleanup, and the `Chapters` production chunk remains healthy at roughly 91.1 KB before gzip.
- The remaining `chapterActionDialogs.tsx` structure still uses broad `any` typing and mixed modal/UI orchestration, so it stays a candidate for a later typed cleanup pass.

Refreshed frontend recommendation:
1. Continue with a later focused cleanup of `utils/chapterActionDialogs.tsx` typing and responsibility boundaries after the main `Chapters.tsx` Phase B seams are reduced further.
2. Keep treating encoding regressions as build-health issues, not cosmetic-only issues.
3. Preserve the current pattern: extract orchestration first, then tighten helper typing and shared dialog utility quality.

## Progress Refresh / 2026-04-21 / Iteration 122

Frontend progress update:
- Reworked `utils/chapterActionDialogs.tsx` from broad `any`-driven dialog wiring into a typed utility with explicit modal, message, chapter, writing-style, and manual-create form contracts.
- Fixed a latent bug in `openContinueGenerateDialog`: the dialog now reads `CREATIVE_MODE_OPTIONS` and `STORY_FOCUS_OPTIONS` directly from shared preference-option utilities instead of expecting missing caller-provided arrays.
- Cleaned the visible copy in the dialog-related content components touched by this flow: `ManualChapterCreateFormContent.tsx`, `ContinueGenerateConfirmContent.tsx`, `ChapterNumberConflictConfirmContent.tsx`, and `ChapterExpansionPlanPreviewContent.tsx`.
- `lint` and `build` both passed after the typed cleanup, and the `Chapters` production chunk remains healthy at roughly 91.1 KB before gzip.
- This continues Phase B by improving both maintainability and runtime safety around the shared chapter dialog layer without changing orchestration boundaries again.

Refreshed frontend recommendation:
1. Continue with a targeted encoding-debt cleanup pass in other chapter-domain components and messages that still show broken copy in `Chapters.tsx` or adjacent modal content.
2. After that, consider a deeper typed cleanup for the remaining shared dialog utilities and modal config helpers.
3. Keep bundle verification after each iteration; current output stays close to the healthy post-cleanup baseline.

## Progress Refresh / 2026-04-21 / Iteration 123

Frontend progress update:
- Recovered `frontend/src/components/ChapterBatchGenerateModal.tsx` from the healthy `HEAD` baseline after confirming the current working copy had large-scale encoding corruption rather than an intentional refactor.
- Preserved the current lazy-entry contract by re-applying the exported `ChapterBatchGenerateModalProps` type needed by `ChapterBatchGenerateModalEntry.tsx` and `Chapters.tsx`.
- This restored the damaged batch-generation modal strings, quality cards, web-research controls, and cancel-confirm copy in one pass instead of trying to patch dozens of broken literals individually.
- `lint` and `build` both passed after the recovery, and the `Chapters` production chunk remains healthy at roughly 91.3 KB before gzip.
- This closes the active frontend blocker from the failed chapter-domain encoding cleanup attempt and returns the frontend to a stable, shippable baseline for the next finishing slice.

Refreshed frontend recommendation:
1. Resume frontend finishing with a targeted scan for remaining encoding debt in adjacent chapter-domain components or messages, but treat whole-file corruption as a restore-from-baseline event first.
2. Keep `ChapterBatchGenerateModal.tsx` stable for now; any future cleanup in this component should be small, typed, and validated immediately because it is a high-coupling workflow modal.
3. Continue Phase B extraction and cleanup from healthier seams around `Chapters.tsx` and shared chapter helpers, while preserving the current chunk baseline.
## Progress Refresh / 2026-04-21 / Iteration 124

Frontend progress update:
- Extended `frontend/src/pages/chapterSingleGenerationHelpers.ts` with `startSingleChapterGenerationWorkflow` so `Chapters.tsx` no longer owns the full single-chapter generation orchestration path around duplicate-task guarding, prompt preparation, progress state setup, generation execution, result tracking, and completion/error feedback.
- Cleaned the remaining broken user-facing copy in `trackSingleChapterGenerationResult`, replacing unreadable placeholder text with readable English status, candidate-draft, local-edit, and failure messages.
- Simplified `frontend/src/pages/Chapters.tsx` by routing `handleGenerate` through the new workflow helper, which keeps the page container more focused on state wiring and modal entry composition.
- `lint` and `build` both passed after the extraction and cleanup; the `Chapters` production chunk remains healthy at roughly 93.1 KB before gzip.
- This continues Phase B with another workflow-owned extraction while also removing one more chapter-domain readability hazard from a shared generation helper.

Refreshed frontend recommendation:
1. Continue front-end finishing by targeting the next real workflow seam in `Chapters.tsx`, not just moving passive prop objects.
2. Keep folding broken user-facing copy into adjacent helper cleanups whenever it appears, especially in generation and polling utilities.
3. Watch the `Chapters` chunk after each iteration; the current size remains acceptable, but use the low-90 KB range as the near-term baseline.
## Progress Refresh / 2026-04-21 / Iteration 125

Frontend progress update:
- Added `openBatchGenerationWorkflow` to `frontend/src/pages/chapterBatchGenerationWorkflowHelpers.ts` so `Chapters.tsx` no longer directly owns the batch-generate modal bootstrap path around active-run guards, first-incomplete-chapter checks, generation-eligibility validation, default-model loading, cockpit reset, plot-stage inference, and initial form seeding.
- Simplified `frontend/src/pages/Chapters.tsx` by routing `handleOpenBatchGenerate` through the new workflow helper, which removes another page-level imperative sequence from the container.
- Kept the new helper aligned with existing chapter workflow patterns by extending the current batch-generation helper module instead of introducing a parallel file for a small seam.
- `lint` and `build` both passed after the extraction; the `Chapters` production chunk remains healthy at roughly 93.8 KB before gzip.
- This continues Phase B with another workflow-owned batch-generation seam while preserving the currently stable frontend baseline.

Refreshed frontend recommendation:
1. Continue extracting real workflow seams from `Chapters.tsx`, especially around modal/bootstrap or task-restore paths, before touching low-value prop assembly blocks.
2. Keep using existing chapter helper modules as the first extension point when the behavior belongs to an already-established domain.
3. Monitor the `Chapters` chunk after each iteration; the current low-90 KB range remains acceptable for this finishing phase.
## Progress Refresh / 2026-04-21 / Iteration 126

Frontend progress update:
- Cleaned the remaining broken helper copy in chapter-domain workflow utilities that still surfaced `????` placeholder text after the recent Phase B extractions.
- `frontend/src/pages/chapterBatchGenerationRestoreHelpers.ts` now reports active batch-task restoration with readable status copy instead of unreadable placeholders.
- `frontend/src/pages/chapterDeferredBatchAnalysisHelpers.ts` now uses readable queued/skipped/failed deferred-analysis messages, which makes the deferred batch-analysis handoff easier to understand during long-running chapter workflows.
- `frontend/src/pages/chapterWritingStyleLoadHelpers.ts` now emits a readable writing-style load failure message, removing another visible readability hazard from the chapter page bootstrap path.
- `lint` and `build` both passed after the cleanup; the `Chapters` production chunk remains healthy at roughly 93.9 KB before gzip.

Refreshed frontend recommendation:
1. Continue the front-end finishing pass by clearing remaining `????` placeholder text in adjacent store/workflow files, prioritizing user-facing runtime messages over comments.
2. Keep chapter-domain helper cleanup focused and incremental; these small readability fixes are low-risk but compound quickly across async task flows.
3. Preserve the current validation discipline after each cleanup slice, using the low-90 KB `Chapters` chunk as the working baseline.
## Progress Refresh / 2026-04-21 / Iteration 127

Frontend progress update:
- Cleaned the remaining user-visible placeholder text in `frontend/src/store/backgroundTaskSelectors.ts`, replacing broken background-task category labels and section titles/descriptions with readable English copy.
- Cleaned the runtime error labels/messages in `frontend/src/store/hooks.ts` for character, outline, chapter, and project load/create/update/delete flows, plus AI generation log labels.
- This removes another batch of visible readability debt from global task-center and store-driven CRUD feedback paths without changing behavior or data flow.
- `lint` and `build` both passed after the cleanup; the `Chapters` production chunk remains healthy at roughly 93.9 KB before gzip.
- The remaining `????` markers in `frontend/src/store/hooks.ts` are currently comment-only and do not affect runtime behavior.

Refreshed frontend recommendation:
1. Continue the finishing pass by cleaning the next user-facing placeholder cluster in `frontend/src/store/chapterGenerationWorkflow.ts`, which now looks like the highest-value remaining runtime readability target.
2. Keep comments and non-runtime text lower priority than strings that surface in notifications, task-center sections, progress messages, or thrown errors.
3. Preserve the current validation cadence after each cleanup slice; the low-90 KB `Chapters` chunk remains the working baseline.

## Progress Refresh / 2026-04-21 / Iteration 128

Frontend progress update:
- Cleaned the remaining runtime placeholder text in `frontend/src/store/chapterGenerationWorkflow.ts`, replacing unreadable progress, SSE, polling, cancellation, timeout, and completion strings with readable English copy.
- Kept the workflow behavior unchanged while improving the quality-gate and candidate-draft messaging shown during single-chapter generation, which removes one of the highest-value remaining readability hazards in the chapter generation path.
- Normalized the running-state retry suffix so polling progress now reads more clearly when retries occur during quality review or content generation.
- `lint` and `build` both passed after the cleanup; the `Chapters` production chunk remains healthy at roughly 93.9 KB before gzip.
- This completes the targeted store-side cleanup that Iteration 127 identified as the next highest-value runtime readability hotspot.

Refreshed frontend recommendation:
1. Continue the finishing pass by scanning for the next remaining user-visible `????` cluster in frontend store/workflow files, especially chapter-related async status or notification paths.
2. Keep prioritizing runtime copy over comment-only placeholders, then return to the next real `Chapters.tsx` workflow seam once the visible encoding debt is mostly gone.
3. Preserve the current validation cadence after each cleanup slice; the low-90 KB `Chapters` chunk remains the working baseline.

## Progress Refresh / 2026-04-21 / Iteration 129

Frontend progress update:
- Cleaned the last remaining runtime placeholder copy found by the front-end scan in `frontend/src/store/projectSyncHelpers.ts`.
- Replaced the broken project-refresh console label and toast message with readable English error text while keeping the existing refresh and loading behavior unchanged.
- `lint` and `build` both passed after the cleanup; the `Chapters` production chunk remains healthy at roughly 93.9 KB before gzip.
- A follow-up scan shows that the remaining `????` markers under `frontend/src` are currently comment-only entries in `frontend/src/store/hooks.ts`, not runtime-facing copy.
- This means the visible front-end encoding debt is now substantially reduced, and the next finishing slice can move back toward workflow extraction or non-comment cleanup with much lower user-facing risk.

Refreshed frontend recommendation:
1. Treat `frontend/src/store/hooks.ts` comment-only placeholders as documentation debt, not a runtime blocker.
2. Return to the next real `Chapters.tsx` workflow seam or related helper extraction now that the visible store/workflow copy hotspots are mostly cleared.
3. Keep the current validation cadence after each finishing slice; the low-90 KB `Chapters` chunk remains the working baseline.

## Progress Refresh / 2026-04-21 / Iteration 130

Frontend progress update:
- Added `openSingleChapterGenerateWorkflow` to `frontend/src/pages/chapterActionDialogCoordinationHelpers.ts`, so the lazy dialog bootstrap for single-chapter generation is now coordinated beside the existing export, manual-create, and expansion-plan dialog workflows.
- Simplified `frontend/src/pages/Chapters.tsx` by routing `showGenerateModal` through the new helper instead of importing and wiring `openContinueGenerateDialog` directly in the page container.
- Kept the refactor intentionally narrow: behavior and dialog content remain unchanged, while `Chapters.tsx` loses another page-owned UI workflow seam.
- `lint` and `build` both passed after the extraction; the `Chapters` production chunk remains acceptable at roughly 94.9 KB before gzip.
- With the runtime placeholder cleanup largely complete, this iteration resumes the Phase B objective of shrinking `Chapters.tsx` orchestration surface through focused helper extraction.

Refreshed frontend recommendation:
1. Continue extracting small but real page-owned interaction seams from `frontend/src/pages/Chapters.tsx`, especially dialog-open or workflow-bootstrap logic that already matches an existing helper domain.
2. Keep an eye on the `Chapters` chunk after each extraction; the current ~94.9 KB baseline is still acceptable, but avoid bundling heavier logic back into the page container.
3. Treat the remaining `frontend/src/store/hooks.ts` comment placeholders as low-priority documentation debt unless they start blocking readability during adjacent changes.
## Progress Refresh / 2026-04-21 / Iteration 131

Frontend progress update:
- Cleaned the remaining runtime toast corruption in rontend/src/pages/chapterEditorLifecycleHelpers.ts and rontend/src/pages/chapterModalSubmitHelpers.ts.
- Replaced the unreadable chapter-update success/failure feedback in both the full editor submit flow and the lightweight modal submit flow with readable English copy, keeping the underlying save behavior unchanged.
- lint and uild both passed after the cleanup; the Chapters production chunk remains acceptable at roughly 94.9 KB before gzip.
- This removes another visible editing-path readability hazard that surfaced while scanning the next Chapters.tsx extraction seam.

Refreshed frontend recommendation:
1. Continue scanning chapter-domain helper files for any remaining runtime-facing mojibake before returning to the next Chapters.tsx workflow extraction.
2. Keep comment-only mojibake lower priority than toast, notification, modal, and thrown-error strings that surface during chapter editing or generation flows.
3. Preserve the current validation cadence after each cleanup slice; the Chapters chunk baseline remains healthy enough for the finishing phase.
## Progress Refresh / 2026-04-21 / Iteration 132

Frontend progress update:
- Added submitChapterEditorWorkflow to rontend/src/pages/chapterEditorLifecycleHelpers.ts, moving the editor-submit guard and close-editor wiring into the existing editor lifecycle helper domain.
- Simplified rontend/src/pages/Chapters.tsx by routing handleEditorSubmit through the new workflow helper instead of composing submitChapterEditorUpdate and closeChapterEditor inline inside the page.
- lint and uild both passed after the extraction; the Chapters production chunk moved to roughly 96.2 KB before gzip, which is still acceptable but should be watched in later iterations.
- This resumes the Phase B goal of shrinking page-owned orchestration in Chapters.tsx after the recent runtime-copy cleanup slices.

Refreshed frontend recommendation:
1. Continue extracting low-risk page-owned editor/modal open flows into existing chapter helper domains, especially where Chapters.tsx still does entity lookup plus helper invocation inline.
2. Watch the Chapters chunk after each extraction now that the baseline has moved into the mid-90 KB range before gzip.
3. Keep runtime-facing string cleanup opportunistic whenever new helper seams surface adjacent user-visible copy.
## Progress Refresh / 2026-04-21 / Iteration 133

Frontend progress update:
- Added openChapterModalWorkflow and openChapterEditorWorkflow to the existing chapter modal/editor open helper modules so the chapter lookup + helper invocation pair is no longer owned inline by Chapters.tsx.
- Simplified rontend/src/pages/Chapters.tsx by routing handleOpenModal and handleOpenEditor through those workflow helpers, which continues reducing page-owned orchestration in the chapter editing path.
- lint and uild both passed after the extraction; the Chapters production chunk rose to roughly 99.0 KB before gzip, so subsequent extractions should stay focused and avoid bundling additional heavy logic into the page path.
- This keeps Phase B moving, but the chunk trend now needs explicit attention alongside maintainability wins.

Refreshed frontend recommendation:
1. Prefer the next extractions from already-loaded helper domains that reduce page orchestration without adding new heavy imports or duplicated utility code.
2. Watch the Chapters chunk after each iteration now that it has approached the 100 KB mark before gzip.
3. Continue opportunistic runtime-copy cleanup whenever chapter-domain helper scans expose user-visible mojibake or placeholder strings.
## Progress Refresh / 2026-04-21 / Iteration 134

Frontend progress update:
- Cleaned the remaining runtime mojibake in rontend/src/pages/chapterReaderLifecycleHelpers.ts, replacing the unreadable reader load failure toast with readable English copy.
- lint and uild both passed after the cleanup; the Chapters production chunk remains roughly 99.0 KB before gzip.
- This removes another visible chapter-domain readability hazard without adding new orchestration weight to Chapters.tsx.

Refreshed frontend recommendation:
1. Continue preferring no-risk runtime copy cleanup and very small workflow extractions while the Chapters chunk sits near the 100 KB mark.
2. Keep the next extraction focused on guard + helper wiring rather than adding new utility branches to the page path.
3. Preserve the current validation cadence after each slice; the chunk trend now matters as much as maintainability progress.
## Progress Refresh / 2026-04-21 / Iteration 135

Frontend progress update:
- Added submitChapterModalWorkflow to rontend/src/pages/chapterModalSubmitHelpers.ts, moving the modal-submit guard into the existing modal submit helper domain.
- Simplified rontend/src/pages/Chapters.tsx by routing handleSubmit through the workflow helper instead of guarding and calling submitChapterModalUpdate inline.
- lint and uild both passed after the extraction, but the Chapters production chunk increased to roughly 104.2 KB before gzip.
- This means the maintainability gain is real, but additional front-end extractions inside the same loaded path should now be treated more cautiously than before.

Refreshed frontend recommendation:
1. Pause further helper extractions inside the hot Chapters path unless the next slice removes more code weight than it adds.
2. Prefer front-end cleanup that does not materially increase the loaded Chapters path, or shift the next refactor slice to backend/shared logic while the front-end chunk is above the prior baseline.
3. Keep runtime-facing string cleanup opportunistic, but weigh every new helper wrapper against the current chunk growth trend.

## Progress Refresh / 2026-04-21 / Iteration 136

Backend progress update:
- Switched the batch-generation execution entry used by `backend/app/api/chapter_batch_generation_routes.py` and `backend/app/api/chapter_generation_routes.py` from the legacy `chapters_api.execute_batch_generation_in_order` route dependency to `batch_generation_entry_compat_service.execute_batch_generation_in_order`.
- Updated the related API test seams in `backend/tests/test_api/chapters_test_support.py` and `backend/tests/test_api/test_chapters_batch_status_resume.py` so both the legacy gateway and the compat seam are monkeypatched consistently during route-level tests.
- While validating this slice, fixed pre-existing syntax corruption in the same target files by replacing unreadable broken string literals with stable English copy, restoring clean import and test collection for the touched backend route and test modules.
- `py_compile` passed for all four touched files, and the targeted regression suite passed: `backend/tests/test_services/test_batch_generation_entry_compat_service.py`, `backend/tests/test_services/test_chapter_generation_background_entry_service.py`, and `backend/tests/test_api/test_chapters_batch_status_resume.py`.
- This iteration completes a low-risk backend seam consolidation step: route handlers now depend on the existing compat entry instead of reaching the legacy batch execution gateway directly, which improves maintainability without changing the orchestration contract.

Refreshed backend recommendation:
1. Continue refactoring from the route layer inward by moving remaining legacy `chapters_api` orchestration wiring behind existing compat or entry services before introducing any new backend abstractions.
2. Prefer the next backend slice in shared orchestration, status, or route wiring modules rather than returning immediately to the `frontend/src/pages/Chapters.tsx` hot path while its production chunk remains above the earlier baseline.
3. Keep `py_compile` plus targeted pytest coverage in the loop for backend slices, because this round showed that historical string corruption inside touched files can block route-level validation before logic regressions appear.

## Progress Refresh / 2026-04-21 / Iteration 137

Backend progress update:
- Decoupled `backend/app/api/chapter_analysis_task_routes.py` from the broad `chapters_api` module for manual analysis task execution. The route now imports `asyncio` directly for its short delay and schedules `execute_chapter_analysis_background` from `manual_chapter_analysis_execution_service` instead of routing through `chapters_api`.
- Updated API test seams in `backend/tests/test_api/chapters_test_support.py` and `backend/tests/test_api/test_chapters_analysis.py` so manual analysis background execution is monkeypatched against the route module and the execution service consistently.
- Kept the slice narrow: request/response behavior stays the same, but route-level ownership is cleaner and the manual analysis execution path now depends on a purpose-built service boundary.
- Validation passed for this slice: `py_compile` succeeded for the touched route and tests, and `backend/tests/test_api/test_chapters_analysis.py` passed end to end.

Refreshed backend recommendation:
1. Continue removing narrow route-level dependencies on `chapters_api` where a focused service already exists, especially for background task execution, query assembly, or status shaping.
2. Treat route-module imported aliases as part of the test seam when refactoring; patching only the source service module may not affect already-bound route symbols.
3. Keep backend refactor slices small and verifiable, pairing each seam cleanup with direct route-level pytest coverage before moving on.


## Progress Refresh / 2026-04-21 / Iteration 138

Backend progress update:
- Decoupled `backend/app/api/chapter_quality_routes.py` from the broad `chapters_api` gateway. The route now relies on query-service defaults for loading quality metric records and uses `project_quality_trend_compat_service.get_project_quality_trend_snapshot_with_default_wiring` as its snapshot seam.
- Expanded `backend/app/services/project_quality_trend_compat_service.py` with a default-wiring entry that binds summary-state builders plus snapshot persistence/loading in one focused compatibility layer, so route code no longer needs to borrow these defaults from `chapters_api`.
- Updated `backend/tests/test_api/test_chapters_quality_views.py` to patch the new compat/service seam directly instead of patching `chapters_api`, preserving cache-behavior and persisted-snapshot verification after the route dependency cleanup.
- Validation passed for this slice: `py_compile` succeeded for the touched route, compat service, and API test module, and `backend/tests/test_api/test_chapters_quality_views.py` passed end to end.
- This keeps the backend refactor moving in the same direction as Iteration 136-137: route modules progressively depend on dedicated services or compat seams instead of the monolithic `chapters_api` module.

Refreshed backend recommendation:
1. Continue scanning `backend/app/api` for routes that still import `chapters_api` only to obtain default dependency bundles, then move those defaults into small compat services near the target domain.
2. Prefer seams that preserve existing pytest monkeypatch entry points, because several route-level tests depend on patching already-bound module symbols rather than deep source functions.
3. Keep backend route cleanup ahead of any new front-end extraction while the `Chapters` page chunk still sits above its earlier baseline.


## Progress Refresh / 2026-04-21 / Iteration 139

Backend progress update:
- Added `backend/app/services/batch_generation_route_compat_service.py` to own the default create/resume wiring for batch generation routes, including prerequisite checks, quality profile resolution, story packet assembly, story-repair state sync, and execution entry selection.
- Simplified `backend/app/api/chapter_batch_generation_routes.py` so the route no longer imports the broad `chapters_api` module just to assemble batch-generation orchestration defaults. The route now delegates create/resume flows to the new compat service.
- Kept the execution seam unchanged for tests and runtime behavior: the new compat layer still calls `batch_generation_entry_compat_service.execute_batch_generation_in_order`, so the existing API regression tests continue to patch and observe the same low-level execution entry.
- Validation passed for this slice: `py_compile` succeeded for the new compat service and route module, and `backend/tests/test_api/test_chapters_batch_status_resume.py` plus `backend/tests/test_services/test_batch_generation_entry_compat_service.py` passed end to end.
- This leaves `backend/app/api/chapter_generation_routes.py` as the main remaining route that still imports `chapters_api` for a large default dependency bundle, making it the next obvious backend seam candidate.

Refreshed backend recommendation:
1. Tackle `chapter_generation_routes.py` next by introducing a focused stream-entry compat seam, because it now represents the largest remaining route-level dependency bundle on `chapters_api`.
2. Preserve low-level execution entry seams when moving route defaults into compat services; stable monkeypatch points keep existing API regression coverage useful during refactors.
3. Continue pairing each route cleanup with targeted route-level pytest runs before widening the scope.

## Progress Refresh / 2026-04-21 / Iteration 140

Backend progress update:
- Added `backend/app/services/chapter_generation_route_compat_service.py` as the focused stream-route compat seam for `backend/app/api/chapter_generation_routes.py`, so the `generate-stream` route no longer imports the broad `chapters_api` module just to assemble default wiring.
- Completed the stream prompt/runtime seam handoff by letting the compat layer own `get_template`, `format_prompt`, `apply_style_to_prompt`, and candidate-record logging defaults, while `backend/app/services/chapter_generation_stream_wiring_service.py` and `backend/app/services/chapter_generation_stream_entry_service.py` now accept these dependencies as injectable callables with backward-compatible defaults.
- Updated route-level regression seams in `backend/tests/test_api/test_chapters_stream_routes.py` and `backend/tests/test_api/test_chapters_batch_generation.py` so tests monkeypatch the new compat prompt entry points directly instead of relying on the old `chapters_api` bundle.
- Validation passed for this slice: `backend/tests/test_api/test_chapters_stream_routes.py -k generate_stream`, `backend/tests/test_api/test_chapters_batch_generation.py -k schedule_followup_analysis_when_generate_stream_hits_quality_gate`, and `backend/tests/test_services/test_chapter_generation_stream_entry_service.py` all passed.
- This closes the main remaining `generate-stream` route dependency bundle on `chapters_api` and keeps the route/service boundary aligned with the earlier backend compat refactor slices.

Refreshed backend recommendation:
1. Continue scanning `backend/app/api/chapter_generation_routes.py` for any remaining low-level helper borrowing from legacy route bundles and move them behind focused compat services only when they improve test seams or route readability.
2. Prefer the next backend slice in regeneration, draft, or shared stream orchestration helpers rather than reopening the front-end hot path, because the backend route boundary cleanup is still yielding low-risk maintainability wins.
3. Preserve backward-compatible defaults whenever a shared entry service gains new injectable seams; route refactors should not make service-level tests or alternate callers pass more parameters unless the contract truly changes.

## Progress Refresh / 2026-04-21 / Iteration 141

Backend progress update:
- Extended `backend/app/services/chapter_generation_route_compat_service.py` so the single-chapter `generate-background` route now uses the same focused route compat layer as `generate-stream`, instead of assembling its default wiring inline inside `backend/app/api/chapter_generation_routes.py`.
- The new background route compat entry binds `load_accessible_chapter_or_404`, prerequisite checks, workflow snapshot building, story-repair state resolution, `sync_task_story_repair_state`, and the batch execution entry in one place. This also closes a route-level wiring gap where `sync_task_story_repair_state_fn` was no longer being forwarded to the background entry service.
- Simplified `backend/app/api/chapter_generation_routes.py` further so both generation endpoints now delegate to the compat layer and the route module stays focused on request/response glue.
- Added modular API regression coverage in `backend/tests/test_api/test_chapters_batch_generation.py` for `/api/chapters/{id}/generate-background`, ensuring the route compat seam creates the expected single-chapter background task under the split router setup instead of relying only on the legacy gateway test suite.
- Validation passed for this slice: `backend/tests/test_api/test_chapters_stream_routes.py -k generate_stream`, `backend/tests/test_api/test_chapters_batch_generation.py -k 'single_chapter_background_generation_task_via_generation_route_compat or schedule_followup_analysis_when_generate_stream_hits_quality_gate'`, and `backend/tests/test_services/test_chapter_generation_background_entry_service.py` all passed.

Refreshed backend recommendation:
1. Keep working inside the split route modules and prioritize seams that remove inline default dependency assembly from route files while preserving stable monkeypatch points for tests.
2. The next low-risk backend slice should target adjacent generation-domain routes or shared orchestration helpers, especially where the split routes still depend on legacy gateway-owned helper bundles indirectly.
3. Maintain the current validation pattern of route-level pytest plus entry-service pytest whenever a compat layer starts owning more default wiring; this catches contract drift before it reaches the monolithic gateway tests.

## Progress Refresh / 2026-04-21 / Iteration 142

Backend progress update:
- Added `backend/app/services/chapter_regeneration_route_compat_service.py` to own the default stream-regeneration route wiring for `backend/app/api/chapter_regeneration_routes.py`, so the split `regenerate-stream` route no longer assembles its context-building, SSE stream construction, sanitizer, and regenerator defaults inline.
- Simplified `backend/app/api/chapter_regeneration_routes.py` so the stream regeneration endpoint now delegates to the compat layer, leaving the route module focused on request/response glue while keeping the regeneration task history query route unchanged.
- Updated test seams in `backend/tests/test_api/chapters_test_support.py`, `backend/tests/test_api/test_chapters_stream_routes.py`, and `backend/tests/test_api/test_chapters.py` so `get_db` overrides and `REGENERATOR_FACTORY` monkeypatches point at the new compat service instead of the route module. This preserves both split-route regression coverage and the older gateway-style regression coverage.
- Validation passed for this slice: `backend/tests/test_api/test_chapters_stream_routes.py -k regenerate_chapter_stream`, `backend/tests/test_api/test_chapters.py -k regeneration_tasks`, and `backend/tests/test_api/test_chapters.py -k apply_project_story_packet_defaults_in_regeneration_prompt_context` all passed.
- This keeps the backend refactor moving consistently across the generation domain: stream generation, background generation, and stream regeneration routes now all delegate their default dependency bundles to focused compat services instead of embedding orchestration wiring inside the route files.

Refreshed backend recommendation:
1. Continue targeting adjacent generation-domain route files where stream or background endpoints still assemble default helper bundles inline, especially if existing tests monkeypatch route-level factories or `get_db` aliases.
2. Prefer compat seams that preserve legacy test monkeypatch points by re-exporting route-level factories or helper callables from the compat layer; this has proven to be the lowest-risk migration pattern so far.
3. Keep pairing each route cleanup with one split-route regression and one older gateway-style regression whenever both exist, so test seam drift is caught immediately.

## Progress Refresh / 2026-04-21 / Iteration 143

Backend progress update:
- Added `backend/app/services/chapter_partial_regeneration_route_compat_service.py` to own the default stream orchestration for `backend/app/api/chapter_partial_regeneration_routes.py`, including chapter access checks, preparation loading, SSE emission flow, partial-output normalization, and generated-text sanitization.
- Simplified `backend/app/api/chapter_partial_regeneration_routes.py` so `partial-regenerate-stream` now delegates to the compat layer while `apply-partial-regenerate` remains inline. This keeps the slice low risk by moving only the stream orchestration path out of the route module.
- Kept the new compat service UTF-8-safe by using stable literals for the partial-output prefix normalization rules instead of reintroducing route-local string cleanup logic.
- Validation passed for this slice: `backend/tests/test_api/test_chapters_stream_routes.py -k 'partial_regenerate or apply_partial_regenerate'` passed end to end, covering partial stream output, web-research grounding, sanitized apply behavior, and invalid selection rejection.
- This continues the same backend cleanup pattern as the previous iterations: split generation-domain stream routes progressively delegate their default runtime wiring to focused compat services, reducing route-module orchestration weight without changing request contracts.

Refreshed backend recommendation:
1. Continue looking for stream-style split routes that still mix access control, preparation loading, SSE progress emission, and text sanitization directly inside the route module; those are the cleanest compat candidates.
2. Keep stream-route refactors narrow by moving orchestration first and leaving adjacent writeback/apply endpoints inline until the stream seam is stable.
3. Preserve UTF-8-safe string handling in newly added compat layers; prefer stable literals or explicit escapes over copying older corrupted text blocks.

## Progress Refresh / 2026-04-21 / Iteration 144

Backend progress update:
- Extended `backend/app/services/batch_generation_route_compat_service.py` with `stream_batch_generation_events_with_default_route_wiring`, so the split batch-generation stream route no longer assembles access validation plus SSE response wiring directly inside `backend/app/api/chapter_batch_generation_routes.py`.
- Simplified `backend/app/api/chapter_batch_generation_routes.py` so `/batch-generate/{batch_id}/stream` now delegates to the compat layer, aligning the batch stream endpoint with the same route/compat pattern already used for batch create/resume.
- Added modular regression coverage in `backend/tests/test_api/test_chapters_stream_routes.py` for the split batch stream route, proving the route delegates through the compat seam and still enforces the existing access-control behavior.
- Validation passed for this slice: `backend/tests/test_api/test_chapters_stream_routes.py -k 'stream_batch_generation_events_via_route_compat or reject_stream_subscription_from_other_user'` passed end to end.
- This keeps the route cleanup moving consistently across the generation domain: split routes now progressively delegate stream access checks and SSE assembly to focused compat services instead of mixing them into the route handlers.

Refreshed backend recommendation:
1. Continue targeting split routes where the remaining inline logic is mostly access validation + SSE construction + default helper binding, because those are the cleanest low-risk compat extractions.
2. Prefer extending existing route compat services before creating new ones when the route belongs to the same domain slice; this keeps the compat surface compact and easier to reason about.
3. Keep adding one small split-route delegation test when introducing a new compat entry, even if an older access-control test already exists; it makes the route boundary refactor explicit and verifiable.


## Progress Refresh / 2026-04-21 / Iteration 145

Backend progress update:
- Added `backend/app/services/chapter_analysis_route_compat_service.py` so the split analysis read route now has a focused compat seam for authenticated chapter access, latest analysis lookup, memory/history aggregation, and candidate-draft enrichment.
- Simplified `backend/app/api/chapter_analysis_routes.py` so `/api/chapters/{chapter_id}/analysis` now delegates to the compat layer and keeps the route module focused on request parsing plus response handoff.
- Added explicit split-route delegation coverage in `backend/tests/test_api/test_chapters.py`, proving the analysis route forwards `chapter_id`, `include_full_draft`, request context, and database session through the new compat seam.
- This keeps the backend cleanup moving in the same low-risk direction as the recent generation-domain slices: route modules steadily shed access control and default wiring glue into narrowly scoped compat services without changing the external API contract.

Refreshed backend recommendation:
1. Continue with neighboring low-risk read-style split routes such as `backend/app/api/chapter_annotation_routes.py` or `backend/app/api/chapter_expansion_plan_routes.py`, because they likely share the same access-and-load glue profile.
2. Keep pairing each new compat seam with one explicit route-delegation regression, even when broader integration coverage already exists, so route boundary changes stay easy to verify.
3. Leave writeback-heavy endpoints like `apply-partial-regenerate` for later slices after the remaining read/stream route seams are reduced.


## Progress Refresh / 2026-04-21 / Iteration 146

Backend progress update:
- Added `backend/app/services/chapter_annotation_route_compat_service.py` so the split annotation read route now has a focused compat seam for authenticated chapter access, latest analysis lookup, and memory aggregation before payload assembly.
- Simplified `backend/app/api/chapter_annotation_routes.py` so `/api/chapters/{chapter_id}/annotations` now delegates to the compat layer and keeps the route module focused on request-to-response glue only.
- Added explicit split-route delegation coverage in `backend/tests/test_api/test_chapters.py`, proving the annotation route forwards `chapter_id`, request context, and database session through the new compat seam while the existing annotation integration view test still covers payload behavior.
- This keeps the backend cleanup aligned with the clean-plan strategy: low-risk read-style split routes progressively move access and default query wiring out of route modules without changing the external API contract.

Refreshed backend recommendation:
1. Continue with `backend/app/api/chapter_expansion_plan_routes.py`, because it should be another small read-style route with similar access-and-load glue.
2. Keep using one explicit delegation test plus one existing behavior test for each route seam; this has stayed cheap and reliable.
3. Defer write-heavy compat candidates until the remaining read-style split routes are exhausted.


## Progress Refresh / 2026-04-21 / Iteration 147

Backend progress update:
- Added `backend/app/services/chapter_expansion_plan_route_compat_service.py` so the split expansion-plan update route now has a focused compat seam for authenticated chapter access, payload merge, fallback overwrite on invalid stored JSON, logging, and commit/refresh wiring.
- Simplified `backend/app/api/chapter_expansion_plan_routes.py` so `/api/chapters/{chapter_id}/expansion-plan` now delegates to the compat layer and keeps the route module focused on request parsing plus response handoff.
- Added explicit split-route delegation coverage in `backend/tests/test_api/test_chapters.py`, proving the route forwards `chapter_id`, parsed `ExpansionPlanUpdate`, request context, and database session through the new compat seam while the existing update behavior test still covers the persisted merge contract.
- This extends the clean-plan backend pattern from read routes into a small write-style route without widening scope: the route sheds default wiring, but the external update behavior and payload shape stay unchanged.

Refreshed backend recommendation:
1. Re-check the remaining split routes and prefer any last small route-level seams before moving into heavier writeback or multi-step orchestration endpoints.
2. Keep write-style compat slices narrow like this one: move access, merge, and commit glue, but avoid mixing in unrelated domain logic.
3. Continue validating each write-route seam with one persisted behavior test and one explicit delegation test.


## Progress Refresh / 2026-04-21 / Iteration 148

Backend progress update:
- Added `backend/app/services/chapter_analysis_task_route_compat_service.py` so the split manual-analysis trigger route now has a focused compat seam for authenticated chapter access, project validation, manual analysis preparation, delayed background scheduling, and response assembly.
- Simplified the `POST /api/chapters/{chapter_id}/analyze` handler in `backend/app/api/chapter_analysis_task_routes.py` so the route now delegates to the compat layer and stays focused on request/dependency glue.
- Moved the stable monkeypatch seam for background analysis execution from the route module to the compat service in `backend/tests/test_api/chapters_test_support.py` and `backend/tests/test_api/test_chapters_analysis.py`, then added an explicit route-delegation regression for the analyze endpoint.
- This keeps the backend refactor moving in the adjacent analysis domain without overextending scope: only the heaviest write-style route in the analysis-task module moved behind a compat seam, while the lighter status-query endpoints remain unchanged for now.

Refreshed backend recommendation:
1. Revisit the remaining endpoints in `backend/app/api/chapter_analysis_task_routes.py` later as a second slice if route-level churn is still worth it, especially `analysis/status` and `can-generate`.
2. Continue favoring one heavy endpoint per iteration when a split-route module mixes read and write behavior; this has kept regressions cheap to isolate.
3. Keep monkeypatch seams anchored in compat services once a route delegates, so later refactors do not need to touch test setup repeatedly.


## Progress Refresh / 2026-04-21 / Iteration 149

Backend progress update:
- Extended `backend/app/services/chapter_analysis_task_route_compat_service.py` so the split `analysis/status` and `can-generate` query routes now share focused compat entries for authenticated chapter access, status payload recovery/commit handling, and generation-prerequisite response assembly.
- Simplified `backend/app/api/chapter_analysis_task_routes.py` further so the two lightweight query endpoints now delegate to the compat layer, leaving only the batch status route inline in this module.
- Added explicit route-delegation regression coverage in `backend/tests/test_api/test_chapters_analysis.py` for both `/analysis/status` and `/can-generate`, while keeping the existing integration-style behavior tests in `backend/tests/test_api/test_chapters.py` as the persisted contract checks.
- This effectively finishes the low-risk route-level cleanup for the analysis-task module: the write-heavy analyze trigger and the two single-chapter query routes now all delegate their default wiring to one focused compat service.

Refreshed backend recommendation:
1. The main remaining route-level candidate in this module is `POST /analysis/status/batch`; evaluate it later only if the extra seam is worth the added cross-project access-check complexity.
2. If staying in the backend, the next higher-value seam is likely back in `chapter_partial_regeneration_routes.py` for `apply-partial-regenerate`, but that route is materially riskier because it mutates chapter content.
3. Keep preferring one module at a time: finishing a module?s low-risk routes before switching domains has kept the plan and tests easy to reason about.


## Progress Refresh / 2026-04-21 / Iteration 150

Backend progress update:
- Extended `backend/app/services/chapter_partial_regeneration_route_compat_service.py` with `apply_partial_regenerate_with_default_route_wiring`, so the split apply route now delegates authenticated chapter access, sanitization, workflow-meta rejection, position validation, and chapter-content writeback through the existing compat layer.
- Simplified `backend/app/api/chapter_partial_regeneration_routes.py` so both partial-regenerate endpoints now share the same compat-service boundary, keeping the route module focused on request/dependency glue only.
- Added explicit route-delegation coverage in `backend/tests/test_api/test_chapters_stream_routes.py` for `/apply-partial-regenerate`, while existing sanitize/writeback regressions in `backend/tests/test_api/test_chapters_stream_routes.py` and `backend/tests/test_api/test_chapters.py` continue to verify the persisted behavior.
- This completes the low-risk route cleanup for the partial-regeneration module: the stream path and the apply path now both delegate to one focused compat seam without changing the external request/response contract.

Refreshed backend recommendation:
1. The remaining backend route-level candidates are now mostly higher-complexity or lower-payoff seams; reassess value before continuing to split them.
2. If continuing in backend, consider `POST /analysis/status/batch` only if consolidating the last inline route in `chapter_analysis_task_routes.py` is worth the extra access-control indirection.
3. Otherwise it may be a good breakpoint to review overall backend refactor progress and re-rank remaining modules by payoff rather than by adjacency alone.


## Progress Refresh / 2026-04-21 / Iteration 151

Backend progress update:
- Extended `backend/app/services/chapter_analysis_task_route_compat_service.py` with `get_batch_analysis_task_status_with_default_route_wiring`, so the last remaining inline route in the analysis-task module now delegates chapter-id normalization, query-context loading, per-project access validation, stale-status recovery commit handling, and response assembly through the compat layer.
- Simplified `backend/app/api/chapter_analysis_task_routes.py` so all four endpoints in the split analysis-task module now delegate to the same focused compat service, leaving the route file as pure request/dependency glue.
- Added explicit route-delegation coverage in `backend/tests/test_api/test_chapters_analysis.py` for `POST /analysis/status/batch`, while the existing empty-input route regression continues to verify the zero-chapter shortcut path.
- This effectively completes the low-risk compat cleanup for `chapter_analysis_task_routes.py`: the module no longer carries default orchestration or access-binding logic inline.

Refreshed backend recommendation:
1. At this point the obvious low-risk split-route compat seams are mostly exhausted; the next backend steps should be re-ranked by payoff rather than by adjacency.
2. If continuing route-level cleanup, prefer a short review pass over the remaining split route modules to find any similarly tiny outliers before committing to heavier service-layer refactors.
3. Otherwise this is a good breakpoint to summarize backend progress against the clean plan and decide whether to pivot to broader consolidation, docs, or staging the accumulated changes.



## Progress Refresh / 2026-04-21 / Iteration 152

Backend review update:
- Completed a backend review pass after `Iteration 151` and confirmed that the low-risk split-route compat campaign is largely finished. The modules already covered by focused route compat services now include generation, batch generation, regeneration stream paths, partial regeneration stream and apply paths, analysis read routes, annotation routes, expansion-plan update, and the full analysis-task route set.
- The remaining split route modules are no longer the same class of work. `backend/app/api/chapter_quality_routes.py` is already a very thin view route with little compat payoff. `backend/app/api/chapter_regeneration_routes.py` mostly leaves only `GET /regeneration/tasks` inline. `backend/app/api/chapter_batch_generation_routes.py` still has a few inline endpoints, but they are mostly simple auth, status, and cancel handlers rather than heavy default dependency bundles.
- The largest untouched backend route modules are now `backend/app/api/chapter_crud_routes.py` and `backend/app/api/chapter_draft_routes.py`. Both still contain repeated access loading plus write-side business logic, but they are no longer ideal route-compat candidates. Their complexity comes from domain mutations, history application, project word-count syncing, and side-effect coordination rather than from bulky default wiring alone.
- Based on that review, the next backend phase should not continue adjacency-driven compat extraction by default. The better split is now by payoff: either stop at this route-boundary milestone and stabilize it, or pivot into deeper service-layer consolidation for the highest-value business workflows.

Refined backend priority tiers:
1. **Pause / Stabilize now** - Highest recommendation. The low-risk route-boundary goal is effectively met; use this breakpoint to summarize completed work, review diffs, and decide whether to batch, commit, or stage the backend changes.
2. **Targeted service consolidation in `chapter_draft_routes.py`** - Medium payoff, high complexity. Worth doing only if draft apply or recovery flows are a real maintenance pain point, because most remaining complexity lives below the route layer.
3. **Targeted service consolidation in `chapter_crud_routes.py`** - Medium payoff, high complexity. Candidate only if create, update, delete, or navigation logic is expected to keep changing; otherwise the current inline shape may be acceptable.
4. **Tiny leftover compat extractions** - Low payoff. `chapter_regeneration_routes.py` task listing or a few `chapter_batch_generation_routes.py` auth and status handlers can still be moved, but they are no longer likely to materially improve maintainability.
5. **Defer `chapter_quality_routes.py`** - Very low payoff. It is already close to pure glue and not worth another seam on its own.

Refined recommendation:
1. Treat the current backend state as the end of the low-risk route compat phase and avoid continuing seam extraction just to make the compat list longer.
2. If more backend refactor work is needed, choose a business workflow (`draft apply`, `chapter CRUD`, or batch task lifecycle) and refactor it as a service-boundary project rather than as another route-only cleanup slice.
3. Before the next coding phase, produce a short backend milestone summary so the remaining work is driven by maintenance pain and payoff instead of adjacency.

## Progress Refresh / 2026-04-21 / Iteration 153

Backend progress update:
- Started the post-compat backend phase by moving the chapter draft workflow orchestration out of `backend/app/api/chapter_draft_routes.py` into the new `backend/app/services/chapter_draft_workflow_service.py`.
- The new workflow service now owns the repeated draft-loading, stale-check, content sanitizing, apply-history creation, and response-payload assembly for both auto-revision drafts and candidate drafts.
- `backend/app/api/chapter_draft_routes.py` is now reduced to route-boundary responsibilities only: authentication, chapter access loading, request parsing, and delegation into the workflow service.
- This is intentionally different from the earlier compat slices. The goal here is not to grow the compat-service list, but to start consolidating a real business workflow that was still duplicated inside the route layer.

Updated backend recommendation:
1. Validate the draft workflow extraction with the existing draft endpoint regression suite before expanding scope.
2. If this stays stable, continue within the same workflow boundary by deciding whether draft-detail reads should remain in the same workflow service or split into a dedicated query service later.
3. Only after the draft workflow settles should the next deeper backend target be re-evaluated against `chapter_crud_routes.py`.

## Progress Refresh / 2026-04-21 / Iteration 154

Backend progress update:
- Continued the `chapter_draft` service-boundary refactor by separating read-side draft detail assembly from write-side apply orchestration.
- Added `backend/app/services/chapter_draft_query_service.py` so auto-revision draft detail reads and candidate draft detail reads now share a dedicated query boundary instead of living inside the workflow apply service.
- Reduced `backend/app/services/chapter_draft_workflow_service.py` back to write-oriented responsibilities only: stale validation, content sanitizing, history-entry creation, and persisted chapter apply flow.
- Kept `backend/app/api/chapter_draft_routes.py` as a thin route shell while making the read/write split explicit at the service layer, which should make later draft maintenance safer than continuing to grow one mixed service.

Updated backend recommendation:
1. Keep `chapter_draft_query_service.py` focused on read models and avoid reintroducing apply-side logic into it.
2. If the draft area remains stable after this split, the next worthwhile move is to normalize shared draft error messages and request contracts without broadening route scope.
3. Only after the draft read/write boundary settles should the next heavier backend candidate be reconsidered against `chapter_crud_routes.py`.

## Progress Refresh / 2026-04-21 / Iteration 155

Backend progress update:
- Continued the `chapter_draft` refactor by adding `backend/app/services/chapter_draft_state_service.py` as a shared state-and-contract layer below both the query service and the workflow service.
- Centralized draft loading helpers, preview-only validation, and shared error and success message constants so query-side and apply-side flows no longer maintain parallel copies of the same contract details.
- Kept the layering clearer: `chapter_draft_query_service.py` now focuses on read payload assembly, `chapter_draft_workflow_service.py` focuses on persisted apply workflows, and `chapter_draft_state_service.py` owns shared draft retrieval and validation primitives.
- This gives the draft domain a more stable internal boundary and reduces the chance that later changes to not-found, preview-only, or stale-draft behavior drift across read and write paths.

Updated backend recommendation:
1. Use the new draft state layer as the single source of truth for future draft-specific contract changes.
2. If further cleanup is still needed in the draft domain, prefer tightening type contracts and request schemas before expanding to another route module.
3. Re-evaluate `chapter_crud_routes.py` only after the draft stack stays stable through another regression pass.

## Progress Refresh / 2026-04-21 / Iteration 156

Backend progress update:
- Started the `chapter_crud` backend phase by extracting the write-side chapter lifecycle into `backend/app/services/chapter_crud_workflow_service.py`.
- Moved create, update, and delete chapter orchestration out of `backend/app/api/chapter_crud_routes.py`, including project word-count synchronization and delete-side cleanup hooks.
- Kept list, detail, and navigation reads in the route for now, so the first CRUD slice stays focused on the higher-payoff write workflows rather than widening into read-model reshaping too early.
- Added explicit route-delegation regressions for create, update, and delete, while the existing CRUD behavior test continues to cover persisted word-count and lifecycle behavior.

Updated backend recommendation:
1. If CRUD refactor continues, the natural second slice is a `chapter_crud_query_service.py` for project chapter list and navigation payload assembly.
2. Keep the write-side workflow service focused on mutations and side effects; avoid pulling read-model shaping into it.
3. Reassess whether `chapter_crud` still has enough remaining payoff before starting any lower-value route-only cleanup elsewhere.

## Progress Refresh / 2026-04-21 / Iteration 157

Backend progress update:
- Continued the `chapter_crud` refactor by adding `backend/app/services/chapter_crud_query_service.py` for project chapter list assembly and chapter navigation payload loading.
- Simplified `backend/app/api/chapter_crud_routes.py` so project list and navigation routes now delegate their read-side query work instead of building result payloads inline.
- At this point the CRUD module has a clearer split: route for auth/access and delegation, workflow service for mutations, and query service for read-model shaping.
- Added explicit route-delegation regressions for the project chapter list and navigation endpoints, while the existing CRUD behavior tests still cover the persisted contract.

Updated backend recommendation:
1. `chapter_crud_routes.py` is now close to a stable service-boundary target; avoid widening it unless a new maintenance pain appears.
2. If backend refactor continues, the next candidate should be re-ranked between `chapter_regeneration_routes.py` small leftovers and broader cleanup outside the route layer.
3. Prefer a brief backend milestone summary after this CRUD phase before starting another large service split.

## Progress Refresh / 2026-04-21 / Iteration 158

Backend milestone update:
- After the `chapter_draft` and `chapter_crud` refactors, the backend route-boundary cleanup now has two stable workflow areas with explicit service layering instead of route-heavy business logic.
- Re-ranked the remaining low-risk candidates and selected `GET /regeneration/tasks` in `backend/app/api/chapter_regeneration_routes.py` as the best small follow-up, because it has a tiny inline query, direct existing coverage, and no broader workflow coupling.
- Added `backend/app/services/chapter_regeneration_query_service.py` so regeneration task history loading now sits behind a small read-side service instead of staying inline in the route.
- Kept this slice intentionally narrow: the regeneration SSE workflow remains in the existing compat path, while task-history reading now follows the same route-to-query delegation pattern established in the CRUD cleanup.

Updated backend recommendation:
1. Treat the current backend state as a good milestone boundary for summarizing completed refactor themes rather than immediately starting another large service split.
2. Remaining route-only leftovers in batch generation are now lower priority than documentation, milestone capture, or targeted fixes driven by actual maintenance pain.
3. If more backend cleanup is still desired later, prefer narrow query or workflow extractions with direct tests over broad adjacency-driven campaigns.

## Progress Refresh / 2026-04-21 / Iteration 159

Backend milestone documentation update:
- Added `docs/architecture/backend-refactor-milestone-summary-2026-04-21.zh-CN.md` to capture the completed backend refactor themes, current boundary patterns, validation approach, and remaining priority tiers.
- The document explicitly marks the current backend state as a recommended milestone stop point: the highest-payoff route-boundary cleanup is done, and future backend work should be selected by maintenance pain rather than by route adjacency.
- It also records the current three-way pattern split: route compat cleanup, query/workflow service layering, and narrow query extractions for low-risk leftovers.
- This gives the project a stable written handoff for the backend refactor campaign instead of relying only on the iterative clean-plan log.

Updated backend recommendation:
1. Use the milestone document as the default handoff reference before starting any new backend refactor slice.
2. Prefer consolidating the current completed state over opening another broad backend campaign immediately.
3. If backend refactor resumes later, require a concrete maintenance pain point as the entry condition.

## Progress Refresh / 2026-04-21 / Iteration 160

Backend handoff inventory update:
- Added `docs/architecture/backend-refactor-change-inventory-2026-04-21.zh-CN.md` to catalog the concrete backend files touched in this refactor phase, grouped by compat seams, workflow/query layering, tests, docs, and remaining low-priority items.
- The inventory document also proposes a practical commit grouping strategy so the current backend work can be reviewed or staged in coherent slices instead of as one large mixed patch.
- This complements the milestone summary by shifting from outcome-oriented narration to file-level handoff and acceptance planning.
- With both documents present, the current backend refactor state is now documented at both the milestone level and the change-inventory level.

Updated backend recommendation:
1. Use the change inventory document when preparing commit batches or manual acceptance.
2. Avoid opening new backend refactor scope until the existing slices are either validated or staged.
3. If another development round starts later, reopen from the documented remaining-items list instead of from raw git status.

## Progress Refresh / 2026-04-21 / Iteration 161

Backend commit batching update:
- Added `docs/architecture/backend-refactor-commit-batches-2026-04-21.zh-CN.md` to turn the current backend refactor state into a concrete staging and review plan.
- The document splits the backend work into five practical commit batches: route compat cleanup, `chapter_draft`, `chapter_crud`, regeneration task query, and docs/plan handoff.
- Each batch now has a suggested file scope, minimal validation entry, and commit-message direction, which makes the current backend work easier to stage without relying on memory or ad-hoc grouping.
- This gives the project a third handoff layer after the milestone summary and change inventory: a directly actionable staging plan.

Updated backend recommendation:
1. If the next action is submission or review, use the commit batches document rather than continuing code changes.
2. Treat the current backend work as ready for staged acceptance instead of opening another refactor branch immediately.
3. Resume backend coding only after one of the documented batches exposes a concrete follow-up issue.

## Progress Refresh / 2026-04-21 / Iteration 162

Backend staging-command update:
- Added `docs/architecture/backend-refactor-staging-commands-2026-04-21.zh-CN.md` as an execution-oriented companion to the milestone, inventory, and commit-batches documents.
- The new document provides both a conservative two-batch staging path and a finer-grained multi-batch path, with explicit `git add` / `git add -p` guidance for overlapping files such as `test_chapters.py` and `chapter_regeneration_routes.py`.
- It also explicitly lists files that should stay out of the default backend batches unless reviewed separately, reducing the risk of accidentally mixing unrelated changes into backend acceptance work.
- This makes the current backend refactor handoff complete at four levels: milestone summary, change inventory, commit grouping, and staging commands.

Updated backend recommendation:
1. If the next action is staging or submission, stop coding and use the staging-commands document directly.
2. Prefer the conservative two-batch path unless there is a strong review reason to split more aggressively.
3. Resume backend development only after the staged batches reveal a concrete follow-up issue.

## Progress Refresh / 2026-04-21 / Iteration 163

Frontend stabilization update:
- Completed runtime validation for the current frontend refactor batch: `npm run validate:services`, `npx tsc -b --pretty false`, `npm run build`, and `npm run lint -- --quiet` all passed.
- This confirms the service-layer modularization, chapter-page helper extraction, floating-index layering, and background-task presentation split are currently in a stable acceptance state rather than a speculative in-progress state.
- Added frontend handoff documents for milestone summary, change inventory, commit grouping, and staging commands so the next iteration can move directly into selective staging instead of reopening broad exploratory refactors.
- The recommended next action is to stop expanding the frontend code surface, stage the frontend changes in focused batches, and only resume code edits if batch validation exposes a concrete defect.


## Progress Refresh / 2026-04-21 / Iteration 164

Frontend staging update:
- Staged frontend batch A for the service-layer modularization track, including the facade guard script, modular service entry, domain service modules, shared HTTP client, service-layer lint/build hooks, and the service-layer conventions document.
- Verified the staged file scope stays inside the intended batch boundary and does not accidentally mix `Chapters.tsx`, floating-index refactors, or background-task-center changes into the service batch.
- `git diff --cached --check` passed for the staged batch, so the current index is ready for a focused frontend service-layer commit when desired.
- Recommended next action: either commit batch A now, or keep the current staged index intact and prepare the batch B staging pass only after batch A is submitted.


## Progress Refresh / 2026-04-21 / Iteration 165

Final delivery update:
- Added a final delivery summary document that consolidates backend milestones, frontend milestones, validation status, commit history, remaining risks, and recommended handoff materials into a single closing reference.
- Confirmed the working tree is clean after the refactor and documentation submission chain.
- Confirmed the superseded legacy roadmap artifact remains excluded only through local `.git/info/exclude`, so repository history stays focused on the clean-plan baseline.
- The refactor track should now be considered closed unless a new follow-up task is opened for performance, package-size, or residual transition-debt cleanup.


## Progress Refresh / 2026-04-21 / Iteration 166

Release-note update:
- Added a team-facing release note / MR description draft that summarizes the refactor scope, validation status, risks, rollback guidance, and recommended review order.
- This closes the gap between internal handoff documents and reviewer-facing communication material.
- The repository remains clean after generating the release-note draft, so the remaining work is purely optional follow-up communication.
- The current refactor track should now be considered closed both technically and operationally.


## Progress Refresh / 2026-04-23 / Iteration 167

Frontend and backend integration stabilization update:
- Completed a real backend auth smoke verification against the Docker-exposed application on port 8004. The backend auth contract remained healthy for config fetch, login, current-user lookup, refresh, project access, and logout flow.
- Completed a real Playwright auth regression against the same backend and identified the frontend auth interceptor as the primary failure point rather than the backend API.
- Updated `frontend/src/services/core/httpClient.ts` so 401 handling now preserves backend error details, avoids forced redirect loops on auth requests and auth pages, and redirects unauthenticated non-auth pages through the shared redirect builder instead of a bare `/login` jump.
- Re-ran frontend validation and real end-to-end auth regression successfully: `npm run validate:text`, `npm run build`, and `APP_PORT=8004 E2E_REAL_BACKEND=1 npm run e2e:auth` all passed, with Playwright reporting 7/7 passing tests.
- This closes the currently exposed auth-state regression chain and reduces the risk of user-facing `Network request failed` / misplaced login redirects during fast page switching while background work is active.

Updated recommendation:
1. Keep the new 401 strategy as the default shared behavior unless a specific module requires a stricter redirect policy.
2. If the next iteration continues frontend stabilization, prioritize visual spot checks around the chapter reader and chapter analysis panel under expired-session conditions.
3. If later regressions appear, inspect shared HTTP interception first before expanding backend debugging scope.


## Progress Refresh / 2026-04-24 / Iteration 168

Frontend chapter-reader and analysis-panel stabilization update:
- Completed a focused display and state-boundary sweep for `frontend/src/pages/ChapterReader.tsx` and `frontend/src/components/ChapterAnalysis.tsx` after the earlier auth-chain fixes.
- Added abort-driven request cleanup for the chapter reader load flow so rapid module switching or chapter switching no longer leaves stale responses racing back into the UI.
- Added the same request-cancellation and stale-state protection to the chapter analysis modal flow, including initial status fetch, result fetch, polling requests, and modal-close cleanup.
- Fixed a reader-side rendering bug where invalid annotation positions were detected but the original unfiltered annotation list was still rendered, which could produce display anomalies in the annotated text view and sidebar.
- Fixed analysis-panel state reset behavior so a chapter with no analysis task no longer reuses stale analysis results from the previous chapter session.
- Validation passed after the changes: `npm run validate:text`, `npm run validate:services`, and `npm run build` all succeeded.

Updated recommendation:
1. Treat the chapter reader and chapter analysis panel as stabilized for the current front-end cleanup pass.
2. If a final polish round is still desired, prioritize manual visual checks for mobile drawer layout, long-title wrapping, and background-task polling under deliberately slow network conditions.
3. Keep using abort-based cleanup for other long-lived page panels that still rely on route-driven or modal-driven async loading.
