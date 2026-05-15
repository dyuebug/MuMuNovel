import axios from 'axios';

import type {
  Project,
  ProjectCreate,
  ProjectUpdate,
} from '../../types';
import { api } from '../core/httpClient';
import type { RequestConfigWithToastControl } from '../core/httpClient';

export const projectApi = {
  getProjects: () => api.get<unknown, Project[]>('/projects'),

  getProject: (id: string, config?: RequestConfigWithToastControl) =>
    api.get<unknown, Project>(`/projects/${id}`, config),

  createProject: (data: ProjectCreate) => api.post<unknown, Project>('/projects', data),

  updateProject: (id: string, data: ProjectUpdate) =>
    api.put<unknown, Project>(`/projects/${id}`, data),

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
