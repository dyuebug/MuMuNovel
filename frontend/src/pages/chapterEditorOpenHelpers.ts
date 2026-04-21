import type { Chapter, ChapterQualityMetrics } from '../types';

type ChapterEditorFormValues = Pick<Chapter, 'title' | 'content'>;

export function openChapterEditor({
  chapter,
  editorForm,
  setCurrentChapter,
  resetSingleStoryCreationCockpit,
  setEditingId,
  setIsEditorOpen,
  setChapterQualityMetrics,
  loadAvailableModels,
}: {
  chapter: Chapter;
  editorForm: {
    setFieldsValue: (values: ChapterEditorFormValues) => void;
  };
  setCurrentChapter: (chapter: Chapter) => void;
  resetSingleStoryCreationCockpit: (chapterNumber?: number | null) => void;
  setEditingId: (id: string | null) => void;
  setIsEditorOpen: (open: boolean) => void;
  setChapterQualityMetrics: (metrics: ChapterQualityMetrics | null) => void;
  loadAvailableModels: () => Promise<unknown> | void;
}): void {
  setCurrentChapter(chapter);
  editorForm.setFieldsValue({
    title: chapter.title,
    content: chapter.content,
  });
  resetSingleStoryCreationCockpit(chapter.chapter_number);
  setEditingId(chapter.id);
  setIsEditorOpen(true);
  setChapterQualityMetrics(null);
  void loadAvailableModels();
}

export function openChapterEditorWorkflow({
  chapterId,
  chapters,
  editorForm,
  setCurrentChapter,
  resetSingleStoryCreationCockpit,
  setEditingId,
  setIsEditorOpen,
  setChapterQualityMetrics,
  loadAvailableModels,
}: {
  chapterId: string;
  chapters: Chapter[];
  editorForm: {
    setFieldsValue: (values: ChapterEditorFormValues) => void;
  };
  setCurrentChapter: (chapter: Chapter) => void;
  resetSingleStoryCreationCockpit: (chapterNumber?: number | null) => void;
  setEditingId: (id: string | null) => void;
  setIsEditorOpen: (open: boolean) => void;
  setChapterQualityMetrics: (metrics: ChapterQualityMetrics | null) => void;
  loadAvailableModels: () => Promise<unknown> | void;
}): void {
  const chapter = chapters.find((item) => item.id === chapterId);
  if (!chapter) {
    return;
  }

  openChapterEditor({
    chapter,
    editorForm,
    setCurrentChapter,
    resetSingleStoryCreationCockpit,
    setEditingId,
    setIsEditorOpen,
    setChapterQualityMetrics,
    loadAvailableModels,
  });
}