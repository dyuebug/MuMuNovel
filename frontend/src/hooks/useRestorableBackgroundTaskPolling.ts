import { useCallback, useEffect, useRef } from 'react';

import { backgroundTaskApi, type BackgroundTaskStatus } from '../services/api';
import type { TrackedBackgroundTask } from '../store/backgroundTasks';
import {
  startBackgroundTaskPolling,
  type PollableBackgroundTask,
} from '../utils/taskPolling';

type RestoreTaskPayload = {
  taskId: string;
  progress?: number | null;
  message?: string | null;
};

type UseRestorableBackgroundTaskPollingOptions<TTask extends PollableBackgroundTask = BackgroundTaskStatus> = {
  projectId?: string | null;
  activeTrackedTask?: TrackedBackgroundTask | null;
  canRestore?: boolean;
  restoreListLimit?: number;
  isMatchingTask: (task: BackgroundTaskStatus) => boolean;
  onRestoreTask: (payload: RestoreTaskPayload) => void;
  createPollingOptions: (taskId: string) => {
    pollTask: (currentPollingTaskId: string) => Promise<TTask>;
    onTask?: (task: TTask) => void;
    onCompleted?: (task: TTask) => void;
    onFailed?: (task: TTask) => void;
    onCancelled?: (task: TTask) => void;
    onPollingError?: (error: unknown) => void;
  };
};

export const useRestorableBackgroundTaskPolling = <TTask extends PollableBackgroundTask = BackgroundTaskStatus>({
  projectId,
  activeTrackedTask = null,
  canRestore = true,
  restoreListLimit = 20,
  isMatchingTask,
  onRestoreTask,
  createPollingOptions,
}: UseRestorableBackgroundTaskPollingOptions<TTask>) => {
  const taskPollStopRef = useRef<(() => void) | null>(null);
  const currentTaskIdRef = useRef<string | null>(null);

  const stopTaskPolling = useCallback(() => {
    taskPollStopRef.current?.();
    taskPollStopRef.current = null;
  }, []);

  const startTaskPolling = useCallback((taskId: string) => {
    stopTaskPolling();
    currentTaskIdRef.current = taskId;
    taskPollStopRef.current = startBackgroundTaskPolling(taskId, createPollingOptions(taskId));
  }, [createPollingOptions, stopTaskPolling]);

  useEffect(() => {
    return () => {
      stopTaskPolling();
      currentTaskIdRef.current = null;
    };
  }, [stopTaskPolling]);

  useEffect(() => {
    if (!projectId || currentTaskIdRef.current || taskPollStopRef.current || !canRestore) {
      return;
    }

    let disposed = false;

    const restoreLocalTask = (payload: RestoreTaskPayload) => {
      if (currentTaskIdRef.current === payload.taskId || taskPollStopRef.current) {
        return;
      }

      onRestoreTask(payload);
      startTaskPolling(payload.taskId);
    };

    const restoreTaskPolling = async () => {
      try {
        const { items } = await backgroundTaskApi.listTasks({
          project_id: projectId,
          active_only: true,
          limit: restoreListLimit,
        });

        if (disposed) {
          return;
        }

        const activeTask = [...(items || [])]
          .filter(isMatchingTask)
          .sort(
            (left, right) =>
              new Date(right.updated_at || right.created_at || 0).getTime()
              - new Date(left.updated_at || left.created_at || 0).getTime(),
          )[0];

        if (activeTask) {
          restoreLocalTask({
            taskId: activeTask.task_id,
            progress: activeTask.progress,
            message: activeTask.message,
          });
          return;
        }
      } catch (error) {
        console.error('恢复后台任务失败:', error);
      }

      if (!disposed && activeTrackedTask) {
        restoreLocalTask({
          taskId: activeTrackedTask.taskId,
          progress: activeTrackedTask.progress,
          message: activeTrackedTask.message,
        });
      }
    };

    void restoreTaskPolling();

    return () => {
      disposed = true;
    };
  }, [
    activeTrackedTask,
    canRestore,
    isMatchingTask,
    onRestoreTask,
    projectId,
    restoreListLimit,
    startTaskPolling,
  ]);

  return {
    currentTaskIdRef,
    startTaskPolling,
    stopTaskPolling,
  };
};
