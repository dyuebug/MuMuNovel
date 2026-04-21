import { useCallback } from 'react';
import { runStoreMutation } from './storeMutationHelpers';

export interface EntityCrudSyncConfig<TItem, TCreateInput, TUpdateInput> {
  refresh: (projectId?: string) => Promise<TItem[]>;
  create: (data: TCreateInput) => Promise<TItem>;
  update: (id: string, data: TUpdateInput) => Promise<TItem>;
  remove: (id: string) => Promise<unknown>;
  addToStore: (item: TItem) => void;
  updateInStore: (id: string, item: TItem) => void;
  removeFromStore: (id: string) => void;
  createErrorLabel: string;
  updateErrorLabel: string;
  deleteErrorLabel: string;
}

export function useEntityCrudSync<TItem, TCreateInput, TUpdateInput>({
  refresh,
  create,
  update,
  remove,
  addToStore,
  updateInStore,
  removeFromStore,
  createErrorLabel,
  updateErrorLabel,
  deleteErrorLabel,
}: EntityCrudSyncConfig<TItem, TCreateInput, TUpdateInput>) {
  const refreshItems = useCallback(async (projectId?: string) => {
    return refresh(projectId);
  }, [refresh]);

  const createItem = useCallback(async (data: TCreateInput) => {
    return runStoreMutation({
      request: () => create(data),
      onSuccess: addToStore,
      errorLogLabel: createErrorLabel,
    });
  }, [create, addToStore, createErrorLabel]);

  const updateItem = useCallback(async (id: string, data: TUpdateInput) => {
    return runStoreMutation({
      request: () => update(id, data),
      onSuccess: (updated) => updateInStore(id, updated),
      errorLogLabel: updateErrorLabel,
    });
  }, [update, updateInStore, updateErrorLabel]);

  const deleteItem = useCallback(async (id: string) => {
    return runStoreMutation({
      request: async () => {
        await remove(id);
        return id;
      },
      onSuccess: removeFromStore,
      errorLogLabel: deleteErrorLabel,
    });
  }, [remove, removeFromStore, deleteErrorLabel]);

  return {
    refreshItems,
    createItem,
    updateItem,
    deleteItem,
  };
}
