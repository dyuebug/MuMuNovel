import type { TrackedBackgroundTask } from '../../../store/backgroundTasks';
import {
  selectCurrentProjectActiveTaskCount,
  selectFailedBackgroundTaskCount,
  selectRecoverableBackgroundTaskCount,
  selectTerminalBackgroundTaskCount,
} from './selectors';

export type BackgroundTaskCenterSummary = {
  currentProjectActiveCount: number;
  terminalTaskCount: number;
  failedTaskCount: number;
  recoverableTaskCount: number;
  otherActiveCount: number;
};

export const buildBackgroundTaskCenterSummary = (params: {
  tasks: TrackedBackgroundTask[];
  activeTasks: TrackedBackgroundTask[];
  focusProjectId: string | null;
  isTaskResumable: (task: TrackedBackgroundTask) => boolean;
}): BackgroundTaskCenterSummary => {
  const { tasks, activeTasks, focusProjectId, isTaskResumable } = params;
  const currentProjectActiveCount = selectCurrentProjectActiveTaskCount(tasks, focusProjectId);
  const terminalTaskCount = selectTerminalBackgroundTaskCount(tasks);
  const otherActiveCount = Math.max(0, activeTasks.length - currentProjectActiveCount);
  const failedTaskCount = selectFailedBackgroundTaskCount(tasks);
  const recoverableTaskCount = selectRecoverableBackgroundTaskCount(tasks, isTaskResumable);

  return {
    currentProjectActiveCount,
    terminalTaskCount,
    failedTaskCount,
    recoverableTaskCount,
    otherActiveCount,
  };
};
