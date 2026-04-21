# ???????????2026-04-21?

## 1. ????

??????????????????????????????????????????????????????

?????????????????????????

- ???????? `git add`
- ???????????? backend ??
- ???????????????? `git add -p`

## 2. ????? backend ?????

??????????????? backend ?????

- `backend/app/api/CLAUDE.md`
- `docs/architecture/frontend-service-layer-conventions.zh-CN.md`
- `plan/2026-04-20_16-32-00-refactor-roadmap.md`

???

- ??????? backend ???????
- ??? backend ???????
- ???????????????

## 3. ?????????????

??????????? backend ?????????????????????????

### Batch 1????????

?????

- ?????? backend ????????
- ??? `test_chapters.py` ???????????? hunk ???

?????

```powershell
git add --       backend/app/api/chapter_analysis_routes.py       backend/app/api/chapter_analysis_task_routes.py       backend/app/api/chapter_annotation_routes.py       backend/app/api/chapter_batch_generation_routes.py       backend/app/api/chapter_crud_routes.py       backend/app/api/chapter_draft_routes.py       backend/app/api/chapter_expansion_plan_routes.py       backend/app/api/chapter_generation_routes.py       backend/app/api/chapter_partial_regeneration_routes.py       backend/app/api/chapter_quality_routes.py       backend/app/api/chapter_regeneration_routes.py       backend/app/services/batch_generation_route_compat_service.py       backend/app/services/chapter_analysis_route_compat_service.py       backend/app/services/chapter_analysis_task_route_compat_service.py       backend/app/services/chapter_annotation_route_compat_service.py       backend/app/services/chapter_crud_query_service.py       backend/app/services/chapter_crud_workflow_service.py       backend/app/services/chapter_draft_query_service.py       backend/app/services/chapter_draft_state_service.py       backend/app/services/chapter_draft_workflow_service.py       backend/app/services/chapter_expansion_plan_route_compat_service.py       backend/app/services/chapter_generation_route_compat_service.py       backend/app/services/chapter_generation_stream_entry_service.py       backend/app/services/chapter_generation_stream_wiring_service.py       backend/app/services/chapter_partial_regeneration_route_compat_service.py       backend/app/services/chapter_regeneration_query_service.py       backend/app/services/chapter_regeneration_route_compat_service.py       backend/app/services/project_quality_trend_compat_service.py       backend/tests/test_api/chapters_test_support.py       backend/tests/test_api/test_chapters.py       backend/tests/test_api/test_chapters_analysis.py       backend/tests/test_api/test_chapters_batch_generation.py       backend/tests/test_api/test_chapters_batch_status_resume.py       backend/tests/test_api/test_chapters_quality_views.py       backend/tests/test_api/test_chapters_stream_routes.py
```

??????

```powershell
git diff --cached --name-only
```

???????

```powershell
git commit -m "refactor(backend): consolidate chapter route boundaries and workflows"
```

### Batch 2?backend ??? clean plan

?????

```powershell
git add --       docs/architecture/backend-refactor-milestone-summary-2026-04-21.zh-CN.md       docs/architecture/backend-refactor-change-inventory-2026-04-21.zh-CN.md       docs/architecture/backend-refactor-commit-batches-2026-04-21.zh-CN.md       docs/architecture/backend-refactor-staging-commands-2026-04-21.zh-CN.md       plan/2026-04-21_10-45-59-frontend-backend-refactor-plan-clean.md
```

??????

```powershell
git diff --cached --name-only
```

???????

```powershell
git commit -m "docs(backend): add refactor milestone inventory batching and staging notes"
```

## 4. ?????????????????

??????????????????????

### Step A???? route compat ??

??????????? compat service ??? route ???

```powershell
git add --       backend/app/api/chapter_analysis_routes.py       backend/app/api/chapter_analysis_task_routes.py       backend/app/api/chapter_annotation_routes.py       backend/app/api/chapter_batch_generation_routes.py       backend/app/api/chapter_expansion_plan_routes.py       backend/app/api/chapter_generation_routes.py       backend/app/api/chapter_partial_regeneration_routes.py       backend/app/services/batch_generation_route_compat_service.py       backend/app/services/chapter_analysis_route_compat_service.py       backend/app/services/chapter_analysis_task_route_compat_service.py       backend/app/services/chapter_annotation_route_compat_service.py       backend/app/services/chapter_expansion_plan_route_compat_service.py       backend/app/services/chapter_generation_route_compat_service.py       backend/app/services/chapter_generation_stream_entry_service.py       backend/app/services/chapter_generation_stream_wiring_service.py       backend/app/services/chapter_partial_regeneration_route_compat_service.py       backend/app/services/project_quality_trend_compat_service.py       backend/app/api/chapter_quality_routes.py       backend/tests/test_api/test_chapters_analysis.py       backend/tests/test_api/test_chapters_batch_generation.py       backend/tests/test_api/test_chapters_batch_status_resume.py       backend/tests/test_api/test_chapters_quality_views.py       backend/tests/test_api/test_chapters_stream_routes.py
```

?? `backend/tests/test_api/test_chapters.py` ??? compat ? hunk???????

```powershell
git add -p -- backend/tests/test_api/test_chapters.py
```

### Step B?`chapter_draft`

```powershell
git add --       backend/app/api/chapter_draft_routes.py       backend/app/services/chapter_draft_query_service.py       backend/app/services/chapter_draft_state_service.py       backend/app/services/chapter_draft_workflow_service.py       backend/tests/test_api/test_chapters_stream_routes.py
```

? `backend/tests/test_api/test_chapters.py` ? draft ?? hunk?????

```powershell
git add -p -- backend/tests/test_api/test_chapters.py
```

### Step C?`chapter_crud`

```powershell
git add --       backend/app/api/chapter_crud_routes.py       backend/app/services/chapter_crud_query_service.py       backend/app/services/chapter_crud_workflow_service.py       backend/tests/test_api/chapters_test_support.py
```

? `backend/tests/test_api/test_chapters.py` ? CRUD ?? hunk?????

```powershell
git add -p -- backend/tests/test_api/test_chapters.py
```

### Step D?`chapter_regeneration` ???

?????`backend/app/api/chapter_regeneration_routes.py` ??????? compat ????? task query ???

?????????????????

```powershell
git add -p -- backend/app/api/chapter_regeneration_routes.py
git add -- backend/app/services/chapter_regeneration_query_service.py
git add -p -- backend/tests/test_api/test_chapters.py
```

?????? hunk ??????????????? Batch 1?

### Step E??????

```powershell
git add --       docs/architecture/backend-refactor-milestone-summary-2026-04-21.zh-CN.md       docs/architecture/backend-refactor-change-inventory-2026-04-21.zh-CN.md       docs/architecture/backend-refactor-commit-batches-2026-04-21.zh-CN.md       docs/architecture/backend-refactor-staging-commands-2026-04-21.zh-CN.md       plan/2026-04-21_10-45-59-frontend-backend-refactor-plan-clean.md
```

## 5. ??????????

????

```powershell
git diff --cached --name-only
```

???? diff?

```powershell
git diff --cached
```

??????????

```powershell
git reset
```

??????????????

```powershell
git restore --staged <file>
```

## 6. ??????

?????????????

- ?????????
- ??????????????????? + patch ??

??????????????

- ?? 4 ?????
- ? `test_chapters.py` ? `chapter_regeneration_routes.py` ? patch ???

## 7. ??

?? backend ?????????????????

?????????????????????????????????? backend ??????
