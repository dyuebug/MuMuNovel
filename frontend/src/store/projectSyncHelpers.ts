import { message } from 'antd';
import { useBackgroundTaskStore } from './backgroundTasks';
import { useStore } from './index';
import { normalizeStoreItems } from './storeMutationHelpers';
import { projectApi } from '../services/modularApi';
import type { Project } from '../types';

const syncLoadedProjectToStore = (project: Project) => {
  const store = useStore.getState();

  store.setCurrentProject(project);
  if (store.projects.some((item) => item.id === project.id)) {
    store.updateProject(project.id, project);
  } else {
    store.addProject(project);
  }

  return project;
};

export async function syncProjectToStoreById(projectId: string) {
  const project = await projectApi.getProject(projectId);
  return syncLoadedProjectToStore(project);
}

export async function refreshProjectsToStore() {
  const store = useStore.getState();

  try {
    store.setLoading(true);
    const data = await projectApi.getProjects();
    const projects = normalizeStoreItems<Project>(data);
    store.setProjects(projects);

    const currentProject = store.currentProject;
    if (currentProject && !projects.some((project) => project.id === currentProject.id)) {
      store.setCurrentProject(null);
    }

    useBackgroundTaskStore.getState().pruneTasksByProjectIds(projects.map((project) => project.id));
    return projects;
  } catch (error) {
    console.error('Failed to refresh projects:', error);
    message.error('Failed to refresh projects.');
    return [];
  } finally {
    store.setLoading(false);
  }
}
