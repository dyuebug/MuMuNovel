import { useCallback } from 'react';
import { outlineApi } from '../../services/modularApi';
import { useStore } from '../../store';
import {
  loadProjectCollection,
  type RefreshCollectionOptions,
} from '../../store/projectCollectionRefresh';
import type { Outline } from '../../types';

const outlineRefreshPromises = new Map<string, Promise<Outline[]>>();

export async function loadProjectOutlines(projectId?: string, options: RefreshCollectionOptions = {}) {
  return loadProjectCollection<Outline>({
    projectId,
    options,
    refreshPromises: outlineRefreshPromises,
    collection: 'outlines',
    request: (id) => outlineApi.getOutlines(id),
    updateStore: (outlines) => useStore.getState().setOutlines(outlines),
    errorLogLabel: 'Failed to load outlines:',
    errorMessage: 'Failed to load outlines',
  });
}

export function useOutlineQueries() {
  const refreshOutlines = useCallback(async (projectId?: string) => {
    return loadProjectOutlines(projectId);
  }, []);

  return {
    refreshOutlines,
  };
}
