import { create } from 'zustand';
import { persist } from 'zustand/middleware';
import { compactTasks, mergeTrackedBackgroundTask, type ActiveTaskScope, type TrackedBackgroundTask, type UpsertTaskPayload } from './backgroundTaskModel';
import {
  clearTerminalBackgroundTasks,
  pruneBackgroundTasksByProjectIds,
  pruneMissingActiveBackgroundTasks,
  removeBackgroundTask,
  removeBackgroundTasksByProjectId,
} from './backgroundTaskStateHelpers';

const areStringArraysEqual = (left?: Array<Record<string, unknown>>, right?: Array<Record<string, unknown>>) => {
  const leftSerialized = JSON.stringify(left ?? []);
  const rightSerialized = JSON.stringify(right ?? []);
  return leftSerialized === rightSerialized;
};

const areCheckpointsEqual = (
  left?: Record<string, unknown> | null,
  right?: Record<string, unknown> | null,
) => JSON.stringify(left ?? null) === JSON.stringify(right ?? null);

const isSameTrackedTask = (
  left?: TrackedBackgroundTask,
  right?: TrackedBackgroundTask,
) => {
  if (!left || !right) {
    return false;
  }

  return (
    left.taskId === right.taskId
    && left.taskType === right.taskType
    && left.projectId === right.projectId
    && left.status === right.status
    && left.progress === right.progress
    && left.message === right.message
    && JSON.stringify(left.result ?? null) === JSON.stringify(right.result ?? null)
    && left.error === right.error
    && left.stageCode === right.stageCode
    && left.executionMode === right.executionMode
    && left.workflowScope === right.workflowScope
    && areCheckpointsEqual(left.checkpoint, right.checkpoint)
    && areStringArraysEqual(left.failedChapters, right.failedChapters)
    && JSON.stringify(left.activeStoryRepairPayload ?? null) === JSON.stringify(right.activeStoryRepairPayload ?? null)
    && left.terminalReason === right.terminalReason
    && left.terminalLabel === right.terminalLabel
    && left.reviewRequired === right.reviewRequired
    && left.canResume === right.canResume
    && left.createdAt === right.createdAt
    && left.updatedAt === right.updatedAt
    && left.completedAt === right.completedAt
  );
};

const isSameTaskMap = (
  left: Record<string, TrackedBackgroundTask>,
  right: Record<string, TrackedBackgroundTask>,
) => {
  const leftKeys = Object.keys(left);
  const rightKeys = Object.keys(right);
  if (leftKeys.length !== rightKeys.length) {
    return false;
  }

  return leftKeys.every((taskId) => isSameTrackedTask(left[taskId], right[taskId]));
};

export type { BackgroundTaskRuntimeStatus } from '../services/modules/backgroundTaskTypes';
export type { ActiveTaskScope, TrackedBackgroundTask } from './backgroundTaskModel';
export { getTaskTypeLabel, isActiveBackgroundTask } from './backgroundTaskModel';

interface BackgroundTaskState {
  tasks: Record<string, TrackedBackgroundTask>;
  upsertTask: (task: UpsertTaskPayload) => void;
  removeTask: (taskId: string) => void;
  removeTasksByProjectId: (projectId: string) => void;
  pruneTasksByProjectIds: (projectIds: string[]) => void;
  pruneMissingActiveTasks: (activeTaskIds: string[], scope?: ActiveTaskScope) => void;
  clearTerminalTasks: () => void;
  pruneExpiredTerminalTasks: () => void;
}

export const useBackgroundTaskStore = create<BackgroundTaskState>()(
  persist(
    (set, get) => ({
      tasks: {},
      upsertTask: (task) => {
        if (!task.task_id) return;

        const existing = get().tasks[task.task_id];
        const merged = mergeTrackedBackgroundTask(task, existing);
        if (isSameTrackedTask(existing, merged)) {
          return;
        }
        const nextTasks = { ...get().tasks, [task.task_id]: merged };
        const compacted = compactTasks(nextTasks);
        if (isSameTaskMap(get().tasks, compacted)) {
          return;
        }
        set({ tasks: compacted });
      },
      removeTask: (taskId) => {
        set({ tasks: removeBackgroundTask(get().tasks, taskId) });
      },
      removeTasksByProjectId: (projectId) => {
        set({ tasks: removeBackgroundTasksByProjectId(get().tasks, projectId) });
      },
      pruneTasksByProjectIds: (projectIds) => {
        set({ tasks: pruneBackgroundTasksByProjectIds(get().tasks, projectIds) });
      },
      pruneMissingActiveTasks: (activeTaskIds, scope) => {
        set({ tasks: pruneMissingActiveBackgroundTasks(get().tasks, activeTaskIds, scope) });
      },
      clearTerminalTasks: () => {
        set({ tasks: clearTerminalBackgroundTasks(get().tasks) });
      },
      pruneExpiredTerminalTasks: () => {
        set({ tasks: compactTasks(get().tasks) });
      },
    }),
    {
      name: 'background-task-store',
      partialize: (state) => ({ tasks: state.tasks }),
      onRehydrateStorage: () => (state) => {
        state?.pruneExpiredTerminalTasks();
      },
    }
  )
);
