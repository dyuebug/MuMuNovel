import { ssePost } from '../../utils/sseClient';
import type { SSEClientOptions } from '../../utils/sseClient';
import { api } from '../core/httpClient';
import type { ResearchAssetSummary } from '../../types';
import { waitForBackgroundTaskCompletion } from '../../utils/taskPolling';
import { backgroundTaskApi } from './backgroundTasks';

type RegenerationPayload = {
  modification_source: 'custom' | 'analysis_suggestions' | 'mixed';
  custom_instructions?: string;
  selected_suggestion_indices?: number[];
  preserve_elements?: {
    preserve_structure?: boolean;
    preserve_dialogues?: string[];
    preserve_plot_points?: string[];
    preserve_character_traits?: boolean;
  };
  style_id?: string | number;
  target_word_count?: number;
  focus_areas?: string[];
  creative_mode?: string;
  story_focus?: string;
  plot_stage?: string;
  story_creation_brief?: string;
  quality_preset?: string;
  quality_notes?: string;
  enable_web_research?: boolean;
  web_research_query?: string;
  story_repair_summary?: string;
  story_repair_targets?: string[];
  story_preserve_strengths?: string[];
};

type PartialRegeneratePayload = {
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
};

export type PartialRegenerateResult = {
  new_text: string;
  word_count: number;
  original_word_count: number;
  start_position: number;
  end_position: number;
};

export type ChapterRegenerateResult = {
  content: string;
  word_count: number;
  task_id?: string;
  analysis_task_id?: string | null;
};

export const chapterPartialRegenerationApi = {
  regenerateChapterInBackground: async (
    chapterId: string,
    data: RegenerationPayload,
    options?: SSEClientOptions<ChapterRegenerateResult>,
    projectId?: string,
  ) => {
    const createdTask = await backgroundTaskApi.createTask({
      task_type: 'chapter_regenerate',
      project_id: projectId || chapterId,
      payload: {
        ...data,
        chapter_id: chapterId,
      },
      checkpoint: {
        chapter_id: chapterId,
      },
    });

    return waitForBackgroundTaskCompletion<typeof createdTask, ChapterRegenerateResult>(createdTask, {
      pollTask: backgroundTaskApi.getTaskStatus,
      sseOptions: options,
      progressMessage: '章节重生成任务已创建',
      failureFallbackMessage: '章节重生成失败',
      pollErrorFallbackMessage: '章节重生成任务状态同步失败',
      resolveValue: (task) => task.result as ChapterRegenerateResult,
    });
  },

  partialRegenerateStream: (
    chapterId: string,
    data: PartialRegeneratePayload,
    options?: SSEClientOptions,
  ) => ssePost<PartialRegenerateResult>(
    `/api/chapters/${chapterId}/partial-regenerate-stream`,
    data,
    options,
  ),

  partialRegenerateInBackground: async (
    chapterId: string,
    data: PartialRegeneratePayload,
    options?: SSEClientOptions<PartialRegenerateResult>,
  ) => {
    const createdTask = await backgroundTaskApi.createTask({
      task_type: 'chapter_partial_regenerate',
      project_id: chapterId,
      payload: {
        ...data,
        chapter_id: chapterId,
      },
      checkpoint: {
        chapter_id: chapterId,
      },
    });

    return waitForBackgroundTaskCompletion<typeof createdTask, PartialRegenerateResult>(createdTask, {
      pollTask: backgroundTaskApi.getTaskStatus,
      sseOptions: options,
      progressMessage: '局部重写任务已创建',
      failureFallbackMessage: '局部重写失败',
      pollErrorFallbackMessage: '局部重写任务状态同步失败',
      resolveValue: (task) => task.result as PartialRegenerateResult,
    });
  },

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
