import { useCallback } from 'react';
import { chapterApi } from '../../services/modularApi';
import { useStore } from '../../store';
import {
  loadProjectCollection,
  type RefreshCollectionOptions,
} from '../../store/projectCollectionRefresh';
import type { Chapter } from '../../types';

const chapterRefreshPromises = new Map<string, Promise<Chapter[]>>();

export async function loadProjectChapters(projectId?: string, options: RefreshCollectionOptions = {}) {
  return loadProjectCollection<Chapter>({
    projectId,
    options,
    refreshPromises: chapterRefreshPromises,
    collection: 'chapters',
    request: (id) => chapterApi.getChapters(id),
    updateStore: (chapters) => useStore.getState().setChapters(chapters),
    errorLogLabel: 'Failed to load chapters:',
    errorMessage: 'Failed to load chapters',
  });
}

export function useChapterQueries() {
  const refreshChapters = useCallback(async (projectId?: string) => {
    return loadProjectChapters(projectId);
  }, []);

  return {
    refreshChapters,
  };
}
