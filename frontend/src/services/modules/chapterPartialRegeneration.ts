import { ssePost } from '../../utils/sseClient';
import type { SSEClientOptions } from '../../utils/sseClient';
import { api } from '../core/httpClient';
import type { ResearchAssetSummary } from '../../types';

export const chapterPartialRegenerationApi = {
  partialRegenerateStream: (
    chapterId: string,
    data: {
      selected_text: string;
      start_position: number;
      end_position: number;
      user_instructions: string;
      context_chars?: number;
      style_id?: number;
      length_mode?: 'similar' | 'expand' | 'condense' | 'custom';
      target_word_count?: number;
      enable_web_research?: boolean;
      web_research_query?: string;
      reference_research_assets?: ResearchAssetSummary[];
    },
    options?: SSEClientOptions,
  ) => ssePost<{
    new_text: string;
    word_count: number;
    original_word_count: number;
    start_position: number;
    end_position: number;
  }>(
    `/api/chapters/${chapterId}/partial-regenerate-stream`,
    data,
    options,
  ),

  applyPartialRegenerate: (chapterId: string, data: {
    new_text: string;
    start_position: number;
    end_position: number;
  }) =>
    api.post<unknown, {
      success: boolean;
      chapter_id: string;
      word_count: number;
      old_word_count: number;
      message: string;
    }>(`/chapters/${chapterId}/apply-partial-regenerate`, data),
};