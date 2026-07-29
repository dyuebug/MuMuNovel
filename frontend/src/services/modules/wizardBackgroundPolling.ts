import type { SSEClientOptions } from '../../utils/sseClient';
import { backgroundTaskApi, type BackgroundTaskStatus } from './backgroundTasks';

export const runBackgroundTaskWithPolling = async <T>(
  taskType: BackgroundTaskStatus['task_type'],
  projectId: string | undefined,
  payload: Record<string, unknown>,
  options?: SSEClientOptions<T>,
): Promise<T> => {
  const createPayload: Parameters<typeof backgroundTaskApi.createTask>[0] = {
    task_type: taskType,
    payload,
  };
  if (projectId) {
    createPayload.project_id = projectId;
  }

  const createdTask = await backgroundTaskApi.createTask({
    ...createPayload,
  });

  options?.onTaskCreated?.(createdTask.task_id);
  options?.onProgress?.('Background task created', 0, 'processing');

  const stopTaskStream = (options?.onChunk || options?.onReasoningChunk)
    ? backgroundTaskApi.subscribeTaskStream(createdTask.task_id, {
        onChunk: options.onChunk,
        onReasoningChunk: options.onReasoningChunk,
      })
    : null;

  return new Promise<T>((resolve, reject) => {
    let timer: number | null = null;

    const stopPolling = () => {
      if (timer !== null) {
        window.clearInterval(timer);
        timer = null;
      }
      stopTaskStream?.();
    };

    const poll = async () => {
      try {
        const task = await backgroundTaskApi.getTaskStatus(createdTask.task_id);
        options?.onProgress?.(task.message || '', task.progress || 0, task.status);

        if (task.status === 'completed') {
          stopPolling();
          if (task.result !== undefined && task.result !== null) {
            options?.onResult?.(task.result as T);
          }
          options?.onComplete?.();
          resolve((task.result as T) ?? (true as T));
          return;
        }

        if (task.status === 'failed') {
          stopPolling();
          const errorMsg = task.error || task.message || 'Background task failed';
          options?.onError?.(errorMsg);
          reject(new Error(errorMsg));
          return;
        }

        if (task.status === 'cancelled') {
          stopPolling();
          const errorMsg = task.message || 'Background task cancelled';
          options?.onCancelled?.(errorMsg);
          const cancelledError = new Error(errorMsg) as Error & { code?: string };
          cancelledError.name = 'TaskCancelledError';
          cancelledError.code = 'TASK_CANCELLED';
          reject(cancelledError);
          return;
        }
      } catch (error) {
        stopPolling();
        const errorMsg = error instanceof Error ? error.message : 'Failed to poll task status';
        options?.onError?.(errorMsg);
        reject(error);
      }
    };

    void poll();
    timer = window.setInterval(() => {
      void poll();
    }, 1500);
  });
};
