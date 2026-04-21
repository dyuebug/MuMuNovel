import type { Chapter } from '../types';
import type { ChapterPlanEditorData } from './chapterPlanEditorDataHelpers';

export type ChapterPlanEditorModalState = {
  visible: boolean;
  planData: ChapterPlanEditorData['planData'];
  chapterSummary: string | null;
  projectId: string;
};

export function buildChapterPlanEditorModalState({
  planEditorVisible,
  editingPlanChapter,
  editingPlanEditorData,
  currentProjectId,
}: {
  planEditorVisible: boolean;
  editingPlanChapter: Chapter | null;
  editingPlanEditorData: ChapterPlanEditorData | null;
  currentProjectId?: string;
}): ChapterPlanEditorModalState | null {
  if (!planEditorVisible || !editingPlanChapter || !currentProjectId) {
    return null;
  }

  return {
    visible: planEditorVisible,
    planData: editingPlanEditorData?.planData ?? null,
    chapterSummary: editingPlanEditorData?.chapterSummary ?? null,
    projectId: currentProjectId,
  };
}