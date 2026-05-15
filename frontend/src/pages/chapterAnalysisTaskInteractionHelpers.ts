import { chapterApi } from '../services/modularApi';
import type { AnalysisTask } from '../types';

export function startChapterAnalysisPollingTask({
  chapterId,
  pollingIntervalsRef,
  currentProjectId,
  ensureAnalysisPolling,
}: {
  chapterId: string;
  pollingIntervalsRef: { current: Set<string> };
  currentProjectId?: string | null;
  ensureAnalysisPolling: (projectId: string) => void;
}): void {
  pollingIntervalsRef.current.add(chapterId);

  if (!currentProjectId) {
    return;
  }

  ensureAnalysisPolling(currentProjectId);
}

export async function refreshChapterAnalysisTaskStatus({
  chapterId,
  isPageActiveRef,
  currentProjectIdRef,
  currentProjectId,
  syncAnalysisTasksFromBatch,
  startPollingTask,
  pollingIntervalsRef,
  stopAnalysisPolling,
  isAnalysisTaskInProgress,
}: {
  chapterId: string;
  isPageActiveRef?: { current: boolean };
  currentProjectIdRef: { current: string | null };
  currentProjectId?: string | null;
  syncAnalysisTasksFromBatch: (items: Record<string, AnalysisTask>, options?: { notifyOnTerminalTransitions?: boolean; reset?: boolean }) => Record<string, AnalysisTask>;
  startPollingTask: (chapterId: string) => void;
  pollingIntervalsRef: { current: Set<string> };
  stopAnalysisPolling: (clearTrackedChapterIds?: boolean) => void;
  isAnalysisTaskInProgress: (task?: AnalysisTask | null) => boolean;
}): Promise<void> {
  const projectId = currentProjectIdRef.current ?? currentProjectId ?? null;
  if (!projectId) {
    return;
  }

  if (isPageActiveRef && !isPageActiveRef.current) {
    return;
  }

  const task = await chapterApi.getChapterAnalysisStatus(chapterId, projectId, {
    syncBackgroundTaskStore: false,
  });
  if ((isPageActiveRef && !isPageActiveRef.current) || currentProjectIdRef.current !== projectId) {
    return;
  }

  syncAnalysisTasksFromBatch({ [chapterId]: task }, { notifyOnTerminalTransitions: true });

  if (isAnalysisTaskInProgress(task)) {
    startPollingTask(chapterId);
    return;
  }

  pollingIntervalsRef.current.delete(chapterId);
  if (pollingIntervalsRef.current.size === 0) {
    stopAnalysisPolling(false);
  }
}
