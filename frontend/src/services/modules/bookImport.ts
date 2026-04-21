import { ssePost } from '../../utils/sseClient';
import type { SSEClientOptions } from '../../utils/sseClient';
import type {
  BookImportApplyPayload,
  BookImportPreview,
  BookImportResult,
  BookImportRetryResult,
  BookImportTask,
} from '../../types';
import { api } from '../core/httpClient';

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

  cancelTask: (taskId: string) =>
    api.delete<unknown, { success: boolean; message: string }>(`/book-import/tasks/${taskId}`),
};