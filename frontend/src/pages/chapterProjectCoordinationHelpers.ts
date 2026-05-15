import { projectApi } from '../services/modularApi';
import type { AnalysisTask, Project } from '../types';

export function initializeChapterProjectWorkflow({
  projectId,
  currentProjectIdRef,
  stopAnalysisPolling,
  updateAnalysisTasksMap,
  chapterAnalysisTasksCache,
  chapterCount,
  refreshChapters,
  loadWritingStyles,
  loadAnalysisTasks,
  checkAndRestoreBatchTask,
}: {
  projectId: string | null;
  currentProjectIdRef: { current: string | null };
  stopAnalysisPolling: (clearTrackedChapterIds?: boolean) => void;
  updateAnalysisTasksMap: (next: Record<string, AnalysisTask>) => void;
  chapterAnalysisTasksCache: Map<string, Record<string, AnalysisTask>>;
  chapterCount: number;
  refreshChapters: () => Promise<unknown> | unknown;
  loadWritingStyles: () => Promise<void> | void;
  loadAnalysisTasks: () => Promise<void> | void;
  checkAndRestoreBatchTask: () => Promise<void> | void;
}): void {
  currentProjectIdRef.current = projectId;
  stopAnalysisPolling();
  updateAnalysisTasksMap(projectId ? (chapterAnalysisTasksCache.get(projectId) ?? {}) : {});

  if (!projectId) {
    return;
  }

  if (chapterCount === 0) {
    void refreshChapters();
  }

  void loadWritingStyles();
  void loadAnalysisTasks();
  void checkAndRestoreBatchTask();
}

export async function reloadChapterProjectWorkflow({
  projectId,
  isPageActiveRef,
  currentProjectIdRef,
  setCurrentProject,
}: {
  projectId?: string | null;
  isPageActiveRef?: { current: boolean };
  currentProjectIdRef?: { current: string | null };
  setCurrentProject: (project: Project | null) => void;
}): Promise<void> {
  if (!projectId) {
    return;
  }

  if (isPageActiveRef && !isPageActiveRef.current) {
    return;
  }

  const updatedProject = await projectApi.getProject(projectId);

  if ((isPageActiveRef && !isPageActiveRef.current) || (currentProjectIdRef && currentProjectIdRef.current !== projectId)) {
    return;
  }

  setCurrentProject(updatedProject);
}

export async function deleteChapterWithRefreshWorkflow({
  chapterId,
  deleteChapter,
  refreshChapters,
  reloadCurrentProject,
  onSuccess,
  onError,
}: {
  chapterId: string;
  deleteChapter: (chapterId: string) => Promise<unknown>;
  refreshChapters: () => Promise<unknown> | unknown;
  reloadCurrentProject: () => Promise<void>;
  onSuccess: () => void;
  onError: (error: Error) => void;
}): Promise<void> {
  try {
    await deleteChapter(chapterId);
    await refreshChapters();
    await reloadCurrentProject();
    onSuccess();
  } catch (error) {
    onError(error as Error);
  }
}
