# R3 Design: Novel Workflow State Machine

## 1. Architecture Boundary

```text
PostgreSQL projects.status
        |
        v
NovelWorkflowService (single phase owner)
   |       |          |           |
   |       |          |           +-- project/book import canonicalization
   |       |          +-------------- wizard complete / internal reset
   |       +------------------------- legacy Project PUT adapter
   +--------------------------------- GET/POST workflow API
        |
        v
frontend project workflow feature
        |
        +-- local remote-state view (phase + allowed transitions)
        +-- Zustand Project.status cache synchronization
```

`NovelWorkflowService` owns phase parsing, legacy normalization, transition validation, conditional persistence,
state projection and structured audit emission. API/UI layers cannot duplicate the transition table.

## 2. Backend Modules

### `backend-rs/src/services/novel_workflow_service.rs`

Define:

- `NOVEL_WORKFLOW_SCHEMA_VERSION: u32 = 1`
- `NovelWorkflowPhase`
- legacy storage/input parsing and canonical serialization
- `NovelWorkflowStateView`
- `NovelWorkflowTransitionReceipt`
- `NovelWorkflowError`
- `allowed_transitions(phase)` and `suggested_next_phase(phase)`
- `get_state(db, project_id, user_id)`
- `transition(db, project_id, user_id, expected, target, audit_context)`
- internal helpers for wizard completion/reset and import canonicalization

The model remains a string column for compatibility; all new writes use canonical snake_case values.

### Conditional Update

A state-changing transition performs a single conditional update equivalent to:

```sql
UPDATE projects
SET status = :target, updated_at = :now
WHERE id = :project_id
  AND user_id = :user_id
  AND status IN (:canonical_expected, :legacy_aliases_for_expected)
```

If `rows_affected == 0`, re-read by `id + user_id`:

1. no row → 404;
2. row with a different normalized phase → stale expected phase conflict;
3. row with unknown status → data contract conflict;
4. otherwise treat as internal persistence failure.

Same-phase requests return a no-op receipt after ownership and expected-phase validation.

### Legacy PUT Adapter

`ProjectService::update` keeps its existing metadata update behavior. If `status` is present:

1. parse legacy/canonical input through `NovelWorkflowPhase`;
2. determine the current normalized phase;
3. validate with the same public transition table;
4. apply the phase through the service owner rather than direct string assignment.

Metadata-only updates remain unchanged. Unknown and illegal status values become domain errors mapped to 4xx.

### Wizard and Import

- Normal project creation writes `foundation`.
- Full wizard creation writes `foundation` while `wizard_status/step` retain their separate meaning.
- Wizard completion uses the internal service transition to `writing`; it does not consult background task status.
- Wizard cleanup uses a documented internal reset to `foundation`; the reset is not exposed in
  `allowed_transitions`.
- Project import normalizes known legacy aliases and rejects unknown project phase values before insert.
- Book import enters `writing` through the same internal owner instead of a direct string write.

## 3. API Contract

### GET `/api/projects/{project_id}/workflow-state`

Response:

```json
{
  "schema_version": 1,
  "project_id": "...",
  "phase": "writing",
  "allowed_transitions": ["outline", "reviewing", "completed"],
  "can_rollback": true,
  "suggested_next_phase": "reviewing",
  "updated_at": "2026-07-14T12:00:00",
  "source": "projects.status"
}
```

### POST `/api/projects/{project_id}/workflow-state/transition`

Request:

```json
{
  "target_phase": "reviewing",
  "expected_phase": "writing",
  "reason": "进入人工审校",
  "related_task_id": null
}
```

Response:

```json
{
  "schema_version": 1,
  "changed": true,
  "previous_phase": "writing",
  "state": {
    "schema_version": 1,
    "project_id": "...",
    "phase": "reviewing",
    "allowed_transitions": ["writing", "polishing", "completed"],
    "can_rollback": true,
    "suggested_next_phase": "polishing",
    "updated_at": "2026-07-14T12:00:01",
    "source": "projects.status"
  }
}
```

Error mapping:

- invalid JSON enum/field → 400/422 according to current Axum extractor behavior;
- illegal transition → 409;
- stale expected phase → 409 with actual phase in safe error detail;
- unknown persisted phase → 409 data contract conflict;
- missing/foreign project → 404;
- database/internal error → 500.

## 4. Audit Contract

A successful changed transition emits one structured event named
`novel_workflow_phase_transition` with bounded fields. `reason` is trimmed, control characters are removed,
and length is capped. Empty values become null. Raw prompts, API keys and background-task checkpoints are never logged.

This event is operational audit evidence only. R3 deliberately does not claim a queryable historical ledger.
A future ledger requires a separately approved migration and must consume the same transition owner.

## 5. Frontend Design

Add a focused feature module under `frontend/src/features/projects/workflow/`:

- phase presentation metadata (label/color/description only; no transition rules)
- `useProjectWorkflowState` hook for GET/POST, pending state and conflict refresh
- `ProjectWorkflowStatePanel` component

Integrate the panel into the persistent project-detail summary area. The component receives `projectId`, reads
its own workflow view, and calls existing Zustand `updateProject` after a successful transition. It must not add
workflow state to the background-task store or create another global source.

The UI renders only server-provided transitions. Completed rollback/high-impact actions require confirmation;
normal adjacent progress uses a direct action. Mobile layout wraps instead of modifying the main header geometry.

## 6. Compatibility and Rollback

- No database migration or bulk rewrite is required.
- Removing the new routes/component reverts user-visible R3 behavior while existing `projects.status` data remains valid.
- Canonical values are strings; older clients may display an unknown label but Project CRUD transport remains valid.
- Legacy four-state inputs remain accepted as aliases.
- If frontend integration must be rolled back, GET/POST APIs remain independently usable.
- Do not edit or discard unrelated dirty worktree changes; use focused new files and minimal hunks.

## 7. Security and Reliability

- Identity comes only from authenticated Claims.
- Conditional update prevents concurrent last-write-wins.
- Reason/task identifiers are bounded and sanitized before logging.
- Unknown persisted values fail visibly instead of being guessed.
- Background task lifecycle cannot change project phase.
- No sensitive checkpoint or prompt data enters the state response or audit event.
