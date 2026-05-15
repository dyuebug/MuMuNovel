import { message } from 'antd';
import { useStore } from './index';
import type { PaginationResponse } from '../types';

export type ProjectCollectionKey = 'characters' | 'outlines' | 'chapters';

export interface RefreshCollectionOptions {
  silent?: boolean;
}

export type ProjectCollectionRefreshMap<T> = Map<string, Promise<T[]>>;

const collectionLoadedAt: Record<ProjectCollectionKey, Map<string, number>> = {
  characters: new Map<string, number>(),
  outlines: new Map<string, number>(),
  chapters: new Map<string, number>(),
};

const resolveProjectId = (projectId?: string) => projectId || useStore.getState().currentProject?.id;

const markCollectionLoaded = (collection: ProjectCollectionKey, projectId: string) => {
  collectionLoadedAt[collection].set(projectId, Date.now());
};

const shouldApplyCollectionToCurrentProject = (projectId: string) =>
  useStore.getState().currentProject?.id === projectId;

const normalizeCollectionItems = <T>(data: T[] | PaginationResponse<T>): T[] =>
  Array.isArray(data) ? data : data.items || [];

export const isProjectCollectionFresh = (
  collection: ProjectCollectionKey,
  projectId: string,
  maxAgeMs = 5000,
) => {
  const loadedAt = collectionLoadedAt[collection].get(projectId);
  return typeof loadedAt === 'number' && Date.now() - loadedAt <= maxAgeMs;
};

export const invalidateProjectCollectionFreshness = (
  collection: ProjectCollectionKey,
  projectId?: string,
) => {
  if (!projectId) {
    return;
  }

  collectionLoadedAt[collection].delete(projectId);
};

export const invalidateAllProjectCollectionFreshness = (projectId?: string) => {
  if (!projectId) {
    return;
  }

  invalidateProjectCollectionFreshness('characters', projectId);
  invalidateProjectCollectionFreshness('outlines', projectId);
  invalidateProjectCollectionFreshness('chapters', projectId);
};

export async function loadProjectCollection<T>({
  projectId,
  options = {},
  refreshPromises,
  collection,
  request,
  updateStore,
  errorLogLabel,
  errorMessage,
}: {
  projectId?: string;
  options?: RefreshCollectionOptions;
  refreshPromises: ProjectCollectionRefreshMap<T>;
  collection: ProjectCollectionKey;
  request: (projectId: string) => Promise<T[] | PaginationResponse<T>>;
  updateStore: (items: T[]) => void;
  errorLogLabel: string;
  errorMessage: string;
}): Promise<T[]> {
  const id = resolveProjectId(projectId);
  if (!id) return [];

  const existingRefresh = refreshPromises.get(id);
  if (existingRefresh) {
    return existingRefresh;
  }

  const refreshPromise = (async () => {
    try {
      const data = await request(id);
      const items = normalizeCollectionItems(data);
      const appliedToCurrentProject = shouldApplyCollectionToCurrentProject(id);
      if (appliedToCurrentProject) {
        updateStore(items);
      }
      if (appliedToCurrentProject) {
        markCollectionLoaded(collection, id);
      }
      return items;
    } catch (error) {
      console.error(errorLogLabel, error);
      if (!options.silent) {
        message.error(errorMessage);
      }
      return [];
    }
  })();

  refreshPromises.set(id, refreshPromise);

  try {
    return await refreshPromise;
  } finally {
    refreshPromises.delete(id);
  }
}
