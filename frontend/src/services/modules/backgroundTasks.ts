import { api, getAxiosErrorStatus, silentRequestConfig, type RequestConfigWithToastControl } from '../core/httpClient';
import {
  buildMissingBackgroundTaskStatus,
  removeBackgroundTaskFromStore,
  syncBackgroundTaskListToStore,
  syncBackgroundTaskToStore,
} from './backgroundTaskStoreSync';
import type { BackgroundTaskListResponse, BackgroundTaskStatus } from './backgroundTaskTypes';

export type { BackgroundTaskListResponse, BackgroundTaskStatus } from './backgroundTaskTypes';

let backgroundTasksEndpointSupported = true;

export const backgroundTaskApi = {
  createTask: async (data: {
    task_type: BackgroundTaskStatus['task_type'];
    project_id?: string;
    payload?: Record<string, unknown>;
    stage_code?: string;
    execution_mode?: 'interactive' | 'auto';
    workflow_scope?: string;
    checkpoint?: Record<string, unknown>;
  }) => {
    const created = await api.post<unknown, BackgroundTaskStatus>('/background-tasks', data);
    return syncBackgroundTaskToStore(created);
  },

  getTaskStatus: async (taskId: string) => {
    try {
      const status = await api.get<unknown, BackgroundTaskStatus>(
        `/background-tasks/${taskId}`,
        { ...silentRequestConfig(), suppressAuthRedirect: true } as RequestConfigWithToastControl,
      );
      return syncBackgroundTaskToStore(status);
    } catch (error: unknown) {
      if (getAxiosErrorStatus(error) === 404) {
        removeBackgroundTaskFromStore(taskId);
        return buildMissingBackgroundTaskStatus(taskId);
      }
      throw error;
    }
  },

  listTasks: async (params?: {
    project_id?: string;
    statuses?: string;
    active_only?: boolean;
    limit?: number;
  }) => {
    if (!backgroundTasksEndpointSupported) {
      return { total: 0, items: [] } as BackgroundTaskListResponse;
    }

    let data: BackgroundTaskListResponse;
    try {
      data = await api.get<unknown, BackgroundTaskListResponse>(
        '/background-tasks',
        { ...silentRequestConfig({ params }), suppressAuthRedirect: true } as RequestConfigWithToastControl,
      );
    } catch (error: unknown) {
      if (getAxiosErrorStatus(error) === 404) {
        backgroundTasksEndpointSupported = false;
        return { total: 0, items: [] } as BackgroundTaskListResponse;
      }
      throw error;
    }

    return syncBackgroundTaskListToStore(data);
  },

  updateWorkflowState: async (
    taskId: string,
    payload: {
      stage_code?: string;
      execution_mode?: 'interactive' | 'auto';
      workflow_scope?: string;
      checkpoint?: Record<string, unknown>;
      message?: string;
      progress?: number;
    },
  ) => {
    const status = await api.patch<unknown, BackgroundTaskStatus>(`/background-tasks/${taskId}/workflow-state`, payload);
    return syncBackgroundTaskToStore(status);
  },

  cancelTask: async (taskId: string) => {
    const cancelled = await api.post<unknown, BackgroundTaskStatus>(`/background-tasks/${taskId}/cancel`);
    return syncBackgroundTaskToStore(cancelled);
  },
};