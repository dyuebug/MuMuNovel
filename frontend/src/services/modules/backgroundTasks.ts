import { api, getAxiosErrorStatus, silentRequestConfig, type RequestConfigWithToastControl } from '../core/httpClient';
import { SSEClient, type SSEClientOptions } from '../../utils/sseClient';
import {
  buildMissingBackgroundTaskStatus,
  removeBackgroundTaskFromStore,
  syncBackgroundTaskListToStore,
  syncBackgroundTaskToStore,
} from './backgroundTaskStoreSync';
import type {
  BackgroundTaskListResponse,
  BackgroundTaskStatus,
  ConfirmedAutopilotWorkflowTransitionRequest,
} from './backgroundTaskTypes';

export type {
  BackgroundTaskListResponse,
  BackgroundTaskStatus,
  ConfirmedAutopilotWorkflowTransitionRequest,
} from './backgroundTaskTypes';

let backgroundTasksEndpointSupported = true;

export type BackgroundTaskStreamOptions = Pick<
  SSEClientOptions,
  'onChunk' | 'onReasoningChunk'
>;

export const backgroundTaskApi = {
  subscribeTaskStream: (
    taskId: string,
    options: BackgroundTaskStreamOptions,
  ): (() => void) => {
    const baseUrl = String(api.defaults.baseURL || '/api').replace(/\/+$/, '');
    const client = new SSEClient(
      `${baseUrl}/background-tasks/${encodeURIComponent(taskId)}/stream`,
      options,
    );

    void client.connect().catch(() => {
      // 实时输出是 best-effort；断线时仍由状态轮询负责终态与正式结果。
    });

    return () => client.close();
  },

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

  createConfirmedAutopilotWorkflowTransition: async (
    projectId: string,
    data: ConfirmedAutopilotWorkflowTransitionRequest,
  ) => {
    const created = await api.post<unknown, BackgroundTaskStatus>(
      `/projects/${projectId}/autopilot/actions`,
      data,
    );
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
    try {
      const cancelled = await api.post<unknown, BackgroundTaskStatus>(`/background-tasks/${taskId}/cancel`);
      return syncBackgroundTaskToStore(cancelled);
    } catch (error: unknown) {
      if (getAxiosErrorStatus(error) === 404) {
        removeBackgroundTaskFromStore(taskId);
        return buildMissingBackgroundTaskStatus(taskId);
      }
      throw error;
    }
  },
};
