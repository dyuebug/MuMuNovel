# Hook Guidelines

> Custom hooks, data fetching patterns, and hook design rules.

---

## Overview

Hooks in this repository are used for three main purposes:

- state synchronization between API/features and Zustand stores
- reusable workflow state for complex UI panels
- page/runtime lifecycle helpers

The repo is moving away from dumping every async action into pages and toward
feature or store hook modules.

---

## Hook Placement

- Put generic app hooks in `src/hooks/` when they are not strongly tied to one
  domain.
- Put store synchronization hooks in `src/store/hooks.ts` or nearby store
  helpers when the main purpose is "fetch and write into Zustand".
- Put domain-specific query/command/workflow hooks under
  `src/features/<domain>/`.
- Put component-local reusable workflow hooks next to the owning component if
  the hook is only meaningful with that component contract.

---

## Hook Design Rules

- Expose semantic operations, not low-level HTTP details.
- Prefer wrapping feature commands/queries and returning stable action bundles,
  as `useProjectSync()`, `useCharacterSync()`, `useOutlineSync()`, and
  `useChapterSync()` already do.
- Guard async lifecycle carefully with refs, request ids, or abort semantics
  when a hook owns in-flight UI state.
- Keep hooks typed. Inputs and outputs should map cleanly to shared types or
  well-defined view-model structures.

---

## Data Fetching and Sync

- Store sync is an explicit layer in this project. Do not reimplement the same
  "fetch → write store → refresh freshness markers" flow in every page.
- Prefer using feature query/command modules and store sync hooks instead of
  calling service methods directly in many pages.
- For long-running workflows, preserve cancellation and stale-request handling.
- Examples:
  `frontend/src/store/hooks.ts`,
  `frontend/src/components/GenerationExecutionSettings.tsx`.

---

## Review Checklist

- Should this logic live in a feature hook, store hook, or component-local hook?
- Does the hook hide transport details and expose a useful semantic API?
- If async, does it prevent stale updates on unmount or request replacement?
- Is there already a similar refresh/query/workflow hook you should extend?

---

## Examples

- Store sync façade hooks:
  `frontend/src/store/hooks.ts`
- Feature query/command usage:
  `frontend/src/features/projects/`,
  `frontend/src/features/chapters/`
- Component-owned workflow hook:
  `frontend/src/components/GenerationExecutionSettings.tsx`

## Anti-Patterns

- Do not duplicate fetch-and-sync logic across many pages.
- Do not return untyped `any` blobs when shared types already exist.
- Do not ignore mounted-state / cancellation issues in hooks that fetch or
  coordinate async workflows.
