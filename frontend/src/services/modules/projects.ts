import axios from 'axios';

import type {
  NovelWorkflowPhase,
  NovelWorkflowStateView,
  NovelWorkflowTransitionReceipt,
  NovelWorkflowTransitionRequest,
  Project,
  ProjectCreate,
  ProjectUpdate,
  RuntimeMetricsResponseV1,
} from '../../types';
import { api } from '../core/httpClient';
import type { RequestConfigWithToastControl } from '../core/httpClient';

export type AutopilotInvocationAuditStatus =
  | 'queued'
  | 'running'
  | 'succeeded'
  | 'failed'
  | 'cancelled';

export interface AutopilotInvocationAuditInputSummary {
  expected_phase: NovelWorkflowPhase;
  target_phase: NovelWorkflowPhase;
  reason_provided: boolean;
  related_task_id_provided: boolean;
}

export interface AutopilotInvocationAuditResultSummary {
  changed: boolean;
  previous_phase: NovelWorkflowPhase;
  current_phase: NovelWorkflowPhase;
}

/**
 * 仅供项目工作流中的只读审计历史展示。
 * 有意不声明 raw arguments、reason、Prompt、provider/model、digest 与 actor 字段，
 * 防止组件层将敏感审计原始数据接入 UI。
 */
export interface AutopilotInvocationAuditHistoryItem {
  audit_id: string;
  tool_name: string;
  tool_schema_version: string;
  confirmed_by_user: boolean;
  execution_mode: 'direct_business_tool';
  input_summary: AutopilotInvocationAuditInputSummary;
  status: AutopilotInvocationAuditStatus;
  result_summary: AutopilotInvocationAuditResultSummary | null;
  error_code: string | null;
  created_at: string;
  started_at: string | null;
  completed_at: string | null;
}

export interface AutopilotInvocationAuditHistoryResponse {
  items: AutopilotInvocationAuditHistoryItem[];
}

export const projectApi = {
  getProjects: () => api.get<unknown, Project[]>('/projects'),

  getProject: (id: string, config?: RequestConfigWithToastControl) =>
    api.get<unknown, Project>(`/projects/${id}`, config),

  createProject: (data: ProjectCreate) => api.post<unknown, Project>('/projects', data),

  updateProject: (id: string, data: ProjectUpdate) =>
    api.put<unknown, Project>(`/projects/${id}`, data),

  getWorkflowState: (id: string, config?: RequestConfigWithToastControl) =>
    api.get<unknown, NovelWorkflowStateView>(`/projects/${id}/workflow-state`, config),

  getRuntimeMetrics: (id: string, config?: RequestConfigWithToastControl) =>
    api.get<unknown, RuntimeMetricsResponseV1>(`/projects/${id}/runtime-metrics`, config),

  transitionWorkflowState: (
    id: string,
    data: NovelWorkflowTransitionRequest,
    config?: RequestConfigWithToastControl,
  ) => api.post<unknown, NovelWorkflowTransitionReceipt>(
    `/projects/${id}/workflow-state/transition`,
    data,
    config,
  ),

  getAutopilotInvocationHistory: (
    id: string,
    config?: RequestConfigWithToastControl,
  ) => api.get<unknown, AutopilotInvocationAuditHistoryResponse>(
    `/projects/${id}/autopilot/invocations`,
    config,
  ),

  deleteProject: (id: string) => api.delete(`/projects/${id}`),

  exportProject: (id: string) => {
    window.open(`/api/projects/${id}/export`, '_blank');
  },

  exportProjectData: async (id: string, options: {
    include_generation_history?: boolean;
    include_writing_styles?: boolean;
    include_careers?: boolean;
    include_memories?: boolean;
    include_plot_analysis?: boolean;
  }) => {
    const response = await axios.post(
      `/api/projects/${id}/export-data`,
      options,
      {
        responseType: 'blob',
        headers: {
          'Content-Type': 'application/json',
        },
      }
    );

    const contentDisposition = response.headers['content-disposition'];
    let filename = 'project_export.json';
    if (contentDisposition) {
      const matches = /filename\*=UTF-8''(.+)/.exec(contentDisposition);
      if (matches && matches[1]) {
        filename = decodeURIComponent(matches[1]);
      }
    }

    const url = window.URL.createObjectURL(new Blob([response.data]));
    const link = document.createElement('a');
    link.href = url;
    link.setAttribute('download', filename);
    document.body.appendChild(link);
    link.click();
    link.remove();
    window.URL.revokeObjectURL(url);
  },

  validateImportFile: (file: File) => {
    const formData = new FormData();
    formData.append('file', file);
    return api.post<unknown, {
      valid: boolean;
      version: string;
      project_name?: string;
      statistics: Record<string, number>;
      errors: string[];
      warnings: string[];
    }>('/projects/validate-import', formData, {
      headers: { 'Content-Type': 'multipart/form-data' },
    });
  },

  importProject: (file: File) => {
    const formData = new FormData();
    formData.append('file', file);
    return api.post<unknown, {
      success: boolean;
      project_id?: string;
      message: string;
      statistics: Record<string, number>;
      warnings: string[];
    }>('/projects/import', formData, {
      headers: { 'Content-Type': 'multipart/form-data' },
    });
  },
};
