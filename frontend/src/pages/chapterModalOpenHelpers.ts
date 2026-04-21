import type { Chapter } from '../types';

type ChapterModalFormValues = Pick<Chapter, 'title' | 'chapter_number' | 'status'>;

export function openChapterModal({
  chapter,
  form,
  setEditingId,
  setIsModalOpen,
}: {
  chapter: Chapter;
  form: {
    setFieldsValue: (values: ChapterModalFormValues) => void;
  };
  setEditingId: (id: string | null) => void;
  setIsModalOpen: (open: boolean) => void;
}): void {
  form.setFieldsValue({
    title: chapter.title,
    chapter_number: chapter.chapter_number,
    status: chapter.status,
  });
  setEditingId(chapter.id);
  setIsModalOpen(true);
}

export function openChapterModalWorkflow({
  chapterId,
  chapters,
  form,
  setEditingId,
  setIsModalOpen,
}: {
  chapterId: string;
  chapters: Chapter[];
  form: {
    setFieldsValue: (values: ChapterModalFormValues) => void;
  };
  setEditingId: (id: string | null) => void;
  setIsModalOpen: (open: boolean) => void;
}): void {
  const chapter = chapters.find((item) => item.id === chapterId);
  if (!chapter) {
    return;
  }

  openChapterModal({
    chapter,
    form,
    setEditingId,
    setIsModalOpen,
  });
}