import { useStore } from '../../store';
import { useBackgroundTaskStore } from '../../store/backgroundTasks';
import type {
  ActiveStoryRepairPayload,
  CreativeMode,
  PlotStage,
  QualityPreset,
  StoryFocus,
} from '../../types';
import { api, getAxiosErrorStatus, silentRequestConfig, type RequestConfigWithToastControl } from '../core/httpClient';
import {
  upsertChapterTaskToStore,
  type ChapterGenerationTaskType,
} from './chapterTaskState';
import type {
  ChapterBatchActiveResponse,
  ChapterBatchCancelResponse,
  ChapterBatchGenerateResponse,
  ChapterBatchGenerateStatusResponse,
  ChapterBatchResumeResponse,
} from './chapterTaskTypes';

type ChapterBatchGeneratePayload = {
  start_chapter_number: number;
  count: number;
  enable_analysis: boolean;
  style_id: number;
  target_word_count: number;
  model?: string;
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

type ChapterActiveTasksResponse = {
  total: number;
  items: Array<{
    task_type: ChapterGenerationTaskType;
    stage_code?: string | null;
    execution_mode?: 'interactive' | 'auto' | null;
    project_id: string;
    batch_id: string;
    status: string;
    total: number;
    completed: number;
    current_chapter_number?: number | null;
    checkpoint?: Record<string, unknown> | null;
    active_story_repair_payload?: ActiveStoryRepairPayload | null;
    error_message?: string | null;
    created_at?: string | null;
    completed_at?: string | null;
  }>;
};

let chapterActiveTasksEndpointSupported = true;

const getKnownProjectIds = () =>
  new Set(useStore.getState().projects.map((project) => project.id));

export const chapterBatchTaskApi = {
  createBatchGenerateTask: async (
    projectId: string,
    payload: ChapterBatchGeneratePayload,
  ) => {
    const created = await api.post<unknown, ChapterBatchGenerateResponse>(
      `/chapters/project/${projectId}/batch-generate`,
      payload,
    );
    useBackgroundTaskStore.getState().upsertTask({
      task_id: created.batch_id,
      task_type: 'chapters_batch_generate',
      project_id: projectId,
      status: 'pending',
      progress: 0,
      stage_code: '6.writing',
      execution_mode: 'interactive',
      message: created.message || '批量生成任务已创建',
    });
    return created;
  },

  getBatchGenerateStatus: async (batchId: string, projectId?: string) => {
    const status = await api.get<unknown, ChapterBatchGenerateStatusResponse>(
      `/chapters/batch-generate/${batchId}/status`,
      { ...silentRequestConfig(), suppressAuthRedirect: true } as RequestConfigWithToastControl,
    );
    upsertChapterTaskToStore({
      taskType: 'chapters_batch_generate',
      taskId: status.batch_id,
      status: status.status,
      total: status.total,
      completed: status.completed,
      projectId,
      currentChapterNumber: status.current_chapter_number,
      errorMessage: status.error_message,
      stageCode: status.stage_code ?? '6.writing',
      executionMode: status.execution_mode ?? 'interactive',
      checkpoint: status.checkpoint ?? undefined,
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

  getActiveBatchGenerateTask: async (projectId: string) => {
    const active = await api.get<unknown, ChapterBatchActiveResponse>(
      `/chapters/project/${projectId}/batch-generate/active`,
    );
    if (active.has_active_task && active.task) {
      upsertChapterTaskToStore({
        taskType: 'chapters_batch_generate',
        taskId: active.task.batch_id,
        status: active.task.status,
        total: active.task.total,
        completed: active.task.completed,
        projectId,
        currentChapterNumber: active.task.current_chapter_number,
        stageCode: active.task.stage_code ?? '6.writing',
        executionMode: active.task.execution_mode ?? 'interactive',
        checkpoint: active.task.checkpoint ?? undefined,
        activeStoryRepairPayload: active.task.active_story_repair_payload ?? undefined,
        createdAt: active.task.created_at,
      });
    }
    return active;
  },

  listActiveTasks: async (limit = 20) => {
    if (!chapterActiveTasksEndpointSupported) {
      return { total: 0, items: [] };
    }

    let response: ChapterActiveTasksResponse;
    try {
      response = await api.get<unknown, ChapterActiveTasksResponse>(
        '/chapters/batch-generate/active-tasks',
        { ...silentRequestConfig({ params: { limit } }), suppressAuthRedirect: true } as RequestConfigWithToastControl,
      );
    } catch (error: unknown) {
      if (getAxiosErrorStatus(error) === 404) {
        chapterActiveTasksEndpointSupported = false;
        return { total: 0, items: [] };
      }
      throw error;
    }

    const projectIds = getKnownProjectIds();
    const shouldFilterByProject = projectIds.size > 0;
    const items = shouldFilterByProject
      ? (response.items || []).filter((task) => !task.project_id || projectIds.has(task.project_id))
      : (response.items || []);

    if (shouldFilterByProject) {
      useBackgroundTaskStore.getState().pruneTasksByProjectIds([...projectIds]);
    }

    for (const task of items) {
      upsertChapterTaskToStore({
        taskType: task.task_type,
        taskId: task.batch_id,
        status: task.status,
        total: task.total,
        completed: task.completed,
        projectId: task.project_id,
        currentChapterNumber: task.current_chapter_number ?? null,
        errorMessage: task.error_message ?? null,
        stageCode: task.stage_code ?? '6.writing',
        executionMode: task.execution_mode ?? 'interactive',
        checkpoint: task.checkpoint ?? undefined,
        activeStoryRepairPayload: task.active_story_repair_payload ?? undefined,
        createdAt: task.created_at,
        completedAt: task.completed_at,
      });
    }
    return { ...response, items };
  },

  cancelBatchGenerateTask: async (batchId: string, projectId?: string) => {
    const cancelled = await api.post<unknown, ChapterBatchCancelResponse>(
      `/chapters/batch-generate/${batchId}/cancel`,
    );
    useBackgroundTaskStore.getState().upsertTask({
      task_id: cancelled.batch_id,
      task_type: 'chapters_batch_generate',
      project_id: projectId,
      status: 'cancelled',
      progress: 100,
      message: cancelled.message || '批量生成任务已取消',
    });
    return cancelled;
  },

  resumeBatchGenerateTask: async (batchId: string, projectId?: string) => {
    const resumed = await api.post<unknown, ChapterBatchResumeResponse>(
      `/chapters/batch-generate/${batchId}/resume`,
    );
    upsertChapterTaskToStore({
      taskType: resumed.task_type ?? 'chapters_batch_generate',
      taskId: resumed.batch_id,
      status: resumed.status,
      total: resumed.total_chapters,
      completed: resumed.completed_chapters,
      projectId: resumed.project_id ?? projectId,
      stageCode: resumed.stage_code ?? '6.writing.loading',
      executionMode: resumed.execution_mode ?? 'interactive',
      checkpoint: resumed.checkpoint ?? undefined,
      createdAt: resumed.created_at,
    });
    return resumed;
  },
};