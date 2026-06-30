import { ssePost } from '../../utils/sseClient';
import type { SSEClientOptions } from '../../utils/sseClient';
import type {
  BookImportApplyPayload,
  BookImportPreview,
  BookImportResult,
  BookImportStepFailure,
  BookImportRetryResult,
  BookImportTask,
} from '../../types';
import { api } from '../core/httpClient';
import { waitForBackgroundTaskCompletion } from '../../utils/taskPolling';
import { backgroundTaskApi } from './backgroundTasks';

const normalizeBookImportStepFailures = (value: unknown): BookImportStepFailure[] => {
  if (!Array.isArray(value)) return [];

  return value
    .reduce<BookImportStepFailure[]>((acc, item) => {
      if (!item || typeof item !== 'object') return acc;
      const record = item as Record<string, unknown>;
      const stepName = typeof record.step_name === 'string'
        ? record.step_name
        : typeof record.step === 'string'
          ? record.step
          : '';
      const stepLabel = typeof record.step_label === 'string'
        ? record.step_label
        : typeof record.label === 'string'
          ? record.label
          : '';
      const error = typeof record.error === 'string'
        ? record.error
        : typeof record.error_message === 'string'
          ? record.error_message
          : '';
      if (!stepName) return acc;

      acc.push({
        step_name: stepName,
        step_label: stepLabel || stepName,
        error,
        retry_count: typeof record.retry_count === 'number' ? record.retry_count : undefined,
      });
      return acc;
    }, []);
};

const normalizeBookImportResult = (result: BookImportResult): BookImportResult => ({
  ...result,
  failed_steps: normalizeBookImportStepFailures(
    (result as BookImportResult & { failed_steps?: unknown }).failed_steps,
  ),
});

const normalizeBookImportRetryResult = (result: BookImportRetryResult): BookImportRetryResult => ({
  ...result,
  still_failed: normalizeBookImportStepFailures(result.still_failed),
});

export const bookImportApi = {
  createTask: (params: { file: File }) => {
    const formData = new FormData();
    formData.append('file', params.file);

    return api.post<unknown, { task_id: string; status: BookImportTask['status'] }>(
      '/book-import/tasks',
      formData,
      { headers: { 'Content-Type': 'multipart/form-data' } }
    );
  },

  getTaskStatus: (taskId: string) =>
    api.get<unknown, BookImportTask>(`/book-import/tasks/${taskId}`),

  getPreview: (taskId: string) =>
    api.get<unknown, BookImportPreview>(`/book-import/tasks/${taskId}/preview`),

  applyImport: (taskId: string, payload: BookImportApplyPayload) =>
    api.post<unknown, BookImportResult>(`/book-import/tasks/${taskId}/apply`, payload),

  applyImportInBackground: async (taskId: string, payload: BookImportApplyPayload) => {
    const createdTask = await backgroundTaskApi.createTask({
      task_type: 'book_import_apply',
      payload: {
        ...payload,
        book_import_task_id: taskId,
      },
    });

    const result = await waitForBackgroundTaskCompletion<typeof createdTask, BookImportResult>(createdTask, {
      pollTask: backgroundTaskApi.getTaskStatus,
      progressMessage: '拆书导入任务已创建，正在后台执行',
      resolveValue: (task) => normalizeBookImportResult(((task.result as unknown) as BookImportResult) ?? {} as BookImportResult),
    });

    return normalizeBookImportResult(result);
  },

  applyImportStream: (
    taskId: string,
    payload: BookImportApplyPayload,
    options?: SSEClientOptions<BookImportResult>,
  ) => ssePost<BookImportResult>(
    `/api/book-import/tasks/${taskId}/apply-stream`,
    payload,
    options,
  ),

  retryFailedStepsStream: (
    taskId: string,
    steps: string[],
    options?: SSEClientOptions<BookImportRetryResult>,
  ) => ssePost<BookImportRetryResult>(
    `/api/book-import/tasks/${taskId}/retry-stream`,
    { steps },
    options,
  ),

  retryFailedStepsInBackground: async (taskId: string, steps: string[]) => {
    const createdTask = await backgroundTaskApi.createTask({
      task_type: 'book_import_retry_failed_steps',
      payload: {
        book_import_task_id: taskId,
        steps,
      },
    });

    const result = await waitForBackgroundTaskCompletion<typeof createdTask, BookImportRetryResult>(createdTask, {
      pollTask: backgroundTaskApi.getTaskStatus,
      progressMessage: '拆书失败步骤重试任务已创建，正在后台执行',
      resolveValue: (task) => normalizeBookImportRetryResult(((task.result as unknown) as BookImportRetryResult) ?? {} as BookImportRetryResult),
    });

    return normalizeBookImportRetryResult(result);
  },

  cancelTask: (taskId: string) =>
    api.delete<unknown, { success: boolean; message: string }>(`/book-import/tasks/${taskId}`),
};
