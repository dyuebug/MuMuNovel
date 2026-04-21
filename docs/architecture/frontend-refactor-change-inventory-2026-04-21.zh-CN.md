# ?????????2026-04-21?

## 1. ????

?????????????????????????????????????

## 2. ????????

### ????

- `frontend/package.json`
- `frontend/eslint.config.js`
- `frontend/scripts/check-service-facade.mjs`
- `frontend/src/services/api.ts`
- `frontend/src/services/modularApi.ts`
- `frontend/src/services/core/`
- `frontend/src/services/modules/`
- `docs/architecture/frontend-service-layer-conventions.zh-CN.md`

### ??

- ? API ???????????????????
- ??????????????????
- ? lint / build ?????????????

## 3. `Chapters.tsx` ??????

### ????? helpers

- `frontend/src/pages/Chapters.tsx`
- `frontend/src/pages/chapterActionDialogCoordinationHelpers.ts`
- `frontend/src/pages/chapterAnalysisTaskCoordinationHelpers.ts`
- `frontend/src/pages/chapterAnalysisTaskInteractionHelpers.ts`
- `frontend/src/pages/chapterAnalysisTaskLoadHelpers.ts`
- `frontend/src/pages/chapterAnalysisTaskPollingHelpers.ts`
- `frontend/src/pages/chapterBatchGenerationCoordinationHelpers.ts`
- `frontend/src/pages/chapterBatchGenerationPollingHelpers.ts`
- `frontend/src/pages/chapterBatchGenerationRequestHelpers.ts`
- `frontend/src/pages/chapterBatchGenerationRestoreHelpers.ts`
- `frontend/src/pages/chapterBatchGenerationWorkflowHelpers.ts`
- `frontend/src/pages/chapterBatchTaskMetaStorageHelpers.ts`
- `frontend/src/pages/chapterDeferredBatchAnalysisHelpers.ts`
- `frontend/src/pages/chapterEditorLifecycleHelpers.ts`
- `frontend/src/pages/chapterEditorOpenHelpers.ts`
- `frontend/src/pages/chapterFloatingIndexPanelHelpers.ts`
- `frontend/src/pages/chapterFloatingIndexPanelLifecycleHelpers.ts`
- `frontend/src/pages/chapterFloatingIndexTriggerHelpers.ts`
- `frontend/src/pages/chapterModalOpenHelpers.ts`
- `frontend/src/pages/chapterModalSubmitHelpers.ts`
- `frontend/src/pages/chapterModelLoadHelpers.ts`
- `frontend/src/pages/chapterPlanEditorDataHelpers.ts`
- `frontend/src/pages/chapterPlanEditorLifecycleHelpers.ts`
- `frontend/src/pages/chapterPlanEditorModalHelpers.ts`
- `frontend/src/pages/chapterProjectCoordinationHelpers.ts`
- `frontend/src/pages/chapterReaderLifecycleHelpers.ts`
- `frontend/src/pages/chapterReaderModalHelpers.ts`
- `frontend/src/pages/chapterSelectionHelpers.ts`
- `frontend/src/pages/chapterSingleGenerationHelpers.ts`
- `frontend/src/pages/chapterStoryCreationAutoSyncHelpers.ts`
- `frontend/src/pages/chapterStoryCreationCurrentDraftHelpers.ts`
- `frontend/src/pages/chapterStoryCreationDerivedStateHelpers.ts`
- `frontend/src/pages/chapterStoryCreationDraftPersistHelpers.ts`
- `frontend/src/pages/chapterStoryCreationPersistenceCoordinationHelpers.ts`
- `frontend/src/pages/chapterStoryCreationPromptHelpers.ts`
- `frontend/src/pages/chapterStoryCreationRestoreHelpers.ts`
- `frontend/src/pages/chapterStoryCreationSnapshotApplyHelpers.ts`
- `frontend/src/pages/chapterStoryCreationSnapshotDeleteHelpers.ts`
- `frontend/src/pages/chapterStoryCreationSnapshotHelpers.ts`
- `frontend/src/pages/chapterStoryCreationSnapshotSaveHelpers.ts`
- `frontend/src/pages/chapterStoryCreationSnapshotWorkflowHelpers.ts`
- `frontend/src/pages/chapterWritingStyleLoadHelpers.ts`

### ????

- `frontend/src/components/ChapterAnalysisEntry.tsx`
- `frontend/src/components/ChapterBasicModalEntry.tsx`
- `frontend/src/components/ChapterBatchGenerateModalEntry.tsx`
- `frontend/src/components/ChapterBatchProgressEntry.tsx`
- `frontend/src/components/ChapterListSection.tsx`
- `frontend/src/components/ChapterPlanEditorEntry.tsx`
- `frontend/src/components/ChapterReaderEntry.tsx`
- `frontend/src/components/SingleChapterGenerationOverlayEntry.tsx`

## 4. ????????

### ??

- `frontend/src/components/FloatingIndexPanel.tsx`
- `frontend/src/components/FloatingIndexPanelEntry.tsx`
- `frontend/src/components/FloatingIndexPanelContent.tsx`
- `frontend/src/components/FloatingIndexPanelDrawer.tsx`
- `frontend/src/components/FloatingIndexPanelResults.tsx`
- `frontend/src/components/FloatingIndexPanelSearchHeader.tsx`
- `frontend/src/components/FloatingIndexGroupSection.tsx`
- `frontend/src/components/FloatingIndexChapterRow.tsx`

### hooks / utils

- `frontend/src/hooks/useFloatingIndexPanelBindings.ts`
- `frontend/src/hooks/useFloatingIndexPanelLifecycle.ts`
- `frontend/src/hooks/useFloatingIndexPanelState.ts`
- `frontend/src/hooks/useFloatingIndexPanelViewModel.ts`
- `frontend/src/hooks/useFloatingIndexSearchState.ts`
- `frontend/src/hooks/useFloatingIndexTriggerProps.ts`
- `frontend/src/hooks/useChapterFloatingIndexPanelBindings.ts`
- `frontend/src/hooks/useChapterFloatingIndexPanelLifecycle.ts`
- `frontend/src/hooks/useChapterFloatingIndexPanelState.ts`
- `frontend/src/hooks/useChapterFloatingIndexTriggerProps.ts`
- `frontend/src/utils/floatingIndexPanelContracts.ts`
- `frontend/src/utils/floatingIndexPanelLifecycle.ts`
- `frontend/src/utils/floatingIndexPanelState.ts`
- `frontend/src/utils/floatingIndexPanelTriggerProps.ts`
- `frontend/src/utils/floatingIndexPanelViewHelpers.ts`

## 5. ??????????

- `frontend/src/components/BackgroundTaskCenter.tsx`
- `frontend/src/components/backgroundTaskPresentation.ts`
- `frontend/src/store/backgroundTasks.ts`
- `frontend/src/store/backgroundTaskModel.ts`
- `frontend/src/store/backgroundTaskSelectors.ts`
- `frontend/src/store/backgroundTaskStateHelpers.ts`
- `frontend/src/hooks/useRestorableBackgroundTaskPolling.ts`

## 6. Store ??????

- `frontend/src/store/hooks.ts`
- `frontend/src/store/chapterGenerationWorkflow.ts`
- `frontend/src/store/entityCrudSyncHooks.ts`
- `frontend/src/store/projectCollectionRefresh.ts`
- `frontend/src/store/projectSyncHelpers.ts`
- `frontend/src/store/storeMutationHelpers.ts`

## 7. ???????

- `frontend/README.md`
- `frontend/CLAUDE.md`
- `frontend/src/components/CLAUDE.md`
- `frontend/src/services/CLAUDE.md`
- `frontend/src/store/CLAUDE.md`

## 8. ??????? runtime ??????

??????????????????????????????????

- `README.md`
- `docs/README.md`
- `docs/01-????.md`
- `docs/05-????.md`
- `docs/07-????.md`
- `docs/10-????.md`
- `plan/2026-04-20_16-32-00-refactor-roadmap.md`
