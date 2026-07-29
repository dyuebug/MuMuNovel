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
- Project nested pages rendered inside `ProjectDetail` should let the outlet
  scroll container see their real content height. Use `minHeight: '100%'` and
  avoid root-level `overflow: 'hidden'` unless the page is intentionally a
  pure fixed-canvas viewport with no lower panels. Otherwise header cards,
  graph canvases, or detail panels can consume the available height and hide
  the lower work area.
- Optional project-wide diagnostics such as runtime metrics must not be mounted
  as a persistent flex sibling between the project header and nested-page
  outlet. Keep only a compact trigger in the existing header and render the
  full panel in an Ant Design `Drawer`/`Modal` overlay. The regression test
  must assert the outlet container height is unchanged before and after the
  overlay opens, and that expensive data is requested only after activation.
- Non-blocking task progress cards must sit below Ant Design drawers/modals
  and collapse before dispatching `OPEN_BACKGROUND_TASK_CENTER_EVENT`.
  The global task-center float button should be hidden while its drawer is
  open, because it otherwise competes with drawer content at high `z-index`.
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
- Do not combine `height: '100%'` with root-level `overflow: 'hidden'` on
  ordinary `ProjectDetail` child pages that also render multiple header/guide
  cards above the main work area. Prefer natural page height plus scoped inner
  scrolling for tables, tabs, or canvas panels.
- Do not leave non-blocking SSE/task floating cards above the task center
  drawer after the user clicks "view all tasks"; collapse them and keep their
  `z-index` lower than drawer layers.

## Diagnostic Table Layout

- Give the primary identity column in workflow/diagnostic tables an explicit
  minimum width. Secondary identifiers such as step keys should use single-line
  ellipsis with a tooltip instead of increasing the row height.
- Set `scroll.x` from the actual column-width budget rather than an arbitrary
  oversized value, and use `tableLayout="fixed"` when diagnostic columns have
  known widths. At desktop widths, regression tests should assert that the
  table viewport has no unnecessary horizontal overflow.
- Keep timestamps and short status columns compact, but do not hide failure
  codes or quality decisions merely to avoid scrolling. On narrow viewports,
  scoped table scrolling is preferable to shrinking the primary label into an
  unreadable column.

- Render domain enums with a typed, centralized user-facing label map instead
  of exposing machine values such as `auto_repair` or `manual_review` directly.
  Use compact, non-wrapping tags for decisions/statuses and remove the tag's
  trailing margin when the cell is centered.
- Keep diagnostic identifiers such as error codes in their original form. Give
  them an explicit left-aligned bounded column, render them on one line with
  ellipsis, and expose the complete value through a tooltip when truncated.
  Render missing identifiers through the same typography boundary as `—` so
  populated and empty cells retain a consistent alignment contract.
- When a stable machine identifier repeats information already represented by
  a dedicated structured column, avoid rendering it as a permanent second line.
  For example, a chapter row should show `章节分析` plus the independent `第1章`
  column, while `chapter:0001:analyze` remains available from a tooltip on the
  step label. Keep identifiers visible for non-chapter rows when they are still
  needed to distinguish otherwise identical planning or completion steps.

```tsx
const DECISION_META: Record<Decision, { label: string; color: string }> = {
  accept: { label: '通过', color: 'success' },
  manual_review: { label: '人工复核', color: 'gold' },
};

<Tag style={{ marginInlineEnd: 0, whiteSpace: 'nowrap' }}>
  {DECISION_META[value].label}
</Tag>
```
