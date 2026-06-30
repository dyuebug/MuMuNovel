import { useCallback } from 'react';
import { backgroundTaskApi, outlineApi } from '../../services/modularApi';
import type { GenerateOutlineRequest, Outline, OutlineCreate, OutlineUpdate } from '../../types';
import { useStore } from '../../store';
import { useEntityCrudSync } from '../../store/entityCrudSyncHooks';
import { runStoreMutation } from '../../store/storeMutationHelpers';
import { loadProjectOutlines } from './queries';
import { waitForBackgroundTaskCompletion } from '../../utils/taskPolling';

const normalizeGeneratedOutlines = (value: unknown): Outline[] => {
  if (Array.isArray(value)) return value as Outline[];
  if (value && typeof value === 'object' && Array.isArray((value as { items?: unknown }).items)) {
    return (value as { items: Outline[] }).items;
  }
  return [];
};

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
      request: async () => {
        const task = await backgroundTaskApi.createTask({
          task_type: 'outline_generate',
          project_id: data.project_id,
          payload: data as unknown as Record<string, unknown>,
        });
        const result = await waitForBackgroundTaskCompletion<typeof task, { items?: Outline[]; total?: number } | Outline[]>(task, {
          pollTask: backgroundTaskApi.getTaskStatus,
          progressMessage: '大纲生成任务已创建，正在后台执行',
        });
        return normalizeGeneratedOutlines(result);
      },
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
