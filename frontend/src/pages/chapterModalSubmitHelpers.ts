import { message } from 'antd';
import type { ChapterUpdate } from '../types';

export async function submitChapterModalUpdate({
  editingId,
  values,
  updateChapter,
  refreshChapters,
  setIsModalOpen,
  form,
}: {
  editingId: string;
  values: ChapterUpdate;
  updateChapter: (id: string, values: ChapterUpdate) => Promise<unknown>;
  refreshChapters: () => Promise<unknown> | void;
  setIsModalOpen: (open: boolean) => void;
  form: {
    resetFields: () => void;
  };
}): Promise<void> {
  try {
    await updateChapter(editingId, values);
    await refreshChapters();
    message.success('Chapter updated successfully.');
    setIsModalOpen(false);
    form.resetFields();
  } catch {
    message.error('Failed to update chapter.');
  }
}

export async function submitChapterModalWorkflow({
  editingId,
  values,
  updateChapter,
  refreshChapters,
  setIsModalOpen,
  form,
}: {
  editingId: string | null;
  values: ChapterUpdate;
  updateChapter: (id: string, values: ChapterUpdate) => Promise<unknown>;
  refreshChapters: () => Promise<unknown> | void;
  setIsModalOpen: (open: boolean) => void;
  form: {
    resetFields: () => void;
  };
}): Promise<void> {
  if (!editingId) {
    return;
  }

  await submitChapterModalUpdate({
    editingId,
    values,
    updateChapter,
    refreshChapters,
    setIsModalOpen,
    form,
  });
}