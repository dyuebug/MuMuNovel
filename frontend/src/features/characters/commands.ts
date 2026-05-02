import { useCallback } from 'react';
import { characterApi } from '../../services/modularApi';
import type { Character, CharacterUpdate, GenerateCharacterRequest } from '../../types';
import { useStore } from '../../store';
import { useEntityCrudSync } from '../../store/entityCrudSyncHooks';
import { runStoreMutation } from '../../store/storeMutationHelpers';
import { loadProjectCharacters } from './queries';

type CharacterCreatePayload = Parameters<typeof characterApi.createCharacter>[0];

export function useCharacterCommands() {
  const addCharacter = useStore((state) => state.addCharacter);
  const updateCharacter = useStore((state) => state.updateCharacter);
  const removeCharacter = useStore((state) => state.removeCharacter);

  const {
    createItem: createCharacter,
    updateItem: updateCharacterSync,
    deleteItem: deleteCharacter,
  } = useEntityCrudSync<Character, CharacterCreatePayload, CharacterUpdate>({
    refresh: loadProjectCharacters,
    create: (data) => characterApi.createCharacter(data),
    update: (id, data) => characterApi.updateCharacter(id, data),
    remove: (id) => characterApi.deleteCharacter(id),
    addToStore: addCharacter,
    updateInStore: updateCharacter,
    removeFromStore: removeCharacter,
    createErrorLabel: 'Failed to create character:',
    updateErrorLabel: 'Failed to update character:',
    deleteErrorLabel: 'Failed to delete character:',
  });

  const generateCharacter = useCallback(async (data: GenerateCharacterRequest) => {
    return runStoreMutation({
      request: () => characterApi.generateCharacter(data),
      onSuccess: addCharacter,
      errorLogLabel: 'AI character generation failed:',
    });
  }, [addCharacter]);

  return {
    createCharacter,
    updateCharacter: updateCharacterSync,
    deleteCharacter,
    generateCharacter,
  };
}
