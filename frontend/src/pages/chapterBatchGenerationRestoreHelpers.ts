import { message } from 'antd';
import { chapterBatchTaskApi } from '../services/modularApi';
import type {
  ActiveStoryRepairPayload,
  ChapterLatestQualityMetrics,
  ChapterQualityMetricsSummary,
  ChapterQualityProfileSummary,
} from '../types';
import type {
  BatchGenerationCheckpointLike,
  BatchProgressState,
  BatchTaskMeta,
} from './chapterBatchGenerationPollingHelpers';

const batchTaskRestorePromises = new Map<string, Promise<void>>();

type RestorableBatchTask = {
  batch_id: string;
  status: string;
  total: number;
  completed: number;
  current_chapter_number?: number | null;
  checkpoint?: unknown;
  latest_quality_metrics?: ChapterLatestQualityMetrics | null;
  quality_metrics_summary?: ChapterQualityMetricsSummary | null;
  quality_profile_summary?: ChapterQualityProfileSummary | null;
  active_story_repair_payload?: ActiveStoryRepairPayload | null;
};

export function createRestoredBatchProgressState({
  task,
  normalizeBatchGenerationCheckpoint,
}: {
  task: RestorableBatchTask;
  normalizeBatchGenerationCheckpoint: (value: unknown) => BatchGenerationCheckpointLike | null;
}): BatchProgressState {
  return {
    status: task.status,
    total: task.total,
    completed: task.completed,
    current_chapter_number: task.current_chapter_number ?? null,
    checkpoint: normalizeBatchGenerationCheckpoint(task.checkpoint),
    latest_quality_metrics: task.latest_quality_metrics ?? undefined,
    quality_metrics_summary: task.quality_metrics_summary ?? undefined,
    quality_profile_summary: task.quality_profile_summary ?? null,
    active_story_repair_payload: task.active_story_repair_payload ?? null,
  };
}

export async function restoreActiveBatchGenerationTask({
  projectId,
  getPersistedTaskMeta,
  rememberTaskMeta,
  setBatchTaskId,
  setBatchProgress,
  setBatchGenerating,
  setBatchGenerateVisible,
  startBatchPolling,
  normalizeBatchGenerationCheckpoint,
}: {
  projectId: string;
  getPersistedTaskMeta: (taskId: string, projectId?: string) => BatchTaskMeta | undefined;
  rememberTaskMeta: (taskId: string, meta: BatchTaskMeta) => void;
  setBatchTaskId: (taskId: string | null) => void;
  setBatchProgress: (progress: BatchProgressState | null) => void;
  setBatchGenerating: (value: boolean) => void;
  setBatchGenerateVisible: (value: boolean) => void;
  startBatchPolling: (taskId: string) => void;
  normalizeBatchGenerationCheckpoint: (value: unknown) => BatchGenerationCheckpointLike | null;
}): Promise<void> {
  const existingPromise = batchTaskRestorePromises.get(projectId);
  if (existingPromise) {
    await existingPromise;
    return;
  }

  const restorePromise = (async () => {
    try {
      const data = await chapterBatchTaskApi.getActiveBatchGenerateTask(projectId);
      if (!data.has_active_task || !data.task) {
        return;
      }

      const task = data.task as RestorableBatchTask;
      const persistedTaskMeta = getPersistedTaskMeta(task.batch_id, projectId);
      if (persistedTaskMeta) {
        rememberTaskMeta(task.batch_id, persistedTaskMeta);
      }

      setBatchTaskId(task.batch_id);
      setBatchProgress(createRestoredBatchProgressState({
        task,
        normalizeBatchGenerationCheckpoint,
      }));
      setBatchGenerating(true);
      setBatchGenerateVisible(false);
      startBatchPolling(task.batch_id);
      message.info('Restored the active batch generation task.');
    } catch (error) {
      console.error('Failed to restore batch task.', error);
    }
  })();

  batchTaskRestorePromises.set(projectId, restorePromise);
  try {
    await restorePromise;
  } finally {
    batchTaskRestorePromises.delete(projectId);
  }
}
