import { api } from '../core/httpClient';
import type {
  PolishBatchRequest,
  PolishTextRequest,
  PolishTextResponse,
} from '../../types';
import { waitForBackgroundTaskCompletion } from '../../utils/taskPolling';
import { backgroundTaskApi } from './backgroundTasks';

type PolishBatchResponse = {
  total: number;
  results: Array<{
    index: number;
    original: string;
    polished: string;
    word_count_before: number;
    word_count_after: number;
  }>;
};

const normalizePolishBatchPayload = (data: PolishBatchRequest | string[]) => (
  Array.isArray(data) ? { texts: data } : data
);

export const polishApi = {
  polishText: (data: PolishTextRequest) =>
    api.post<unknown, PolishTextResponse>('/polish', data),

  polishTextInBackground: async (data: PolishTextRequest) => {
    const task = await backgroundTaskApi.createTask({
      task_type: 'polish_text',
      project_id: data.project_id ? String(data.project_id) : undefined,
      payload: data as unknown as Record<string, unknown>,
    });

    return waitForBackgroundTaskCompletion<typeof task, PolishTextResponse>(task, {
      pollTask: backgroundTaskApi.getTaskStatus,
      progressMessage: 'AI 去味任务已创建，正在后台执行',
      resolveValue: (latestTask) => latestTask.result as unknown as PolishTextResponse,
    });
  },

  polishBatch: (data: PolishBatchRequest | string[]) =>
    api.post<unknown, PolishBatchResponse>('/polish/batch', normalizePolishBatchPayload(data)),

  polishBatchInBackground: async (data: PolishBatchRequest | string[]) => {
    const payload = normalizePolishBatchPayload(data);
    const task = await backgroundTaskApi.createTask({
      task_type: 'polish_batch',
      payload: payload as unknown as Record<string, unknown>,
    });

    return waitForBackgroundTaskCompletion<typeof task, PolishBatchResponse>(task, {
      pollTask: backgroundTaskApi.getTaskStatus,
      progressMessage: '批量 AI 去味任务已创建，正在后台执行',
      resolveValue: (latestTask) => latestTask.result as unknown as PolishBatchResponse,
    });
  },
};
