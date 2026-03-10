export type ProjectNavigationPageKey = 'outline' | 'characters' | 'chapters' | 'organizations' | 'careers' | 'relationships';

const projectPageLoadPromises = new Map<ProjectNavigationPageKey, Promise<unknown>>();

export const loadOutlinePage = () => import('../pages/Outline');
export const loadCharactersPage = () => import('../pages/Characters');
export const loadChaptersPage = () => import('../pages/Chapters');
export const loadOrganizationsPage = () => import('../pages/Organizations');
export const loadCareersPage = () => import('../pages/Careers');
export const loadRelationshipsPage = () => import('../pages/Relationships');

const projectPageLoaders: Record<ProjectNavigationPageKey, () => Promise<unknown>> = {
  outline: loadOutlinePage,
  characters: loadCharactersPage,
  chapters: loadChaptersPage,
  organizations: loadOrganizationsPage,
  careers: loadCareersPage,
  relationships: loadRelationshipsPage,
};

export const preloadProjectPage = async (pageKey: ProjectNavigationPageKey) => {
  const existingPromise = projectPageLoadPromises.get(pageKey);
  if (existingPromise) {
    await existingPromise;
    return;
  }

  const loadPromise = projectPageLoaders[pageKey]();
  projectPageLoadPromises.set(pageKey, loadPromise);
  await loadPromise;
};

export const preloadProjectNavigationPages = async () => {
  await Promise.allSettled([
    preloadProjectPage('outline'),
    preloadProjectPage('characters'),
    preloadProjectPage('chapters'),
    preloadProjectPage('careers'),
  ]);
};
