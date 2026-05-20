# State Management

> Local state, global state, and server-state guidance.

---

## Overview

The frontend already has a three-way split for runtime state:

- local component/page state for transient UI concerns
- Zustand entity cache in `useStore`
- persisted long-task runtime state in `useBackgroundTaskStore`

A lightweight event bus exists for a small number of cross-view notifications.

---

## Global State Patterns

- `src/store/index.ts` (`useStore`) holds current project context plus cached
  entities like projects, outlines, characters, chapters, and current chapter.
- `src/store/backgroundTasks.ts` (`useBackgroundTaskStore`) is the task runtime
  store and uses persistence because task recovery matters across reloads.
- Use semantic setter/update methods instead of mutating arrays ad hoc in
  consuming components.
- Preserve reference-stability optimizations when extending entity caches.

---

## Local State vs Store

- Keep page layout toggles, drawer visibility, temporary form state, and
  component-only async state local unless multiple screens or reload recovery
  need it.
- Use Zustand when the data is shared across pages/components or is part of the
  app's current working context.
- Use the background-task store for resumable generation/task flows rather than
  inventing parallel task trackers.

---

## Store / Service Boundary

- Services perform I/O; stores hold client runtime state.
- `src/store/hooks.ts` and feature hooks are the preferred bridge between API
  modules and store updates.
- Avoid scattering direct API-to-store writes across unrelated pages.
- If a state field is persisted, review pruning and cleanup behavior before
  extending it.

---

## Event Bus Usage

- `src/store/eventBus.ts` is a narrow escape hatch, not the main state system.
- Use it for low-frequency, non-persistent view-switch notifications.
- If the data must survive navigation or refresh, it belongs in Zustand, not
  in the event bus.

---

## Review Checklist

- Is this state local, shared, or resumable/persisted?
- Should the update flow go through store hooks or feature helpers instead of
  direct page logic?
- If you changed a store field, did you inspect all major consumers?
- If you changed background task state, did you verify cleanup, persistence,
  and task-center UI impact?

---

## Examples

- Entity cache store:
  `frontend/src/store/index.ts`
- Persisted task runtime store:
  `frontend/src/store/backgroundTasks.ts`
- Store/service bridge:
  `frontend/src/store/hooks.ts`
- Local page state with route/layout concerns:
  `frontend/src/pages/ProjectDetail.tsx`

## Anti-Patterns

- Do not use the event bus as a replacement for durable app state.
- Do not create duplicate task/runtime state outside
  `useBackgroundTaskStore`.
- Do not bypass existing sync hooks and then re-implement the same caching
  rules in pages.
