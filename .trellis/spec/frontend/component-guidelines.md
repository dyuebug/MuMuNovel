# Component Guidelines

> Component patterns, props, and composition rules.

---

## Overview

Frontend components in this repository range from simple shared UI pieces to
large business workflow panels. The main rule is to recognize which kind of
component you are editing before you extend it.

---

## Component Categories

- **App shell / global components**:
  `ProtectedRoute.tsx`, `BackgroundTaskCenter.tsx`, `ThemeSwitch.tsx`
- **Heavy workflow components**:
  `ChapterBatchGenerateModal.tsx`,
  `ChapterRegenerationModal.tsx`,
  `GenerationExecutionSettings.tsx`
- **Domain display components**:
  relationship graph canvases, reader panels, cards, preview content
- **Utility-style UI fragments**:
  `storyCreation*` and other render-helper style component files

Treat these categories differently during review. The repository already uses
`components/` as a mixed business/UI layer, not a pure atomic component
library.

---

## Composition Rules

- Pages should compose components; components should not assume route-level
  ownership unless they are intentionally app-shell components.
- If multiple pages need the same business settings or panel behavior, extract
  the shared piece into a reusable component or hook rather than cloning UI.
- Prefer splitting large components by workflow step or visual sub-block when
  that split matches the domain.
- For expensive or low-frequency panels, lazy loading is normal and preferred.

---

## Props and State

- Keep public props semantic and domain-oriented.
- When component-local async state or lifecycle behavior becomes reusable,
  promote it into a hook like `useGenerationExecutionSettings()`.
- Components that coordinate long-running tasks often depend on store or
  services directly; if you add more state, be explicit about ownership and
  cleanup.
- Avoid pushing raw API records through many local casts. Prefer typed data
  from `src/types/` or service-layer helpers.

---

## Reuse Expectations

- Reuse existing workflow panels when the behavior is already present:
  generation settings, SSE progress UI, task-center UI, story quality helpers.
- Before introducing a helper or shared prop shape, search for an existing
  equivalent. This repo already has repeated-generation and task-state patterns.
- If a component is used across pages, do not let one page-specific shortcut
  leak into the shared contract unless all consumers want it.

---

## Review Checklist

- Is this really a component concern, or should the logic live in a hook,
  service, feature workflow, or store?
- If the component touches background tasks, SSE, or project hydration, did
  you check the upstream/downstream consumers?
- If the component is lazily loaded today, did your change preserve that
  loading strategy?
- Did you reuse an existing shared panel or helper instead of cloning logic?

---

## Examples

- Global shell component:
  `frontend/src/components/BackgroundTaskCenter.tsx`
- Reusable workflow settings component + hook:
  `frontend/src/components/GenerationExecutionSettings.tsx`
- Route guard component:
  `frontend/src/components/ProtectedRoute.tsx`

## Anti-Patterns

- Do not treat high-coupling workflow components as if they were stateless
  presentational widgets.
- Do not duplicate generation/task UI patterns that already exist in
  `components/`.
- Do not bury large amounts of route or global orchestration logic in a random
  shared component without documenting the boundary.
