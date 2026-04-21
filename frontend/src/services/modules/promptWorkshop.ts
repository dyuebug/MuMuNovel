import { api } from '../core/httpClient';
import type {
  PromptSubmission,
  PromptSubmissionCreate,
  PromptWorkshopItem,
  PromptWorkshopListResponse,
  WritingStyle,
} from '../../types';

export const promptWorkshopApi = {
  getStatus: () =>
    api.get<unknown, { mode: string; instance_id: string; cloud_url?: string; cloud_connected?: boolean }>('/prompt-workshop/status'),

  getItems: (params?: {
    category?: string;
    search?: string;
    tags?: string;
    sort?: 'newest' | 'popular' | 'downloads';
    page?: number;
    limit?: number;
  }) => api.get<unknown, PromptWorkshopListResponse>('/prompt-workshop/items', { params }),

  getItem: (itemId: string) =>
    api.get<unknown, { success: boolean; data: PromptWorkshopItem }>(`/prompt-workshop/items/${itemId}`),

  importItem: (itemId: string, customName?: string) =>
    api.post<unknown, { success: boolean; message: string; writing_style: WritingStyle }>(
      `/prompt-workshop/items/${itemId}/import`,
      { custom_name: customName },
    ),

  toggleLike: (itemId: string) =>
    api.post<unknown, { success: boolean; liked: boolean; like_count: number }>(
      `/prompt-workshop/items/${itemId}/like`,
    ),

  submit: (data: PromptSubmissionCreate) =>
    api.post<unknown, { success: boolean; message: string; submission: PromptSubmission }>('/prompt-workshop/submit', data),

  getMySubmissions: (status?: string) =>
    api.get<unknown, { success: boolean; data: { total: number; items: PromptSubmission[] } }>(
      '/prompt-workshop/my-submissions',
      { params: { status } },
    ),

  withdrawSubmission: (submissionId: string) =>
    api.delete<unknown, { success: boolean; message: string }>(`/prompt-workshop/submissions/${submissionId}`),

  deleteSubmission: (submissionId: string) =>
    api.delete<unknown, { success: boolean; message: string }>(`/prompt-workshop/submissions/${submissionId}`, {
      params: { force: true },
    }),

  adminGetSubmissions: (params?: { status?: string; source?: string; page?: number; limit?: number }) =>
    api.get<unknown, {
      success: boolean;
      data: {
        total: number;
        pending_count: number;
        page: number;
        limit: number;
        items: PromptSubmission[];
      };
    }>('/prompt-workshop/admin/submissions', { params }),

  adminReviewSubmission: (submissionId: string, data: { action: 'approve' | 'reject'; review_note?: string; category?: string; tags?: string[] }) =>
    api.post<unknown, { success: boolean; message: string; workshop_item?: PromptWorkshopItem; submission?: PromptSubmission }>(
      `/prompt-workshop/admin/submissions/${submissionId}/review`,
      data,
    ),

  adminCreateItem: (data: { name: string; description?: string; prompt_content: string; category: string; tags?: string[] }) =>
    api.post<unknown, { success: boolean; item: PromptWorkshopItem }>('/prompt-workshop/admin/items', data),

  adminUpdateItem: (itemId: string, data: { name?: string; description?: string; prompt_content?: string; category?: string; tags?: string[]; status?: string }) =>
    api.put<unknown, { success: boolean; item: PromptWorkshopItem }>(`/prompt-workshop/admin/items/${itemId}`, data),

  adminDeleteItem: (itemId: string) =>
    api.delete<unknown, { success: boolean; message: string }>(`/prompt-workshop/admin/items/${itemId}`),

  adminGetStats: () =>
    api.get<unknown, {
      success: boolean;
      data: {
        total_items: number;
        total_official: number;
        total_pending: number;
        total_downloads: number;
        total_likes: number;
      };
    }>('/prompt-workshop/admin/stats'),
};