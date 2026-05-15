import { message } from 'antd';
import { chapterBatchTaskApi, getBatchManualReviewInfo } from '../services/modularApi';
import type {
  ActiveStoryRepairPayload,
  Chapter,
  ChapterLatestQualityMetrics,
  ChapterQualityMetricsSummary,
  ChapterQualityProfileSummary,
} from '../types';

export interface BatchTaskMeta {
  startChapterNumber: number;
  count: number;
  autoAnalyze: boolean;
  projectId?: string;
}

export interface BatchGenerationCheckpointCompactionDetail {
  before?: number | null;
  after?: number | null;
}

export interface BatchGenerationCheckpointLike {
  current_chapter_number?: number | null;
  candidate_index?: number | null;
  candidate_count?: number | null;
  word_count?: number | null;
  generation_path?: string | null;
  attempt_kind?: string | null;
  rerank_used?: boolean | null;
  word_budget_repair_used?: boolean | null;
  winner_candidate_index?: number | null;
  pre_compaction_total_length?: number | null;
  context_budget_limit?: number | null;
  compaction_applied?: boolean | null;
  compaction_details?: Record<string, BatchGenerationCheckpointCompactionDetail> | null;
}

export interface BatchProgressState {
  status: string;
  total: number;
  completed: number;
  current_chapter_number: number | null;
  progress_percent?: number;
  checkpoint?: BatchGenerationCheckpointLike | null;
  estimated_time_minutes?: number;
  latest_quality_metrics?: ChapterLatestQualityMetrics | null;
  quality_metrics_summary?: ChapterQualityMetricsSummary | null;
  quality_profile_summary?: ChapterQualityProfileSummary | null;
  failed_chapters?: Array<Record<string, unknown>>;
  active_story_repair_payload?: ActiveStoryRepairPayload | null;
}

export function startBatchGenerationPolling({
  taskId,
  projectId,
  projectTitle,
  existingIntervalId,
  existingCloseTimeoutId,
  setIntervalRef,
  setCloseTimeoutRef,
  normalizeBatchGenerationCheckpoint,
  refreshChapters,
  loadAnalysisTasks,
  reloadCurrentProject,
  setBatchProgress,
  setBatchGenerating,
  resolveTaskMeta,
  removeTaskMeta,
  triggerDeferredBatchAnalysis,
  showBrowserNotification,
  closeBatchUi,
  isPollingSessionActive,
}: {
  taskId: string;
  projectId?: string;
  projectTitle?: string;
  existingIntervalId?: number | null;
  existingCloseTimeoutId?: number | null;
  setIntervalRef: (intervalId: number | null) => void;
  setCloseTimeoutRef: (timeoutId: number | null) => void;
  normalizeBatchGenerationCheckpoint: (value: unknown) => BatchGenerationCheckpointLike | null;
  refreshChapters: () => Promise<Chapter[]>;
  loadAnalysisTasks: (chaptersToLoad?: Chapter[]) => Promise<void>;
  reloadCurrentProject: () => Promise<void>;
  setBatchProgress: (progress: BatchProgressState | null) => void;
  setBatchGenerating: (value: boolean) => void;
  resolveTaskMeta: (taskId: string, projectId?: string) => BatchTaskMeta | undefined;
  removeTaskMeta: (taskId: string) => void;
  triggerDeferredBatchAnalysis: (startChapterNumber: number, count: number, latestChapters: Chapter[]) => Promise<void> | void;
  showBrowserNotification: (title: string, body: string, type?: 'success' | 'error' | 'info') => void;
  closeBatchUi: () => void;
  isPollingSessionActive: () => boolean;
}) {
  const resolveBatchProgressPercent = (status: {
    status: string;
    total: number;
    completed: number;
    checkpoint?: { progress?: unknown } | null;
  }): number => {
    const checkpointProgress = status.checkpoint?.progress;
    if (typeof checkpointProgress === 'number' && Number.isFinite(checkpointProgress)) {
      return Math.max(0, Math.min(Math.round(checkpointProgress), 100));
    }

    const total = Math.max(status.total || 0, 0);
    const completed = Math.max(status.completed || 0, 0);
    if (status.status === 'completed') {
      return 100;
    }
    if (total <= 0) {
      return 0;
    }
    return Math.max(0, Math.min(Math.round((completed / total) * 100), 100));
  };

  let activeIntervalId = existingIntervalId ?? null;
  let activeCloseTimeoutId = existingCloseTimeoutId ?? null;
  let lastSyncedCompleted = -1;

  if (activeIntervalId) {
    window.clearInterval(activeIntervalId);
  }
  if (activeCloseTimeoutId) {
    window.clearTimeout(activeCloseTimeoutId);
    activeCloseTimeoutId = null;
    setCloseTimeoutRef(null);
  }

  const poll = async () => {
    if (!isPollingSessionActive()) {
      if (activeIntervalId) {
        window.clearInterval(activeIntervalId);
        activeIntervalId = null;
        setIntervalRef(null);
      }
      if (activeCloseTimeoutId) {
        window.clearTimeout(activeCloseTimeoutId);
        activeCloseTimeoutId = null;
        setCloseTimeoutRef(null);
      }
      return;
    }

    try {
      const status = await chapterBatchTaskApi.getBatchGenerateStatus(taskId, projectId);
      if (!isPollingSessionActive()) {
        return;
      }

      setBatchProgress({
        status: status.status,
        total: status.total,
        completed: status.completed,
        current_chapter_number: status.current_chapter_number ?? null,
        progress_percent: resolveBatchProgressPercent(status),
        checkpoint: normalizeBatchGenerationCheckpoint(status.checkpoint),
        latest_quality_metrics: (status.latest_quality_metrics as ChapterLatestQualityMetrics | null | undefined) ?? undefined,
        quality_metrics_summary: (status.quality_metrics_summary as ChapterQualityMetricsSummary | null | undefined) ?? undefined,
        quality_profile_summary: status.quality_profile_summary ?? null,
        failed_chapters: status.failed_chapters ?? [],
        active_story_repair_payload: status.active_story_repair_payload ?? null,
      });

      if (status.completed > 0 && status.completed !== lastSyncedCompleted) {
        lastSyncedCompleted = status.completed;
        const latestChapters = await refreshChapters();
        if (!isPollingSessionActive()) {
          return;
        }
        await loadAnalysisTasks(latestChapters);
        if (!isPollingSessionActive()) {
          return;
        }
        await reloadCurrentProject();
        if (!isPollingSessionActive()) {
          return;
        }
      }

      if (status.status === 'completed' || status.status === 'failed' || status.status === 'cancelled') {
        if (activeIntervalId) {
          window.clearInterval(activeIntervalId);
          activeIntervalId = null;
        }
        setIntervalRef(null);
        setBatchGenerating(false);

        const taskMeta = resolveTaskMeta(taskId, projectId);
        const finalChapters = await refreshChapters();
        if (!isPollingSessionActive()) {
          return;
        }
        await loadAnalysisTasks(finalChapters);
        if (!isPollingSessionActive()) {
          return;
        }
        await reloadCurrentProject();
        if (!isPollingSessionActive()) {
          return;
        }

        if (status.status === 'completed') {
          message.success(`批量生成完成，共生成 ${status.completed} 章。`);
          showBrowserNotification(
            '批量生成已完成',
            `项目“${projectTitle || '未命名项目'}”已完成 ${status.completed} 章生成。`,
            'success',
          );

          if (taskMeta?.autoAnalyze) {
            void triggerDeferredBatchAnalysis(taskMeta.startChapterNumber, taskMeta.count, finalChapters);
          }
        } else if (status.status === 'failed') {
          const manualReviewInfo = getBatchManualReviewInfo(
            status.failed_chapters,
            status.error_message,
            status.terminal_reason,
            status.terminal_label,
            status.review_required,
          );

          if (manualReviewInfo) {
            const manualReviewMessage = manualReviewInfo.failedMetrics.length > 0
              ? `${manualReviewInfo.message}（关注：${manualReviewInfo.failedMetrics.slice(0, 3).join('、')}）`
              : manualReviewInfo.message;
            message.warning(`批量生成需人工复核：${manualReviewMessage}`);
            showBrowserNotification('批量生成需人工复核', manualReviewMessage, 'info');
          } else {
            message.error(`批量生成失败：${status.error_message || '未知错误'}`);
            showBrowserNotification('批量生成失败', status.error_message || '未知错误', 'error');
          }
        } else if (status.status === 'cancelled') {
          message.warning('批量生成已取消。');
        }

        removeTaskMeta(taskId);
        activeCloseTimeoutId = window.setTimeout(() => {
          activeCloseTimeoutId = null;
          setCloseTimeoutRef(null);
          if (!isPollingSessionActive()) {
            return;
          }
          closeBatchUi();
        }, 2000);
        setCloseTimeoutRef(activeCloseTimeoutId);
      }
    } catch (error) {
      if (isPollingSessionActive()) {
        console.error('Failed to poll batch generate task.', error);
      }
    }
  };

  void poll();
  const intervalId = window.setInterval(poll, 2000);
  activeIntervalId = intervalId;
  setIntervalRef(intervalId);
  return intervalId;
}
