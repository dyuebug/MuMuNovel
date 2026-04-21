import { chapterApi } from '../services/modularApi';
import type { AnalysisTask, Chapter } from '../types';

export async function loadChapterAnalysisTasks({
  projectId,
  chapters,
  chaptersToLoad,
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
    const response = await chapterApi.getBatchChapterAnalysisStatus(targetChapterIds, targetProjectId);
    if (currentProjectIdRef.current !== targetProjectId) {
      return;
    }

    const tasksMap = chaptersToLoad ? { ...analysisTasksMapRef.current } : {};
    targetChapterIds.forEach((chapterId) => {
      const task = response.items[chapterId];
      if (!task) {
        return;
      }

      const previousTask = analysisTasksMapRef.current[chapterId];
      tasksMap[chapterId] = areAnalysisTaskSnapshotsEqual(previousTask, task) ? previousTask : task;
    });

    applyAnalysisPollingState(targetProjectId, tasksMap);
    updateAnalysisTasksMap(tasksMap);
  } catch (error) {
    console.error('Failed to load chapter analysis tasks.', error);
  }
}
