import { useCallback } from 'react';
import { characterApi } from '../../services/modularApi';
import { useStore } from '../../store';
import {
  loadProjectCollection,
  type RefreshCollectionOptions,
} from '../../store/projectCollectionRefresh';
import type { Character } from '../../types';

const characterRefreshPromises = new Map<string, Promise<Character[]>>();

export async function loadProjectCharacters(projectId?: string, options: RefreshCollectionOptions = {}) {
  return loadProjectCollection<Character>({
    projectId,
    options,
    refreshPromises: characterRefreshPromises,
    collection: 'characters',
    request: (id) => characterApi.getCharacters(id),
    updateStore: (characters) => useStore.getState().setCharacters(characters),
    errorLogLabel: 'Failed to load characters:',
    errorMessage: 'Failed to load characters',
  });
}

export function useCharacterQueries() {
  const refreshCharacters = useCallback(async (projectId?: string) => {
    return loadProjectCharacters(projectId);
  }, []);

  return {
    refreshCharacters,
  };
}
