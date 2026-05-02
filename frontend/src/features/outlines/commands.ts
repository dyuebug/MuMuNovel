import { useCallback } from 'react';
import { outlineApi } from '../../services/modularApi';
import type { GenerateOutlineRequest, Outline, OutlineCreate, OutlineUpdate } from '../../types';
import { useStore } from '../../store';
import { useEntityCrudSync } from '../../store/entityCrudSyncHooks';
import { normalizeStoreItems, runStoreMutation } from '../../store/storeMutationHelpers';
import { loadProjectOutlines } from './queries';

export function useOutlineCommands() {
  const addOutline = useStore((state) => state.addOutline);
  const updateOutline = useStore((state) => state.updateOutline);
  const removeOutline = useStore((state) => state.removeOutline);

  const {
    createItem: createOutline,
    updateItem: updateOutlineSync,
    deleteItem: deleteOutline,
  } = useEntityCrudSync<Outline, OutlineCreate, OutlineUpdate>({
    refresh: loadProjectOutlines,
    create: (data) => outlineApi.createOutline(data),
    update: (id, data) => outlineApi.updateOutline(id, data),
    remove: (id) => outlineApi.deleteOutline(id),
    addToStore: addOutline,
    updateInStore: updateOutline,
    removeFromStore: removeOutline,
    createErrorLabel: 'Failed to create outline:',
    updateErrorLabel: 'Failed to update outline:',
    deleteErrorLabel: 'Failed to delete outline:',
  });

  const generateOutlines = useCallback(async (data: GenerateOutlineRequest) => {
    return runStoreMutation({
      request: async () => normalizeStoreItems<Outline>(await outlineApi.generateOutline(data)),
      onSuccess: (outlines) => {
        outlines.forEach((outline) => addOutline(outline));
      },
      errorLogLabel: 'AI outline generation failed:',
    });
  }, [addOutline]);

  return {
    createOutline,
    updateOutline: updateOutlineSync,
    deleteOutline,
    generateOutlines,
  };
}
