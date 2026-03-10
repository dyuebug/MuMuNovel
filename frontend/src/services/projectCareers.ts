import api from './api';

export interface CareerStage {
  level: number;
  name: string;
  description?: string;
}

export interface ProjectCareerRecord {
  id: string;
  project_id: string;
  name: string;
  type: 'main' | 'sub';
  description?: string;
  category?: string;
  stages: CareerStage[];
  max_stage: number;
  requirements?: string;
  special_abilities?: string;
  worldview_rules?: string;
  source: string;
}

export interface ProjectCareerCollection {
  mainCareers: ProjectCareerRecord[];
  subCareers: ProjectCareerRecord[];
}

const projectCareersCache = new Map<string, ProjectCareerCollection>();
const projectCareerLoadPromises = new Map<string, Promise<ProjectCareerCollection>>();

export function getCachedProjectCareers(projectId: string) {
  return projectCareersCache.get(projectId);
}

export function invalidateProjectCareers(projectId?: string) {
  if (!projectId) {
    return;
  }

  projectCareersCache.delete(projectId);
  projectCareerLoadPromises.delete(projectId);
}

export async function loadProjectCareers(projectId: string): Promise<ProjectCareerCollection> {
  const cachedCareers = projectCareersCache.get(projectId);
  if (cachedCareers) {
    return cachedCareers;
  }

  const existingLoad = projectCareerLoadPromises.get(projectId);
  if (existingLoad) {
    return existingLoad;
  }

  const loadPromise = (async () => {
    const response = await api.get<unknown, { main_careers?: ProjectCareerRecord[]; sub_careers?: ProjectCareerRecord[] }>('/careers', {
      params: { project_id: projectId },
    });

    const nextCareers = {
      mainCareers: response.main_careers || [],
      subCareers: response.sub_careers || [],
    };

    projectCareersCache.set(projectId, nextCareers);
    return nextCareers;
  })();

  projectCareerLoadPromises.set(projectId, loadPromise);

  try {
    return await loadPromise;
  } finally {
    projectCareerLoadPromises.delete(projectId);
  }
}

export async function preloadProjectCareers(projectId: string) {
  try {
    await loadProjectCareers(projectId);
  } catch (error) {
    console.error('preload careers failed:', error);
  }
}
