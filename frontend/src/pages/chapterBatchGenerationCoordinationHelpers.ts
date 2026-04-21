import type { MutableRefObject } from 'react';
import type { Chapter } from '../types';
import {
  startBatchGenerationPolling,
  type BatchGenerationCheckpointLike,
  type BatchProgressState,
  type BatchTaskMeta,
} from './chapterBatchGenerationPollingHelpers';
import { restoreActiveBatchGenerationTask } from './chapterBatchGenerationRestoreHelpers';

export function startBatchPollingWorkflow({
  taskId,
  projectId,
  projectTitle,
  batchPollingIntervalRef,
  batchTaskMetaRef,
  normalizeBatchGenerationCheckpoint,
  refreshChapters,
  loadAnalysisTasks,
  reloadCurrentProject,
  setBatchProgress,
  setBatchGenerating,
  getPersistedTaskMeta,
  removePersistedTaskMeta,
  triggerDeferredBatchAnalysis,
  showBrowserNotification,
  setBatchGenerateVisible,
  setBatchTaskId,
}: {
  taskId: string;
  projectId?: string;
  projectTitle?: string;
  batchPollingIntervalRef: MutableRefObject<number | null>;
  batchTaskMetaRef: MutableRefObject<Record<string, BatchTaskMeta>>;
  normalizeBatchGenerationCheckpoint: (value: unknown) => BatchGenerationCheckpointLike | null;
  refreshChapters: () => Promise<Chapter[]>;
  loadAnalysisTasks: (chaptersToLoad?: Chapter[]) => Promise<void>;
  reloadCurrentProject: () => Promise<void>;
  setBatchProgress: (progress: BatchProgressState | null) => void;
  setBatchGenerating: (value: boolean) => void;
  getPersistedTaskMeta: (taskId: string, projectId?: string) => BatchTaskMeta | undefined;
  removePersistedTaskMeta: (taskId: string) => void;
  triggerDeferredBatchAnalysis: (startChapterNumber: number, count: number, latestChapters: Chapter[]) => Promise<void> | void;
  showBrowserNotification: (title: string, body: string, type?: 'success' | 'error' | 'info') => void;
  setBatchGenerateVisible: (value: boolean) => void;
  setBatchTaskId: (taskId: string | null) => void;
}): number {
  return startBatchGenerationPolling({
    taskId,
    projectId,
    projectTitle,
    existingIntervalId: batchPollingIntervalRef.current,
    setIntervalRef: (intervalId) => {
      batchPollingIntervalRef.current = intervalId;
    },
    normalizeBatchGenerationCheckpoint,
    refreshChapters,
    loadAnalysisTasks,
    reloadCurrentProject,
    setBatchProgress,
    setBatchGenerating,
    resolveTaskMeta: (targetTaskId, targetProjectId) => (
      batchTaskMetaRef.current[targetTaskId] ?? getPersistedTaskMeta(targetTaskId, targetProjectId)
    ),
    removeTaskMeta: (targetTaskId) => {
      delete batchTaskMetaRef.current[targetTaskId];
      removePersistedTaskMeta(targetTaskId);
    },
    triggerDeferredBatchAnalysis,
    showBrowserNotification,
    closeBatchUi: () => {
      setBatchGenerateVisible(false);
      setBatchTaskId(null);
      setBatchProgress(null);
    },
  });
}

export async function restoreBatchGenerationWorkflow({
  projectId,
  batchTaskMetaRef,
  getPersistedTaskMeta,
  setBatchTaskId,
  setBatchProgress,
  setBatchGenerating,
  setBatchGenerateVisible,
  startBatchPolling,
  normalizeBatchGenerationCheckpoint,
}: {
  projectId?: string;
  batchTaskMetaRef: MutableRefObject<Record<string, BatchTaskMeta>>;
  getPersistedTaskMeta: (taskId: string, projectId?: string) => BatchTaskMeta | undefined;
  setBatchTaskId: (taskId: string | null) => void;
  setBatchProgress: (progress: BatchProgressState | null) => void;
  setBatchGenerating: (value: boolean) => void;
  setBatchGenerateVisible: (value: boolean) => void;
  startBatchPolling: (taskId: string) => void;
  normalizeBatchGenerationCheckpoint: (value: unknown) => BatchGenerationCheckpointLike | null;
}): Promise<void> {
  if (!projectId) {
    return;
  }

  await restoreActiveBatchGenerationTask({
    projectId,
    getPersistedTaskMeta,
    rememberTaskMeta: (taskId, meta) => {
      batchTaskMetaRef.current[taskId] = meta;
    },
    setBatchTaskId,
    setBatchProgress,
    setBatchGenerating,
    setBatchGenerateVisible,
    startBatchPolling,
    normalizeBatchGenerationCheckpoint,
  });
}