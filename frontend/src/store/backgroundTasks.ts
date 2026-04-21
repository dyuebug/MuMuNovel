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
        const nextTasks = { ...get().tasks, [task.task_id]: merged };
        set({ tasks: compactTasks(nextTasks) });
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