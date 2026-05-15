import { chapterApi } from '../services/modularApi';
import type { AnalysisTask, Chapter } from '../types';

export async function loadChapterAnalysisTasks({
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
  const targetProjectId = projectId ?? null;
  const targetChapters = chaptersToLoad || chapters;

  if (!targetProjectId) {
    stopAnalysisPolling();
    return;
  }

  if (isPageActiveRef && !isPageActiveRef.current) {
    return;
  }

  currentProjectIdRef.current = targetProjectId;

  if (!targetChapters || targetChapters.length === 0) {
    if (!chaptersToLoad) {
      updateAnalysisTasksMap({});
    }
    stopAnalysisPolling();
    return;
  }

  if (!chaptersToLoad) {
    const cachedTasks = chapterAnalysisTasksCache.get(targetProjectId);
    if (cachedTasks) {
      updateAnalysisTasksMap(cachedTasks);
      applyAnalysisPollingState(targetProjectId, cachedTasks);
      return;
    }
  }

  const targetChapterIds = targetChapters
    .filter((chapter) => chapter.content && chapter.content.trim() !== '')
    .map((chapter) => chapter.id);

  if (targetChapterIds.length === 0) {
    updateAnalysisTasksMap(chaptersToLoad ? { ...analysisTasksMapRef.current } : {});
    stopAnalysisPolling();
    return;
  }

  try {
    const response = await chapterApi.getBatchChapterAnalysisStatus(targetChapterIds, targetProjectId, {
      syncBackgroundTaskStore: false,
    });
    if ((isPageActiveRef && !isPageActiveRef.current) || currentProjectIdRef.current !== targetProjectId) {
      return;
    }

    const tasksMap = chaptersToLoad ? { ...analysisTasksMapRef.current } : {};
    let changed = !chaptersToLoad;
    targetChapterIds.forEach((chapterId) => {
      const task = response.items[chapterId];
      if (!task) {
        return;
      }

      const previousTask = analysisTasksMapRef.current[chapterId];
      const nextTask = areAnalysisTaskSnapshotsEqual(previousTask, task) ? previousTask : task;
      tasksMap[chapterId] = nextTask;
      if (nextTask !== previousTask) {
        changed = true;
      }
    });

    if (!changed) {
      applyAnalysisPollingState(targetProjectId, analysisTasksMapRef.current);
      return;
    }

    applyAnalysisPollingState(targetProjectId, tasksMap);
    updateAnalysisTasksMap(tasksMap);
  } catch (error) {
    console.error('Failed to load chapter analysis tasks.', error);
  }
}
