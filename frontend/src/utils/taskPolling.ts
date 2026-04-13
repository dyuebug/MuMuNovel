import type { SSEClientOptions } from './sseClient';

export const MAX_CONSECUTIVE_TASK_POLL_ERRORS = 3;

export interface PollableBackgroundTask {
  task_id: string;
  status: 'pending' | 'running' | 'completed' | 'failed' | 'cancelled';
  progress: number;
  message: string;
  result?: Record<string, unknown> | null;
  error?: string | null;
}

interface WaitForBackgroundTaskCompletionOptions<
  TTask extends PollableBackgroundTask,
  TResult,
> {
  pollTask: (taskId: string) => Promise<TTask>;
  sseOptions?: SSEClientOptions;
  pollIntervalMs?: number;
  progressMessage?: string;
  initialStatus?: string;
  failureFallbackMessage?: string;
  cancelledFallbackMessage?: string;
  pollErrorFallbackMessage?: string;
  createPollError?: (error: unknown, fallbackMessage: string) => Error;
  resolveValue?: (task: TTask) => TResult;
}

interface StartBackgroundTaskPollingOptions<TTask extends PollableBackgroundTask> {
  pollTask: (taskId: string) => Promise<TTask>;
  pollIntervalMs?: number;
  maxConsecutiveErrors?: number;
  onTask?: (task: TTask) => void;
  onCompleted?: (task: TTask) => void;
  onFailed?: (task: TTask) => void;
  onCancelled?: (task: TTask) => void;
  onPollingError?: (error: unknown) => void;
}

const hasTerminalResult = (task: Pick<PollableBackgroundTask, 'result'>) => (
  task.result !== undefined && task.result !== null
);

export const formatBackgroundTaskError = (
  error?: string | null,
  message?: string | null,
  fallback = '??????'
): string => {
  const normalizedError = typeof error === 'string' ? error.trim() : '';
  if (normalizedError === 'task_missing') {
    return '??????????????????';
  }

  const normalizedMessage = typeof message === 'string' ? message.trim() : '';
  return normalizedError || normalizedMessage || fallback;
};

export const waitForBackgroundTaskCompletion = <
  TTask extends PollableBackgroundTask,
  TResult = TTask['result'] | true,
>(
  task: TTask,
  {
    pollTask,
    sseOptions,
    pollIntervalMs = 1500,
    progressMessage = '???????',
    initialStatus = task.status,
    failureFallbackMessage = '????????',
    cancelledFallbackMessage = '???????',
    pollErrorFallbackMessage = '????????',
    createPollError = (error, fallbackMessage) => (
      error instanceof Error ? error : new Error(fallbackMessage)
    ),
    resolveValue = (latestTask) => ((latestTask.result as TResult) ?? (true as TResult)),
  }: WaitForBackgroundTaskCompletionOptions<TTask, TResult>
): Promise<TResult> => {
  sseOptions?.onTaskCreated?.(task.task_id);
  sseOptions?.onProgress?.(task.message || progressMessage, task.progress || 0, initialStatus);

  return new Promise<TResult>((resolve, reject) => {
    let timer: number | null = null;
    let consecutivePollErrors = 0;
    let settled = false;
    let polling = false;

    const stopPolling = () => {
      if (timer !== null) {
        window.clearTimeout(timer);
        timer = null;
      }
    };

    const resolveWithTaskResult = (latestTask: TTask) => {
      stopPolling();
      settled = true;
      if (hasTerminalResult(latestTask)) {
        sseOptions?.onResult?.(latestTask.result);
      }
      sseOptions?.onComplete?.();
      resolve(resolveValue(latestTask));
    };

    const scheduleNextPoll = () => {
      if (settled) {
        return;
      }
      timer = window.setTimeout(() => {
        void poll();
      }, pollIntervalMs);
    };

    const poll = async () => {
      if (settled || polling) {
        return;
      }

      polling = true;
      try {
        const latestTask = await pollTask(task.task_id);
        consecutivePollErrors = 0;
        sseOptions?.onProgress?.(latestTask.message || '', latestTask.progress || 0, latestTask.status);

        if (latestTask.status === 'completed' || hasTerminalResult(latestTask)) {
          resolveWithTaskResult(latestTask);
          return;
        }

        if (latestTask.status === 'failed') {
          stopPolling();
          settled = true;
          const errorMessage = formatBackgroundTaskError(
            latestTask.error,
            latestTask.message,
            failureFallbackMessage,
          );
          sseOptions?.onError?.(errorMessage);
          reject(new Error(errorMessage));
          return;
        }

        if (latestTask.status === 'cancelled') {
          stopPolling();
          settled = true;
          const errorMessage = latestTask.message || cancelledFallbackMessage;
          sseOptions?.onCancelled?.(errorMessage);
          const cancelledError = new Error(errorMessage) as Error & { code?: string };
          cancelledError.name = 'TaskCancelledError';
          cancelledError.code = 'TASK_CANCELLED';
          reject(cancelledError);
          return;
        }
      } catch (error) {
        consecutivePollErrors += 1;
        if (consecutivePollErrors < MAX_CONSECUTIVE_TASK_POLL_ERRORS) {
          return;
        }

        stopPolling();
        settled = true;
        const pollError = createPollError(error, pollErrorFallbackMessage);
        sseOptions?.onError?.(pollError.message || pollErrorFallbackMessage);
        reject(pollError);
      } finally {
        polling = false;
        if (!settled) {
          scheduleNextPoll();
        }
      }
    };

    void poll();
  });
};

export const startBackgroundTaskPolling = <TTask extends PollableBackgroundTask>(
  taskId: string,
  {
    pollTask,
    pollIntervalMs = 1500,
    maxConsecutiveErrors = MAX_CONSECUTIVE_TASK_POLL_ERRORS,
    onTask,
    onCompleted,
    onFailed,
    onCancelled,
    onPollingError,
  }: StartBackgroundTaskPollingOptions<TTask>
): (() => void) => {
  let timer: number | null = null;
  let stopped = false;
  let polling = false;
  let consecutivePollErrors = 0;

  const stop = () => {
    stopped = true;
    if (timer !== null) {
      window.clearTimeout(timer);
      timer = null;
    }
  };

  const scheduleNextPoll = () => {
    if (stopped) {
      return;
    }
    timer = window.setTimeout(() => {
      void poll();
    }, pollIntervalMs);
  };

  const poll = async () => {
    if (stopped || polling) {
      return;
    }

    polling = true;
    try {
      const task = await pollTask(taskId);
      consecutivePollErrors = 0;
      onTask?.(task);

      if (task.status === 'completed') {
        stop();
        onCompleted?.(task);
        return;
      }

      if (task.status === 'failed') {
        stop();
        onFailed?.(task);
        return;
      }

      if (task.status === 'cancelled') {
        stop();
        onCancelled?.(task);
        return;
      }
    } catch (error) {
      consecutivePollErrors += 1;
      if (consecutivePollErrors < maxConsecutiveErrors) {
        return;
      }
      stop();
      onPollingError?.(error);
      return;
    } finally {
      polling = false;
      if (!stopped) {
        scheduleNextPoll();
      }
    }
  };

  void poll();
  return stop;
};
