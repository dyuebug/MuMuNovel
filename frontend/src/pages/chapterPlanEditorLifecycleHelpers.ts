import { message } from 'antd';
import type { Chapter, ExpansionPlanData } from '../types';

export function openChapterPlanEditor({
  chapter,
  setEditingPlanChapter,
  setPlanEditorVisible,
}: {
  chapter: Chapter;
  setEditingPlanChapter: (chapter: Chapter | null) => void;
  setPlanEditorVisible: (visible: boolean) => void;
}): void {
  setEditingPlanChapter(chapter);
  setPlanEditorVisible(true);
}

export function closeChapterPlanEditor({
  setEditingPlanChapter,
  setPlanEditorVisible,
}: {
  setEditingPlanChapter: (chapter: Chapter | null) => void;
  setPlanEditorVisible: (visible: boolean) => void;
}): void {
  setPlanEditorVisible(false);
  setEditingPlanChapter(null);
}

export async function saveChapterPlan({
  chapterId,
  planData,
  refreshChapters,
  closePlanEditor,
}: {
  chapterId: string;
  planData: ExpansionPlanData;
  refreshChapters: () => Promise<unknown>;
  closePlanEditor: () => void;
}): Promise<void> {
  try {
    const response = await fetch(`/api/chapters/${chapterId}/expansion-plan`, {
      method: 'PUT',
      headers: {
        'Content-Type': 'application/json',
      },
      body: JSON.stringify(planData),
    });

    if (!response.ok) {
      const error = await response.json();
      throw new Error(error.detail || 'Save plan failed.');
    }

    await refreshChapters();
    message.success('Chapter plan saved.');
    closePlanEditor();
  } catch (error: unknown) {
    const err = error as Error;
    message.error('Save chapter plan failed: ' + (err.message || 'Unknown error'));
    throw error;
  }
}
