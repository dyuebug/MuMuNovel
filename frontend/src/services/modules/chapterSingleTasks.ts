import { useBackgroundTaskStore } from '../../store/backgroundTasks';
import type {
  CreativeMode,
  PlotStage,
  QualityPreset,
  StoryFocus,
} from '../../types';
import { api, silentRequestConfig, type RequestConfigWithToastControl } from '../core/httpClient';
import {
  normalizeChapterTaskStatus,
  upsertChapterTaskToStore,
} from './chapterTaskState';
import type {
  ChapterBatchCancelResponse,
  ChapterBatchGenerateStatusResponse,
  ChapterBatchResumeResponse,
  ChapterSingleGenerateResponse,
} from './chapterTaskTypes';

type ChapterSingleGeneratePayload = {
  style_id?: number;
  target_word_count?: number;
  model?: string;
  narrative_perspective?: string;
  creative_mode?: CreativeMode;
  story_focus?: StoryFocus;
  plot_stage?: PlotStage;
  story_creation_brief?: string;
  quality_preset?: QualityPreset;
  quality_notes?: string;
  enable_web_research?: boolean;
  web_research_query?: string;
  story_repair_summary?: string;
  story_repair_targets?: string[];
  story_preserve_strengths?: string[];
};

export const chapterSingleTaskApi = {
  createSingleGenerateTask: async (
    chapterId: string,
    payload: ChapterSingleGeneratePayload,
    projectId?: string,
  ) => {
    const created = await api.post<unknown, ChapterSingleGenerateResponse>(
      `/chapters/${chapterId}/generate-background`,
      payload,
    );
    useBackgroundTaskStore.getState().upsertTask({
      task_id: created.task_id,
      task_type: 'chapter_single_generate',
      project_id: projectId,
      status: normalizeChapterTaskStatus(created.status),
      progress: created.status === 'pending' ? 0 : 10,
      stage_code: '6.writing',
      execution_mode: 'interactive',
      message: created.message || '单章后台任务已创建',
      checkpoint: { chapter_id: created.chapter_id || chapterId },
      active_story_repair_payload: created.active_story_repair_payload ?? undefined,
    });
    return created;
  },

  getSingleGenerateTaskStatus: async (taskId: string, projectId?: string) => {
    const status = await api.get<unknown, ChapterBatchGenerateStatusResponse>(
      `/chapters/batch-generate/${taskId}/status`,
      { ...silentRequestConfig(), suppressAuthRedirect: true } as RequestConfigWithToastControl,
    );
    upsertChapterTaskToStore({
      taskType: 'chapter_single_generate',
      taskId: status.batch_id,
      status: status.status,
      total: status.total,
      completed: status.completed,
      projectId,
      currentChapterNumber: status.current_chapter_number,
      errorMessage: status.error_message,
      stageCode: status.stage_code ?? '6.writing',
      executionMode: status.execution_mode ?? 'interactive',
      checkpoint: {
        ...(status.checkpoint ?? {}),
        chapter_id: status.current_chapter_id ?? status.checkpoint?.chapter_id ?? null,
      },
      failedChapters: status.failed_chapters ?? undefined,
      activeStoryRepairPayload: status.active_story_repair_payload ?? undefined,
      terminalReason: status.terminal_reason,
      terminalLabel: status.terminal_label,
      reviewRequired: status.review_required,
      canResume: status.can_resume,
      createdAt: status.created_at,
      completedAt: status.completed_at,
    });
    return status;
  },

  cancelSingleGenerateTask: async (taskId: string, projectId?: string) => {
    const cancelled = await api.post<unknown, ChapterBatchCancelResponse>(
      `/chapters/batch-generate/${taskId}/cancel`,
    );
    useBackgroundTaskStore.getState().upsertTask({
      task_id: cancelled.batch_id,
      task_type: 'chapter_single_generate',
      project_id: projectId,
      status: 'cancelled',
      progress: 100,
      message: cancelled.message || '单章生成任务已取消',
    });
    return cancelled;
  },

  resumeSingleGenerateTask: async (taskId: string, projectId?: string) => {
    const resumed = await api.post<unknown, ChapterBatchResumeResponse>(
      `/chapters/batch-generate/${taskId}/resume`,
    );
    upsertChapterTaskToStore({
      taskType: 'chapter_single_generate',
      taskId: resumed.batch_id,
      status: resumed.status,
      total: resumed.total_chapters,
      completed: resumed.completed_chapters,
      projectId: resumed.project_id ?? projectId,
      stageCode: resumed.stage_code ?? '6.writing.loading',
      executionMode: resumed.execution_mode ?? 'interactive',
      checkpoint: {
        ...(resumed.checkpoint ?? {}),
        chapter_id: resumed.current_chapter_id ?? resumed.checkpoint?.chapter_id ?? null,
      },
      createdAt: resumed.created_at,
    });
    return resumed;
  },
};
