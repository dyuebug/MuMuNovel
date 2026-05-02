import { useCallback } from 'react';
import { projectApi } from '../../services/modularApi';
import { normalizeStoreItems } from '../../store/storeMutationHelpers';
import { refreshProjectsToStore } from '../../store/projectSyncHelpers';
import type { Project } from '../../types';

export function useProjectQueries() {
  const refreshProjects = useCallback(async () => {
    return refreshProjectsToStore();
  }, []);

  return {
    refreshProjects,
  };
}

export async function loadProjects() {
  const data = await projectApi.getProjects();
  return normalizeStoreItems<Project>(data);
}
