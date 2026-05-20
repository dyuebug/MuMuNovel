# Type Safety

> Type patterns, API contracts, and validation rules.

---

## Overview

This frontend relies on shared TypeScript interfaces in `src/types/` plus
typed service/module exports. The codebase favors explicit domain interfaces
and typed API surfaces over ad hoc object usage.

---

## Shared Types

- Shared domain and API-facing types live in `src/types/index.ts`.
- Keep commonly reused shapes centralized there instead of redefining them in
  each page.
- Use literal unions for constrained domain values when the backend contract is
  stable, for example project status or fallback strategy.
- If a backend schema change affects widely shared payloads, update shared
  types first and then trace consumers.

---

## Service Contracts

- Prefer typed exports from `src/services/modularApi.ts` and
  `src/services/modules/*`.
- Add explicit response/request types when a service surface is reused across
  pages or stores.
- Keep compatibility exports in `api.ts` thin; do not let it become a new home
  for untyped or duplicated runtime code.

---

## UI-Level Normalization

- Small normalization helpers are acceptable at the UI boundary when dealing
  with loose remote payloads.
- Example: `normalizeModelOptions()` in
  `GenerationExecutionSettings.tsx` safely turns unknown model payloads into a
  typed `ModelOption[]`.
- Prefer narrow normalization functions over scattered inline casts.

---

## Review Checklist

- Did you update shared types when the backend contract changed?
- Are you reusing an existing interface instead of redefining a near-copy?
- If the payload is loosely typed, did you normalize it once instead of
  casting it everywhere?
- If a union or enum-like field changed, did you check store, page, and
  service consumers together?

---

## Examples

- Shared domain types:
  `frontend/src/types/index.ts`
- Typed service aggregator:
  `frontend/src/services/modularApi.ts`
- Local normalization helper:
  `frontend/src/components/GenerationExecutionSettings.tsx`

## Anti-Patterns

- Do not spread `any` or repeated `Record<string, unknown>` casts across pages.
- Do not keep stale frontend type unions after backend schemas evolve.
- Do not introduce a second competing contract definition when a shared type
  already exists.
