import {
  ACTIVE_TASK_GRACE_MS,
  TERMINAL_STATUSES,
  matchesActiveTaskScope,
  type ActiveTaskScope,
  type TrackedBackgroundTask,
} from './backgroundTaskModel';

export type BackgroundTaskMap = Record<string, TrackedBackgroundTask>;

export const removeBackgroundTask = (
  tasks: BackgroundTaskMap,
  taskId: string,
): BackgroundTaskMap => {
  if (!taskId || !(taskId in tasks)) {
    return tasks;
  }

  const next = { ...tasks };
  delete next[taskId];
  return next;
};

export const removeBackgroundTasksByProjectId = (
  tasks: BackgroundTaskMap,
  projectId: string,
): BackgroundTaskMap => {
  if (!projectId) {
    return tasks;
  }

  return Object.fromEntries(
    Object.entries(tasks).filter(([, task]) => task.projectId !== projectId)
  );
};

export const pruneBackgroundTasksByProjectIds = (
  tasks: BackgroundTaskMap,
  projectIds: string[],
): BackgroundTaskMap => {
  const allowed = new Set(projectIds);
  return Object.fromEntries(
    Object.entries(tasks).filter(([, task]) => !task.projectId || allowed.has(task.projectId))
  );
};

export const pruneMissingActiveBackgroundTasks = (
  tasks: BackgroundTaskMap,
  activeTaskIds: string[],
  scope?: ActiveTaskScope,
  now = Date.now(),
): BackgroundTaskMap => {
  const activeSet = new Set(activeTaskIds);
  return Object.fromEntries(
    Object.entries(tasks).filter(([, task]) => {
      if (TERMINAL_STATUSES.includes(task.status)) {
        return true;
      }
      if (!matchesActiveTaskScope(task, scope)) {
        return true;
      }
      if (activeSet.has(task.taskId)) {
        return true;
      }
      if (now - task.updatedAt <= ACTIVE_TASK_GRACE_MS) {
        return true;
      }
      return false;
    })
  );
};

export const clearTerminalBackgroundTasks = (
  tasks: BackgroundTaskMap,
): BackgroundTaskMap =>
  Object.fromEntries(
    Object.entries(tasks).filter(([, task]) => !TERMINAL_STATUSES.includes(task.status))
  );