import { useEntityCrudSync } from '../../store/entityCrudSyncHooks';
import type { Chapter, ChapterCreate, ChapterUpdate } from '../../types';
import { chapterApi } from '../../services/modularApi';
import { useStore } from '../../store';
import { loadProjectChapters } from './queries';

export function useChapterCommands() {
  const addChapter = useStore((state) => state.addChapter);
  const updateChapter = useStore((state) => state.updateChapter);
  const removeChapter = useStore((state) => state.removeChapter);

  const {
    createItem: createChapter,
    updateItem: updateChapterSync,
    deleteItem: deleteChapter,
  } = useEntityCrudSync<Chapter, ChapterCreate, ChapterUpdate>({
    refresh: loadProjectChapters,
    create: (data) => chapterApi.createChapter(data),
    update: (id, data) => chapterApi.updateChapter(id, data),
    remove: (id) => chapterApi.deleteChapter(id),
    addToStore: addChapter,
    updateInStore: updateChapter,
    removeFromStore: removeChapter,
    createErrorLabel: 'Failed to create chapter:',
    updateErrorLabel: 'Failed to update chapter:',
    deleteErrorLabel: 'Failed to delete chapter:',
  });

  return {
    createChapter,
    updateChapter: updateChapterSync,
    deleteChapter,
  };
}
