import { api } from '../core/httpClient';
import type {
  Foreshadow,
  ForeshadowContextResponse,
  ForeshadowCreate,
  ForeshadowListResponse,
  ForeshadowStats,
  ForeshadowUpdate,
  PlantForeshadowRequest,
  ResolveForeshadowRequest,
  SyncFromAnalysisRequest,
  SyncFromAnalysisResponse,
} from '../../types';

export const foreshadowApi = {
  getProjectForeshadows: (projectId: string, params?: {
    status?: string;
    category?: string;
    source_type?: string;
    is_long_term?: boolean;
    page?: number;
    limit?: number;
  }) =>
    api.get<unknown, ForeshadowListResponse>(
      `/foreshadows/projects/${projectId}`,
      { params },
    ),

  getForeshadowStats: (projectId: string, currentChapter?: number) =>
    api.get<unknown, ForeshadowStats>(
      `/foreshadows/projects/${projectId}/stats`,
      { params: { current_chapter: currentChapter } },
    ),

  getChapterContext: (projectId: string, chapterNumber: number, params?: {
    include_pending?: boolean;
    include_overdue?: boolean;
    lookahead?: number;
  }) =>
    api.get<unknown, ForeshadowContextResponse>(
      `/foreshadows/projects/${projectId}/context/${chapterNumber}`,
      { params },
    ),

  getPendingResolveForeshadows: (projectId: string, currentChapter: number, lookahead?: number) =>
    api.get<unknown, { total: number; items: Foreshadow[] }>(
      `/foreshadows/projects/${projectId}/pending-resolve`,
      { params: { current_chapter: currentChapter, lookahead } },
    ),

  getForeshadow: (foreshadowId: string) =>
    api.get<unknown, Foreshadow>(`/foreshadows/${foreshadowId}`),

  createForeshadow: (data: ForeshadowCreate) =>
    api.post<unknown, Foreshadow>('/foreshadows', data),

  updateForeshadow: (foreshadowId: string, data: ForeshadowUpdate) =>
    api.put<unknown, Foreshadow>(`/foreshadows/${foreshadowId}`, data),

  deleteForeshadow: (foreshadowId: string) =>
    api.delete<unknown, { message: string; id: string }>(`/foreshadows/${foreshadowId}`),

  plantForeshadow: (foreshadowId: string, data: PlantForeshadowRequest) =>
    api.post<unknown, Foreshadow>(`/foreshadows/${foreshadowId}/plant`, data),

  resolveForeshadow: (foreshadowId: string, data: ResolveForeshadowRequest) =>
    api.post<unknown, Foreshadow>(`/foreshadows/${foreshadowId}/resolve`, data),

  abandonForeshadow: (foreshadowId: string, reason?: string) =>
    api.post<unknown, Foreshadow>(
      `/foreshadows/${foreshadowId}/abandon`,
      null,
      { params: { reason } },
    ),

  syncFromAnalysis: (projectId: string, data: SyncFromAnalysisRequest) =>
    api.post<unknown, SyncFromAnalysisResponse>(
      `/foreshadows/projects/${projectId}/sync-from-analysis`,
      data,
    ),
};