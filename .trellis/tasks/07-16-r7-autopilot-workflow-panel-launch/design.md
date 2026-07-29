# Technical Design: R7 Autopilot Workflow Panel Launch

## Boundary

```text
ProjectWorkflowStatePanel
  -> select existing allowed target phase
  -> explicit Autopilot confirmation modal
  -> typed backgroundTaskApi.createConfirmedAutopilotWorkflowTransition
  -> POST /projects/:project_id/autopilot/actions
  -> existing syncBackgroundTaskToStore
  -> OPEN_BACKGROUND_TASK_CENTER_EVENT
  -> existing BackgroundTaskCenter poll/cancel/result projection
```

The frontend is a launch-and-observe surface only. The server remains the
authority for actor identity, project access, scope injection, confirmation
validation, allowlist, workflow transition, terminal state, and result receipt.

## Frontend Contract

Add a typed request shape alongside the generic background-task types:

```ts
interface ConfirmedAutopilotWorkflowTransitionRequest {
  tool_name: 'transition_project_workflow';
  arguments: {
    expected_phase: NovelWorkflowPhase;
    target_phase: NovelWorkflowPhase;
    reason?: string;
    related_task_id?: string;
  };
  confirmed_by_user: true;
}
```

The API method belongs in `backgroundTaskApi`, because its response is a generic
background-task resource and it must reuse the module's existing store-sync
owner. It posts to the project-scoped path and returns `BackgroundTaskStatus`.

## UI Design

Keep the current direct dropdown intact. Add a second small dropdown/button
labelled `后台受控切换` that lists exactly `state.allowed_transitions`. Selecting
a target opens a dedicated modal for the background path, even for forward
transitions that do not require the existing direct-flow confirmation.

The modal displays current/target labels, explains that the operation is queued
as a controllable background task, accepts an optional 500-character reason,
and has a clear submit label. On submit:

1. capture the current state phase as `expected_phase`;
2. call the typed API with `confirmed_by_user: true`;
3. show a success message with the created task ID;
4. close/reset only after the request succeeds;
5. dispatch `OPEN_BACKGROUND_TASK_CENTER_EVENT`;
6. leave canonical workflow state unchanged locally, because task execution can
   fail due to a stale expected phase or another server-side conflict.

On failure, retain the modal and reason so the user can read the error and retry
or cancel. No front-end retry loop is introduced.

## Compatibility and Safety

- Existing direct transitions retain their current `transition()` hook and
  confirmation/rollback handling.
- The background request derives target choices from canonical state
  `allowed_transitions`; it never accepts a manually entered phase string.
- No project/user scope field is exposed in the request body.
- The task center continues to own task visibility, polling, generic cancellation
  and terminal presentation. The panel only emits the established open event.
- `novel_autopilot` remains non-resumable. The UI must not display Resume/Pause/
  Steer controls or imply that a completed/failed transition is automatically
  recoverable.

## Testing Strategy

- Add pure helper tests where practical for request construction / allowed target
  handling without introducing a new runtime state layer.
- Validate TypeScript with the repository build, then run lint and full frontend
  build.
- Run focused Rust Autopilot API tests to prove the UI's backend contract remains
  available.

## Rollback

Remove the typed client and the separate background launch UI path together. The
existing direct transition UI, backend endpoint, TaskRegistry, Coordinator and
workflow service remain untouched.
