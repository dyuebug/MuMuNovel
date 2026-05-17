import type { AnalysisTask, Chapter } from '../types';
import { loadChapterAnalysisTasks } from './chapterAnalysisTaskLoadHelpers';
import {
  refreshChapterAnalysisTaskStatus,
  startChapterAnalysisPollingTask,
} from './chapterAnalysisTaskInteractionHelpers';

export async function loadAnalysisTasksWorkflow({
  projectId,
  chapters,
  chaptersToLoad,
  isPageActiveRef,
  currentProjectIdRef,
  analysisTasksMapRef,
  chapterAnalysisTasksCache,
  updateAnalysisTasksMap,
  applyAnalysisPollingState,
  stopAnalysisPolling,
  areAnalysisTaskSnapshotsEqual,
}: {
  projectId?: string | null;
  chapters: Chapter[];
  chaptersToLoad?: Chapter[];
  isPageActiveRef?: { current: boolean };
  currentProjectIdRef: { current: string | null };
  analysisTasksMapRef: { current: Record<string, AnalysisTask> };
  chapterAnalysisTasksCache: Map<string, Record<string, AnalysisTask>>;
  updateAnalysisTasksMap: (next: Record<string, AnalysisTask>) => void;
  applyAnalysisPollingState: (projectId: string, tasksMap: Record<string, AnalysisTask>) => void;
  stopAnalysisPolling: (clearTrackedChapterIds?: boolean) => void;
  areAnalysisTaskSnapshotsEqual: (leftTask?: AnalysisTask | null, rightTask?: AnalysisTask | null) => boolean;
}): Promise<void> {
  await loadChapterAnalysisTasks({
    projectId,
    chapters,
    chaptersToLoad,
    isPageActiveRef,
    currentProjectIdRef,
    analysisTasksMapRef,
    chapterAnalysisTasksCache,
    updateAnalysisTasksMap,
    applyAnalysisPollingState,
    stopAnalysisPolling,
    areAnalysisTaskSnapshotsEqual,
  });
}

export function startAnalysisPollingTaskWorkflow({
  chapterId,
  pollingIntervalsRef,
  currentProjectIdRef,
  currentProjectId,
  ensureAnalysisPolling,
}: {
  chapterId: string;
  pollingIntervalsRef: { current: Set<string> };
  currentProjectIdRef: { current: string | null };
  currentProjectId?: string | null;
  ensureAnalysisPolling: (projectId: string) => void;
}): void {
  startChapterAnalysisPollingTask({
    chapterId,
    pollingIntervalsRef,
    currentProjectId: currentProjectIdRef.current ?? currentProjectId,
    ensureAnalysisPolling,
  });
}

export async function refreshAnalysisTaskWorkflow({
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
  await refreshChapterAnalysisTaskStatus({
    chapterId,
    isPageActiveRef,
    currentProjectIdRef,
    currentProjectId,
    syncAnalysisTasksFromBatch,
    startPollingTask,
    pollingIntervalsRef,
    stopAnalysisPolling,
    isAnalysisTaskInProgress,
  });
}

export function closeAnalysisWorkflow({
  analysisChapterId,
  projectId,
  isPageActiveRef,
  currentProjectIdRef,
  setAnalysisVisible,
  refreshChapterAnalysisTask,
  setAnalysisChapterId,
}: {
  analysisChapterId?: string | null;
  projectId?: string;
  isPageActiveRef?: { current: boolean };
  currentProjectIdRef?: { current: string | null };
  setAnalysisVisible: (value: boolean) => void;
  refreshChapterAnalysisTask: (chapterId: string) => Promise<void>;
  setAnalysisChapterId: (value: string | null) => void;
}): void {
  setAnalysisVisible(false);
  const chapterIdToRefresh = analysisChapterId;
  setAnalysisChapterId(null);

  if (isPageActiveRef && !isPageActiveRef.current) {
    return;
  }

  const activeProjectId = currentProjectIdRef?.current ?? projectId ?? null;
  if (chapterIdToRefresh && (!activeProjectId || activeProjectId === projectId)) {
    void refreshChapterAnalysisTask(chapterIdToRefresh)
      .catch((error) => {
        if (
          (isPageActiveRef && !isPageActiveRef.current)
          || (projectId && currentProjectIdRef && currentProjectIdRef.current !== projectId)
        ) {
          return;
        }
        console.error('Failed to refresh chapter analysis after closing modal.', error);
      });
  }
}
