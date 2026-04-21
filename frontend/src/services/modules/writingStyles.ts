import type {
  PresetStyle,
  WritingStyle,
  WritingStyleCreate,
  WritingStyleListResponse,
  WritingStyleUpdate,
} from '../../types';
import { api } from '../core/httpClient';

export const writingStyleApi = {
  getPresetStyles: () =>
    api.get<unknown, PresetStyle[]>('/writing-styles/presets/list'),

  getUserStyles: () =>
    api.get<unknown, WritingStyleListResponse>('/writing-styles/user'),

  getProjectStyles: (projectId: string) =>
    api.get<unknown, WritingStyleListResponse>(`/writing-styles/project/${projectId}`),

  createStyle: (data: WritingStyleCreate) =>
    api.post<unknown, WritingStyle>('/writing-styles', data),

  updateStyle: (styleId: number, data: WritingStyleUpdate) =>
    api.put<unknown, WritingStyle>(`/writing-styles/${styleId}`, data),

  deleteStyle: (styleId: number) =>
    api.delete<unknown, { message: string }>(`/writing-styles/${styleId}`),

  setDefaultStyle: (styleId: number, projectId: string) =>
    api.post<unknown, WritingStyle>(`/writing-styles/${styleId}/set-default`, { project_id: projectId }),

  initializeDefaultStyles: (projectId: string) =>
    api.post<unknown, WritingStyleListResponse>(`/writing-styles/project/${projectId}/initialize`, {}),
};