export type ProjectNavigationPageKey = 'outline' | 'characters' | 'chapters' | 'organizations' | 'careers' | 'relationships';

const projectPageLoadPromises = new Map<ProjectNavigationPageKey, Promise<unknown>>();
const PROJECT_NAVIGATION_PRELOAD_ORDER: readonly ProjectNavigationPageKey[] = ['characters', 'chapters', 'careers'];
const SLOW_CONNECTION_TYPES = new Set(['slow-2g', '2g', '3g']);

interface ProjectNavigationPreloadOptions {
  delayMs?: number;
  pages?: readonly ProjectNavigationPageKey[];
}

interface NetworkInformationLike {
  effectiveType?: string;
  saveData?: boolean;
}

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

const waitFor = (delayMs: number) => new Promise<void>((resolve) => {
  window.setTimeout(resolve, delayMs);
});

export const shouldSkipProjectNavigationPreload = () => {
  const connection = (navigator as Navigator & { connection?: NetworkInformationLike }).connection;
  const effectiveType = connection?.effectiveType;
  return Boolean(
    connection?.saveData
    || (effectiveType && SLOW_CONNECTION_TYPES.has(effectiveType))
  );
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

export const preloadProjectNavigationPages = async (options: ProjectNavigationPreloadOptions = {}) => {
  const pages = options.pages ?? PROJECT_NAVIGATION_PRELOAD_ORDER;
  const delayMs = Math.max(options.delayMs ?? 0, 0);

  for (let index = 0; index < pages.length; index += 1) {
    const pageKey = pages[index];

    try {
      await preloadProjectPage(pageKey);
    } catch (error) {
      console.debug(`Preload failed for project page: ${pageKey}`, error);
    }

    if (delayMs > 0 && index < pages.length - 1) {
      await waitFor(delayMs);
    }
  }
};
