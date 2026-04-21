import { useBackgroundTaskStore } from '../../store/backgroundTasks';
import { api, silentRequestConfig } from '../core/httpClient';
import { upsertChapterAnalysisTaskToStore } from './chapterTaskState';
import type {
  AnalysisTask,
  ApplyAutoRevisionDraftRequest,
  ApplyAutoRevisionDraftResponse,
  ApplyCandidateDraftRequest,
  ApplyCandidateDraftResponse,
  BatchAnalysisStatusResponse,
  ChapterAnalysisResponse,
  ChapterAutoRevisionDraftResponse,
  ChapterCandidateDraftResponse,
  TriggerAnalysisResponse,
} from '../../types';

const formatChapterAnalysisError = (
  errorCode?: AnalysisTask['error_code'],
  errorMessage?: string | null,
): string | null => {
  if (!errorCode && !errorMessage) {
    return null;
  }

  if (errorCode === 'json_parse_failed') {
    return 'Chapter analysis failed: invalid AI response format';
  }

  if (errorCode === 'ai_empty') {
    return 'Chapter analysis failed: empty AI response';
  }

  if (errorCode === 'stream_interrupted') {
    return 'Chapter analysis failed: stream interrupted';
  }

  if (errorCode === 'timeout') {
    return 'Chapter analysis timed out';
  }

  if (errorCode === 'chapter_empty') {
    return 'Chapter analysis failed: chapter content is empty';
  }

  if (errorCode === 'project_missing') {
    return 'Chapter analysis failed: project not found';
  }

  if (errorCode === 'retrying') {
    return errorMessage ?? null;
  }

  return errorMessage ?? null;
};

export const chapterAnalysisApi = {
  upsertChapterAnalysisTaskToStore: (
    task: AnalysisTask,
    projectId?: string,
    messageOverride?: string,
  ) => upsertChapterAnalysisTaskToStore(task, projectId, messageOverride),

  getChapterAnalysis: (chapterId: string, includeFullDraft = false) =>
    api.get<unknown, ChapterAnalysisResponse>(
      `/chapters/${chapterId}/analysis`,
      { params: { include_full_draft: includeFullDraft } },
    ),

  getChapterAnalysisStatus: async (chapterId: string, projectId?: string) => {
    const status = await api.get<unknown, AnalysisTask>(
      `/chapters/${chapterId}/analysis/status`,
      silentRequestConfig(),
    );
    status.error_message = formatChapterAnalysisError(status.error_code, status.error_message);
    upsertChapterAnalysisTaskToStore(status, projectId);
    return status;
  },

  getBatchChapterAnalysisStatus: async (chapterIds: string[], projectId?: string) => {
    const response = await api.post<unknown, BatchAnalysisStatusResponse>(
      '/chapters/analysis/status/batch',
      { chapter_ids: chapterIds },
      silentRequestConfig(),
    );
    Object.values(response.items).forEach((status) => {
      status.error_message = formatChapterAnalysisError(status.error_code, status.error_message);
      upsertChapterAnalysisTaskToStore(status, projectId);
    });
    return response;
  },

  triggerChapterAnalysis: async (chapterId: string, projectId?: string) => {
    const created = await api.post<unknown, TriggerAnalysisResponse>(`/chapters/${chapterId}/analyze`);
    useBackgroundTaskStore.getState().upsertTask({
      task_id: created.task_id,
      task_type: 'chapter_analysis',
      project_id: projectId,
      status: 'pending',
      progress: 0,
      message: created.message || 'Chapter analysis task created',
      stage_code: 'analysis',
      execution_mode: 'interactive',
      checkpoint: { chapter_id: chapterId },
      created_at: new Date().toISOString(),
      updated_at: new Date().toISOString(),
    });
    return created;
  },

  getAutoRevisionDraft: (chapterId: string, historyId?: string) =>
    api.get<unknown, ChapterAutoRevisionDraftResponse>(
      `/chapters/${chapterId}/analysis/auto-revision-draft`,
      { params: { history_id: historyId } },
    ),

  applyAutoRevisionDraft: (
    chapterId: string,
    data: ApplyAutoRevisionDraftRequest = {},
  ) =>
    api.post<unknown, ApplyAutoRevisionDraftResponse>(
      `/chapters/${chapterId}/analysis/auto-revision-draft/apply`,
      data,
    ),

  getCandidateDraft: (chapterId: string, attemptId?: string) =>
    api.get<unknown, ChapterCandidateDraftResponse>(
      `/chapters/${chapterId}/analysis/candidate-draft`,
      { params: { attempt_id: attemptId } },
    ),

  applyCandidateDraft: (
    chapterId: string,
    data: ApplyCandidateDraftRequest = {},
  ) =>
    api.post<unknown, ApplyCandidateDraftResponse>(
      `/chapters/${chapterId}/analysis/candidate-draft/apply`,
      data,
    ),
};