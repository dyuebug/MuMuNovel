import { expect, type BrowserContext, type Page, type Request } from '@playwright/test';

export const usernameSelector = 'input[autocomplete="username"]';
export const passwordSelector = 'input[autocomplete="current-password"]';
export const submitSelector = 'button[type="submit"]';
export const requiresRealBackend = !process.env.E2E_REAL_BACKEND?.trim();

export type CreatedProject = {
  id: string;
  title: string;
};

export type CreatedOutline = {
  id: string;
  project_id: string;
  title: string;
  content: string;
  order_index: number;
};

export type BackgroundTaskRequestRecord = {
  method: string;
  url: string;
  taskId: string | null;
  timestamp: number;
};

export const extractBackgroundTaskIdFromUrl = (url: string): string | null => {
  const matched = url.match(/\/api\/background-tasks\/([^/?#]+)/);
  return matched?.[1] ?? null;
};

export const login = async (page: Page) => {
  await page.locator(usernameSelector).fill('admin');
  await page.locator(passwordSelector).fill('admin123');
  await page.locator(submitSelector).click();
  await page.waitForTimeout(3000);
};

export const prepareAuthenticatedPage = async (page: Page, context: BrowserContext) => {
  await context.clearCookies();
  await page.addInitScript(() => {
    localStorage.clear();
    sessionStorage.clear();
    localStorage.setItem('announcement_hide_forever', 'true');
  });
  await page.goto('/login');
  await login(page);
};

export const createProject = async (
  context: BrowserContext,
  suffix: string,
  overrides: Record<string, unknown> = {},
): Promise<CreatedProject> => {
  const response = await context.request.post('/api/projects', {
    data: {
      title: `Smoke Project ${suffix}`,
      description: 'Playwright smoke project for background task pages',
      theme: 'fantasy adventure',
      genre: 'fantasy',
      target_words: 12000,
      outline_mode: 'one-to-many',
      ...overrides,
    },
  });

  expect(response.ok()).toBeTruthy();
  const project = await response.json() as CreatedProject;
  expect(project.id).toBeTruthy();
  return project;
};

export const updateProject = async (
  context: BrowserContext,
  projectId: string,
  data: Record<string, unknown>,
) => {
  const response = await context.request.put(`/api/projects/${projectId}`, { data });
  expect(response.ok()).toBeTruthy();
  return response.json();
};

export const seedProjectForPageTasks = async (context: BrowserContext, projectId: string) => {
  await updateProject(context, projectId, {
    narrative_perspective: 'third person',
    chapter_count: 5,
    world_time_period: 'ancient era',
    world_location: 'northern continent',
    world_atmosphere: 'tense and mysterious',
    world_rules: 'magic exists and has a cost',
  });
};

export const deleteProject = async (context: BrowserContext, projectId: string) => {
  const response = await context.request.delete(`/api/projects/${projectId}`);
  expect(response.ok()).toBeTruthy();
};

export const createOutline = async (
  context: BrowserContext,
  projectId: string,
  orderIndex: number,
  title: string,
  content?: string,
): Promise<CreatedOutline> => {
  const response = await context.request.post('/api/outlines', {
    data: {
      project_id: projectId,
      title,
      content: content ?? `${title} 的剧情概要，用于 Playwright 背景任务 smoke。`,
      order_index: orderIndex,
    },
  });

  expect(response.ok()).toBeTruthy();
  return await response.json() as CreatedOutline;
};

export const trackBackgroundTaskRequests = (page: Page) => {
  const requests: BackgroundTaskRequestRecord[] = [];
  const listener = (request: Request) => {
    const url = request.url();
    if (!url.includes('/api/background-tasks')) {
      return;
    }

    requests.push({
      method: request.method(),
      url: url.replace(/[0-9a-f]{8}-[0-9a-f-]{27,}/gi, '{id}'),
      taskId: extractBackgroundTaskIdFromUrl(url),
      timestamp: Date.now(),
    });
  };

  page.on('request', listener);
  return {
    requests,
    detach: () => page.off('request', listener),
  };
};

export const openProjectSubPage = async (
  page: Page,
  projectId: string,
  subPath: 'world-setting' | 'outline' | 'careers' | 'characters',
) => {
  await page.goto(`/project/${projectId}/sponsor`);

  const navLink = page.locator(`a[href="/project/${projectId}/${subPath}"]`).first();
  await expect(navLink).toBeVisible({ timeout: 15000 });
  await navLink.click();
  await expect(page).toHaveURL(new RegExp(`/project/${projectId}/${subPath}$`));
};

export const buildCreateTaskRequestPredicate = (taskType?: string) => {
  return (request: Request) => {
    if (request.method() !== 'POST' || !request.url().includes('/api/background-tasks')) {
      return false;
    }

    if (!taskType) {
      return true;
    }

    return request.postData()?.includes(taskType) ?? false;
  };
};

export const buildTaskStatusRequestPredicate = () => {
  return (request: Request) => (
    request.method() === 'GET' && /\/api\/background-tasks\/.+/.test(request.url())
  );
};

export const assertSingleBackgroundTaskWindow = (
  requests: BackgroundTaskRequestRecord[],
  observationStart: number,
  maxStatusRequests = 20,
  taskId?: string | null,
) => {
  const requestsInWindow = requests.filter((item) => item.timestamp >= observationStart);
  const createRequests = requestsInWindow.filter(
    (item) => item.method === 'POST' && item.url.includes('/api/background-tasks'),
  );
  const statusRequests = requestsInWindow.filter(
    (item) => item.method === 'GET'
      && item.url.includes('/api/background-tasks/{id}')
      && (!taskId || item.taskId === taskId),
  );

  expect(createRequests).toHaveLength(1);
  expect(statusRequests.length).toBeGreaterThan(0);
  expect(statusRequests.length).toBeLessThanOrEqual(maxStatusRequests);
};

export const assertBackgroundTaskPollingCadence = (
  requests: BackgroundTaskRequestRecord[],
  observationStart: number,
  observationEnd: number,
  options?: {
    taskId?: string | null;
    pollIntervalMs?: number;
    extraAllowance?: number;
  },
) => {
  const {
    taskId,
    pollIntervalMs = 1500,
    extraAllowance = 5,
  } = options || {};

  const requestsInWindow = requests.filter(
    (item) => item.timestamp >= observationStart && item.timestamp <= observationEnd,
  );
  const createRequests = requestsInWindow.filter(
    (item) => item.method === 'POST' && item.url.includes('/api/background-tasks'),
  );
  const statusRequests = requestsInWindow.filter(
    (item) => item.method === 'GET'
      && item.url.includes('/api/background-tasks/{id}')
      && (!taskId || item.taskId === taskId),
  );

  const elapsedMs = Math.max(observationEnd - observationStart, pollIntervalMs);
  const maxExpectedStatusRequests = Math.ceil(elapsedMs / pollIntervalMs) + extraAllowance;

  expect(createRequests).toHaveLength(1);
  expect(statusRequests.length).toBeGreaterThan(0);
  expect(statusRequests.length).toBeLessThanOrEqual(maxExpectedStatusRequests);
};

export const waitForOptionalOutlinePreviewDismiss = async (
  page: Page,
  timeout = 8000,
) => {
  const previewDismissButton = page.getByRole('button', { name: '暂不创建', exact: true }).last();

  try {
    await previewDismissButton.waitFor({ state: 'visible', timeout });
    await previewDismissButton.click();
  } catch {
    await page.waitForTimeout(timeout);
  }

  return Date.now();
};

export const waitForOutlinePreviewDismiss = async (
  page: Page,
  timeout = 120000,
) => {
  const previewDismissButton = page.getByRole('button', { name: '暂不创建', exact: true }).last();
  await expect(previewDismissButton).toBeVisible({ timeout });
  return previewDismissButton;
};
