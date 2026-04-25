import { api } from '../core/httpClient';
import type { RequestConfigWithToastControl } from '../core/httpClient';
import { chapterAnalysisApi } from './chapterAnalysis';
import { chapterPartialRegenerationApi } from './chapterPartialRegeneration';
import { chapterRegenerationTaskApi } from './chapterRegenerationTasks';
import type {
  Chapter,
  ChapterCanGenerateResponse,
  ChapterCreate,
  ChapterQualityMetricsResponse,
  ChapterUpdate,
  ProjectChapterQualityTrendResponse,
} from '../../types';

export const chapterApi = {
  ...chapterAnalysisApi,
  ...chapterPartialRegenerationApi,
  ...chapterRegenerationTaskApi,

  getChapters: (projectId: string) =>
    api.get<unknown, { total: number; items: Chapter[] }>(`/chapters/project/${projectId}`).then((res) => res.items),

  getChapter: (id: string, config?: RequestConfigWithToastControl) => api.get<unknown, Chapter>(`/chapters/${id}`, config),

  createChapter: (data: ChapterCreate) => api.post<unknown, Chapter>('/chapters', data),

  updateChapter: (id: string, data: ChapterUpdate) =>
    api.put<unknown, Chapter>(`/chapters/${id}`, data),

  deleteChapter: (id: string) => api.delete(`/chapters/${id}`),

  checkCanGenerate: (chapterId: string) =>
    api.get<unknown, ChapterCanGenerateResponse>(`/chapters/${chapterId}/can-generate`),

  getChapterQualityMetrics: (chapterId: string) =>
    api.get<unknown, ChapterQualityMetricsResponse>(`/chapters/${chapterId}/quality-metrics`),

  getProjectChapterQualityTrend: (projectId: string, limit = 12) =>
    api.get<unknown, ProjectChapterQualityTrendResponse>(
      `/chapters/project/${projectId}/quality-trend`,
      { params: { limit } },
    ),
};