import { useCallback } from 'react';
import { useBackgroundTaskStore } from '../../store/backgroundTasks';
import { useStore } from '../../store';
import { projectApi } from '../../services/modularApi';
import type { ProjectCreate, ProjectUpdate } from '../../types';
import { runStoreMutation } from '../../store/storeMutationHelpers';

export function useProjectCommands() {
  const { addProject, updateProject, removeProject } = useStore();

  const createProject = useCallback(async (data: ProjectCreate) => {
    return runStoreMutation({
      request: () => projectApi.createProject(data),
      onSuccess: addProject,
      errorLogLabel: 'Failed to create project:',
    });
  }, [addProject]);

  const updateProjectSync = useCallback(async (id: string, data: ProjectUpdate) => {
    return runStoreMutation({
      request: () => projectApi.updateProject(id, data),
      onSuccess: (updated) => updateProject(id, updated),
      errorLogLabel: 'Failed to update project:',
    });
  }, [updateProject]);

  const deleteProject = useCallback(async (id: string) => {
    return runStoreMutation({
      request: async () => {
        await projectApi.deleteProject(id);
        return id;
      },
      onSuccess: (deletedId) => {
        removeProject(deletedId);
        useBackgroundTaskStore.getState().removeTasksByProjectId(deletedId);
      },
      errorLogLabel: 'Failed to delete project:',
    });
  }, [removeProject]);

  return {
    createProject,
    updateProject: updateProjectSync,
    deleteProject,
  };
}
