# Quality Guidelines

> Code quality, testing, and review expectations for frontend changes.

---

## Overview

Frontend quality here means more than "passes TypeScript":

- routes, lazy loading, and page registration must stay consistent
- services/store/features should keep their boundaries clean
- changes to long-running task UX must preserve recovery and progress behavior
- validation scripts, lint, build, and Playwright remain the main safety net

The frontend currently has no component-unit-test suite, so review discipline
and E2E-aware validation matter more than in a heavily unit-tested UI repo.

---

## Design Expectations

- Keep route ownership in `src/app/router/AppRouter.tsx`.
- Reuse `services/modularApi.ts`, feature hooks, and Zustand stores instead of
  introducing page-local transport layers.
- Respect the distinction between page components, heavy workflow components,
  and shared infrastructure.
- Prefer extending existing domain modules (`features/`, `services/modules/`,
  `store/hooks.ts`) over creating parallel abstractions.

---

## Validation Expectations

- Run `npm run lint` for code-style and structural checks.
- Run `npm run build` for TypeScript + Vite validation.
- Keep the custom validation scripts green:
  `validate:services` and `validate:text` are part of the normal frontend
  build/lint path.
- Run targeted Playwright flows when the change affects auth, project
  navigation, background tasks, generation flows, or route registration.

---

## Testing Reality

- End-to-end tests live under `frontend/e2e/`.
- Existing coverage includes auth, background-task pages, wizard/background
  tasks, outline expansion, inspiration resume, and web-research payloads.
- There is currently no first-class component/unit test layer, so large UI
  refactors require stronger manual and E2E verification.

---

## Common Mistakes

- Adding a page file but forgetting to wire the route in
  `src/app/router/AppRouter.tsx`.
- Updating API shapes without tracing `types/`, `services/`, `store/`, and page
  consumers.
- Modifying background-task UI without checking persisted state and recovery
  flows.
- Treating `components/` as if everything inside were low-risk presentational
  code.

---

## Examples

- Route tree and lazy wiring:
  `frontend/src/app/router/AppRouter.tsx`
- Service validation boundary:
  `frontend/src/services/modularApi.ts`,
  `frontend/src/services/core/httpClient.ts`
- Store synchronization:
  `frontend/src/store/hooks.ts`
- E2E regression example:
  `frontend/e2e/auth.spec.ts`

## Forbidden / Discouraged Patterns

- Do not add fresh business logic to deprecated compatibility service layers
  when module-specific surfaces already exist.
- Do not bypass typed/shared APIs with ad hoc `fetch` in pages.
- Do not ship route-visible UI changes without checking lazy-loading and route
  registration consistency.

## Async Effect and Real E2E Contracts

### React StrictMode mounted guards

React development StrictMode may replay an effect as setup → cleanup → setup. Any async effect that uses a
mutable mounted guard must restore the guard at the beginning of every setup, not only initialize the ref once.

```tsx
// Wrong: cleanup from the StrictMode probe permanently disables the next setup.
const mountedRef = useRef(true);
useEffect(() => () => {
  mountedRef.current = false;
}, []);

// Correct: every setup owns an active lifecycle; final cleanup rejects late work.
const mountedRef = useRef(true);
useEffect(() => {
  mountedRef.current = true;
  return () => {
    mountedRef.current = false;
    requestIdRef.current += 1;
  };
}, []);
```

Required assertions:

- StrictMode setup/cleanup replay must still allow the active OAuth/auth request to complete.
- A real unmount must reject late responses and prevent navigation or state updates.
- Targeted Playwright auth coverage must reach the post-callback UI, not stop at a loading-state assertion.

Reference: `frontend/src/pages/AuthCallback.tsx` and `frontend/e2e/auth.spec.ts`.

### Stable route ownership in Playwright

`src/app/router/AppRouter.tsx` is the route registry. E2E helpers must not use a removed, unregistered, or
page-layout-specific route as a navigation trampoline to reach the behavior under test.

```tsx
// Wrong: depends on an unrelated historical page and its current link layout.
await page.goto(`/project/${projectId}/sponsor`);
await page.locator(`a[href="/project/${projectId}/${subPath}"]`).click();

// Correct: enter the registered target route and assert the route contract.
await page.goto(`/project/${projectId}/${subPath}`);
await expect(page).toHaveURL(
  new RegExp(`/project/${projectId}/${subPath}$`),
);
```

Validation matrix:

| Case | Required result |
|---|---|
| Target route is registered | Direct navigation reaches the real page and business request assertions run |
| Target route is renamed/removed | Router and E2E contract must be updated together; do not hide drift with a fallback page |
| Navigation layout changes | Business smoke remains stable unless navigation itself is the behavior under test |
| Helper points to an unregistered route | Test must fail during review; never accept it as a valid setup path |

Reference: `frontend/e2e/helpers/backgroundTaskSmoke.ts` and
`frontend/src/app/router/AppRouter.tsx`.
