# Directory Structure

> How frontend code is organized in this project.

---

## Overview

The frontend is a React + TypeScript application that mixes route-oriented
pages with increasingly modularized app/router, service, store, and feature
layers.

Real routing is now split between `src/App.tsx` and
`src/app/router/AppRouter.tsx`: `App.tsx` owns the top-level shell
(`BrowserRouter`, deferred providers), while `AppRouter.tsx` owns the actual
route tree and lazy page wiring.

---

## Directory Layout

```text
frontend/
├── src/
│   ├── App.tsx
│   ├── app/
│   │   ├── router/         # route tree, suspense fallback, router helpers
│   │   ├── providers/      # deferred app-level features
│   │   └── layout/         # app shell slots
│   ├── pages/              # route-level page components
│   ├── components/         # reusable UI + heavy page sub-workflows
│   ├── services/           # HTTP clients and domain API modules
│   ├── store/              # Zustand stores and sync hooks
│   ├── features/           # domain-specific query/command/workflow helpers
│   ├── routes/             # project-page loader / preload utilities
│   ├── theme/              # theme mode/context/config
│   ├── types/              # shared TypeScript interfaces and unions
│   ├── utils/              # browser/runtime helpers, SSE infra, misc utils
│   ├── config/             # version and client-side config constants
│   └── assets/
├── e2e/                    # Playwright end-to-end coverage
├── public/
└── scripts/                # frontend validation/build helpers
```

---

## Module Organization

### App shell and routing

- Keep router shell concerns in `src/App.tsx`.
- Put concrete route definitions and lazy page imports in
  `src/app/router/AppRouter.tsx`.
- Project-specific lazy loaders and preload helpers belong in
  `src/routes/projectPageLoaders.ts`, not inline everywhere.

### Pages

- Route-level components go in `src/pages/`.
- Pages are not always thin, but they should still own page composition rather
  than duplicating service/store internals in many places.
- Before adding a page, decide whether it is:
  - a top-level route
  - a nested project route
  - an internal host-view inside an existing page such as `ProjectList.tsx`

### Components

- Put reusable UI and heavy page-local sub-workflows in `src/components/`.
- This directory is not a pure design-system layer; many files are business
  workflow components such as chapter generation/regeneration modals and the
  global background task center.
- If a component directly depends on store, routing, background tasks, or SSE,
  document that boundary and avoid letting it grow silently.

### Services, store, and features

- Domain HTTP modules live in `src/services/modules/`.
- Shared HTTP transport lives in `src/services/core/httpClient.ts`.
- Aggregate service exports live in `src/services/modularApi.ts`.
- Zustand stores live in `src/store/`.
- Query/command/workflow helpers increasingly live in `src/features/<domain>/`.
- New code should prefer the modular or feature-oriented layout rather than
  expanding legacy central files.

### Types and utilities

- Shared API and domain types live in `src/types/`.
- Generic browser/runtime helpers live in `src/utils/`.
- SSE infrastructure is currently utility-level infra
  (`src/utils/sseClient.ts`) consumed through services/modules.

---

## Naming Conventions

- Use PascalCase for React component/page filenames:
  `ProjectDetail.tsx`, `GenerationExecutionSettings.tsx`.
- Use camelCase or descriptive utility names for non-component TS files:
  `projectPageLoaders.ts`, `chunkLoadRecovery.ts`, `sidebarState.ts`.
- Keep folder names lowercase and domain-oriented:
  `services/modules`, `store`, `features/chapters`.
- When a subdomain becomes complex, prefer a focused subfolder or naming
  family instead of one giant file.

---

## Examples

- App shell and route tree:
  `frontend/src/App.tsx`,
  `frontend/src/app/router/AppRouter.tsx`
- Nested page composition:
  `frontend/src/pages/ProjectDetail.tsx`
- Domain feature split:
  `frontend/src/features/chapters/`,
  `frontend/src/features/projects/`
- Service/store separation:
  `frontend/src/services/modularApi.ts`,
  `frontend/src/store/index.ts`,
  `frontend/src/store/hooks.ts`

## Anti-Patterns

- Do not assume every file in `components/` is a small presentational widget.
- Do not add fresh runtime code to deprecated compatibility surfaces when a
  modular or feature-specific location already exists.
- Do not document or extend routes based only on page filenames; confirm the
  real route tree in `src/app/router/AppRouter.tsx`.
