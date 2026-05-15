import { message } from 'antd';
import { chapterApi } from '../services/modularApi';
import type { AnalysisTask } from '../types';

export function stopChapterAnalysisPolling({
  analysisPollingIntervalRef,
  pollingIntervalsRef,
  clearTrackedChapterIds = true,
}: {
  analysisPollingIntervalRef: { current: number | null };
  pollingIntervalsRef: { current: Set<string> };
  clearTrackedChapterIds?: boolean;
}): void {
  if (analysisPollingIntervalRef.current) {
    clearInterval(analysisPollingIntervalRef.current);
    analysisPollingIntervalRef.current = null;
  }

  if (clearTrackedChapterIds) {
    pollingIntervalsRef.current.clear();
  }
}

export function syncChapterAnalysisTasksFromBatch({
  items,
  analysisTasksMapRef,
  updateAnalysisTasksMap,
  areAnalysisTaskSnapshotsEqual,
  notifyOnTerminalTransitions = false,
  reset = false,
}: {
  items: Record<string, AnalysisTask>;
  analysisTasksMapRef: { current: Record<string, AnalysisTask> };
  updateAnalysisTasksMap: (next: Record<string, AnalysisTask>) => void;
  areAnalysisTaskSnapshotsEqual: (leftTask?: AnalysisTask | null, rightTask?: AnalysisTask | null) => boolean;
  notifyOnTerminalTransitions?: boolean;
  reset?: boolean;
}): Record<string, AnalysisTask> {
  const previousTasks = reset ? {} : analysisTasksMapRef.current;
  const nextTasks = { ...previousTasks };
  let changed = reset;

  Object.entries(items).forEach(([chapterId, task]) => {
    const previousTask = previousTasks[chapterId];
    const nextTask = areAnalysisTaskSnapshotsEqual(previousTask, task)
      ? previousTask
      : task;
    nextTasks[chapterId] = nextTask;

    if (nextTask !== previousTask) {
      changed = true;
    }

    if (notifyOnTerminalTransitions && previousTask?.status !== task.status) {
      if (task.status === 'completed') {
        message.success('Chapter analysis completed.');
      } else if (task.status === 'failed') {
        message.error('Chapter analysis failed: ' + (task.error_message || 'Unknown error'));
      }
    }
  });

  if (!changed) {
    return previousTasks;
  }

  updateAnalysisTasksMap(nextTasks);
  return nextTasks;
}

export async function pollChapterAnalysisTasksBatch({
  projectId,
  currentProjectIdRef,
  pollingIntervalsRef,
  analysisTasksMapRef,
  stopAnalysisPolling,
  updateAnalysisTasksMap,
  areAnalysisTaskSnapshotsEqual,
  isAnalysisTaskInProgress,
}: {
  projectId: string;
  currentProjectIdRef: { current: string | null };
  pollingIntervalsRef: { current: Set<string> };
  analysisTasksMapRef: { current: Record<string, AnalysisTask> };
  stopAnalysisPolling: (clearTrackedChapterIds?: boolean) => void;
  updateAnalysisTasksMap: (next: Record<string, AnalysisTask>) => void;
  areAnalysisTaskSnapshotsEqual: (leftTask?: AnalysisTask | null, rightTask?: AnalysisTask | null) => boolean;
  isAnalysisTaskInProgress: (task?: AnalysisTask | null) => boolean;
}): Promise<void> {
  const chapterIds = Array.from(pollingIntervalsRef.current);
  if (!projectId || chapterIds.length === 0) {
    stopAnalysisPolling(false);
    return;
  }

  try {
    const response = await chapterApi.getBatchChapterAnalysisStatus(chapterIds, projectId, {
      syncBackgroundTaskStore: false,
    });
    if (currentProjectIdRef.current !== projectId) {
      return;
    }

    syncChapterAnalysisTasksFromBatch({
      items: response.items,
      analysisTasksMapRef,
      updateAnalysisTasksMap,
      areAnalysisTaskSnapshotsEqual,
      notifyOnTerminalTransitions: true,
    });

    pollingIntervalsRef.current = new Set(
      chapterIds.filter((chapterId) => isAnalysisTaskInProgress(response.items[chapterId]))
    );

    if (pollingIntervalsRef.current.size === 0) {
      stopAnalysisPolling(false);
    }
  } catch (error) {
    console.error('Failed to poll analysis tasks.', error);
  }
}

export function ensureChapterAnalysisPolling({
  projectId,
  analysisPollingIntervalRef,
  pollingIntervalsRef,
  stopAnalysisPolling,
  pollAnalysisTasksBatch,
}: {
  projectId: string;
  analysisPollingIntervalRef: { current: number | null };
  pollingIntervalsRef: { current: Set<string> };
  stopAnalysisPolling: (clearTrackedChapterIds?: boolean) => void;
  pollAnalysisTasksBatch: (projectId: string) => Promise<void> | void;
}): void {
  if (!projectId || pollingIntervalsRef.current.size === 0) {
    stopAnalysisPolling(false);
    return;
  }

  if (analysisPollingIntervalRef.current) {
    return;
  }

  const poll = () => {
    void pollAnalysisTasksBatch(projectId);
  };

  poll();
  analysisPollingIntervalRef.current = window.setInterval(poll, 2000);
}

export function applyChapterAnalysisPollingState({
  projectId,
  tasksMap,
  pollingIntervalsRef,
  ensureAnalysisPolling,
  stopAnalysisPolling,
  collectActiveAnalysisChapterIds,
}: {
  projectId: string;
  tasksMap: Record<string, AnalysisTask>;
  pollingIntervalsRef: { current: Set<string> };
  ensureAnalysisPolling: (projectId: string) => void;
  stopAnalysisPolling: (clearTrackedChapterIds?: boolean) => void;
  collectActiveAnalysisChapterIds: (tasksMap: Record<string, AnalysisTask>) => string[];
}): void {
  pollingIntervalsRef.current = new Set(collectActiveAnalysisChapterIds(tasksMap));

  if (pollingIntervalsRef.current.size > 0) {
    ensureAnalysisPolling(projectId);
    return;
  }

  stopAnalysisPolling(false);
}
