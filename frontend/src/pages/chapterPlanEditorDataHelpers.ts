import type { Chapter, ExpansionPlanData } from '../types';

export type ChapterPlanEditorData = {
  chapterSummary: string | null;
  planData: ExpansionPlanData | null;
};

export function buildChapterPlanEditorData(
  chapter: Chapter | null,
): ChapterPlanEditorData | null {
  if (!chapter) {
    return null;
  }

  let planData: ExpansionPlanData | null = null;

  if (chapter.expansion_plan) {
    try {
      planData = JSON.parse(chapter.expansion_plan) as ExpansionPlanData;
    } catch (error) {
      console.error('Failed to parse expansion plan JSON.', error);
    }
  }

  return {
    chapterSummary: chapter.summary || null,
    planData,
  };
}