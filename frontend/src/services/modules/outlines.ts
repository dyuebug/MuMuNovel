import { api } from '../core/httpClient';
import type {
  BatchOutlineExpansionRequest,
  BatchOutlineExpansionResponse,
  ChapterPlanItem,
  GenerateOutlineRequest,
  Outline,
  OutlineCreate,
  OutlineExpansionRequest,
  OutlineExpansionResponse,
  OutlineReorderRequest,
  OutlineUpdate,
} from '../../types';

export const outlineApi = {
  getOutlines: (projectId: string) =>
    api.get<unknown, { total: number; items: Outline[] }>(`/outlines/project/${projectId}`).then(res => res.items),

  getOutline: (id: string) => api.get<unknown, Outline>(`/outlines/${id}`),

  createOutline: (data: OutlineCreate) => api.post<unknown, Outline>('/outlines', data),

  updateOutline: (id: string, data: OutlineUpdate) =>
    api.put<unknown, Outline>(`/outlines/${id}`, data),

  deleteOutline: (id: string) => api.delete(`/outlines/${id}`),

  reorderOutlines: (data: OutlineReorderRequest) =>
    api.post<unknown, { message: string; updated_outlines: number; updated_chapters: number }>('/outlines/reorder', data),

  generateOutline: (data: GenerateOutlineRequest) =>
    api.post<unknown, { total: number; items: Outline[] }>('/outlines/generate', data).then(res => res.items),

  getOutlineChapters: (outlineId: string) =>
    api.get<unknown, {
      has_chapters: boolean;
      outline_id: string;
      outline_title: string;
      chapter_count: number;
      chapters: Array<{
        id: string;
        chapter_number: number;
        title: string;
        summary: string;
        sub_index: number;
        status: string;
        word_count: number;
      }>;
      expansion_plans: Array<{
        sub_index: number;
        title: string;
        plot_summary: string;
        key_events: string[];
        character_focus: string[];
        emotional_tone: string;
        narrative_goal: string;
        conflict_type: string;
        estimated_words: number;
        scenes?: Array<{
          location: string;
          characters: string[];
          purpose: string;
        }> | null;
      }> | null;
    }>(`/outlines/${outlineId}/chapters`),

  expandOutline: (outlineId: string, data: OutlineExpansionRequest) =>
    api.post<unknown, OutlineExpansionResponse>(`/outlines/${outlineId}/expand`, data),

  createChaptersFromPlans: (outlineId: string, chapterPlans: ChapterPlanItem[]) =>
    api.post<unknown, {
      outline_id: string;
      outline_title: string;
      chapters_created: number;
      created_chapters: Array<{
        id: string;
        chapter_number: number;
        title: string;
        summary: string;
        outline_id: string;
        sub_index: number;
        status: string;
      }>;
    }>(`/outlines/${outlineId}/create-chapters-from-plans`, { chapter_plans: chapterPlans }),

  batchExpandOutlines: (data: BatchOutlineExpansionRequest) =>
    api.post<unknown, BatchOutlineExpansionResponse>('/outlines/batch-expand', data),
};