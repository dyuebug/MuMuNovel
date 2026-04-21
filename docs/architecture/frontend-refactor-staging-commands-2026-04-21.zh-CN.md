# ???????????2026-04-21?

## 1. ????

????????????????

- `cd frontend && npm run validate:services`
- `cd frontend && npx tsc -b --pretty false`
- `cd frontend && npm run build`
- `cd frontend && npm run lint -- --quiet`

???????????????????????????????

## 2. ?? A????????????

```bash
git add frontend/package.json
git add frontend/eslint.config.js
git add frontend/scripts/check-service-facade.mjs
git add frontend/src/services/api.ts
git add frontend/src/services/modularApi.ts
git add frontend/src/services/core
git add frontend/src/services/modules
git add frontend/src/services/CLAUDE.md
git add docs/architecture/frontend-service-layer-conventions.zh-CN.md
```

## 3. ?? B??????????

```bash
git add frontend/src/pages/Chapters.tsx
git add frontend/src/pages/chapter*.ts
git add frontend/src/components/ChapterAnalysisEntry.tsx
git add frontend/src/components/ChapterBasicModalEntry.tsx
git add frontend/src/components/ChapterBatchGenerateModalEntry.tsx
git add frontend/src/components/ChapterBatchProgressEntry.tsx
git add frontend/src/components/ChapterListSection.tsx
git add frontend/src/components/ChapterPlanEditorEntry.tsx
git add frontend/src/components/ChapterReaderEntry.tsx
git add frontend/src/components/SingleChapterGenerationOverlayEntry.tsx
git add frontend/src/components/FloatingIndex*.tsx
git add frontend/src/hooks/useFloatingIndex*.ts
git add frontend/src/hooks/useChapterFloatingIndex*.ts
git add frontend/src/utils/floatingIndexPanel*.ts
git add frontend/src/utils/chapterActionDialogs.tsx
git add frontend/src/store/hooks.ts
git add frontend/src/store/chapterGenerationWorkflow.ts
git add frontend/src/store/entityCrudSyncHooks.ts
git add frontend/src/store/projectCollectionRefresh.ts
git add frontend/src/store/projectSyncHelpers.ts
git add frontend/src/store/storeMutationHelpers.ts
```

## 4. ?? C????????????

```bash
git add frontend/src/components/BackgroundTaskCenter.tsx
git add frontend/src/components/backgroundTaskPresentation.ts
git add frontend/src/store/backgroundTasks.ts
git add frontend/src/store/backgroundTaskModel.ts
git add frontend/src/store/backgroundTaskSelectors.ts
git add frontend/src/store/backgroundTaskStateHelpers.ts
git add frontend/src/hooks/useRestorableBackgroundTaskPolling.ts
git add frontend/README.md
git add frontend/CLAUDE.md
git add frontend/src/components/CLAUDE.md
git add frontend/src/store/CLAUDE.md
```

## 5. ????????????????

???????????????????????????????? runtime ???

```bash
README.md
docs/README.md
docs/01-????.md
docs/05-????.md
docs/07-????.md
docs/10-????.md
plan/2026-04-20_16-32-00-refactor-roadmap.md
backend/app/api/CLAUDE.md
```

## 6. ????

1. ????????????? `git diff --cached --name-only`?
2. ?????????????????????? `git add -p` ??????
3. ?????????????????????????????
