import { expect, test } from '@playwright/test';

import {
  prepareAuthenticatedPage,
  requiresRealBackend,
} from './helpers/backgroundTaskSmoke';

test.describe('inspiration resume smoke', () => {
  test.beforeEach(async ({ page, context }) => {
    test.skip(requiresRealBackend, 'requires E2E_REAL_BACKEND=1 and a reachable real backend');
    await prepareAuthenticatedPage(page, context);
  });

  test('clears stale inspiration resume task state instead of resuming an old task', async ({ page }) => {
    const staleTaskId = 'stale-inspiration-task-id';
    const staleRequests: string[] = [];

    page.on('request', (request) => {
      if (request.url().includes(`/api/background-tasks/${staleTaskId}`)) {
        staleRequests.push(request.url());
      }
    });

    await page.evaluate((taskId) => {
      localStorage.setItem('inspiration_task_id', taskId);
      localStorage.setItem('inspiration_current_step', 'idea');
      localStorage.removeItem('inspiration_generation_data');
      localStorage.removeItem('inspiration_project_id');
    }, staleTaskId);

    await page.goto('/inspiration');
    await page.waitForLoadState('networkidle');

    await expect(page).toHaveURL(/\/inspiration$/);
    await expect(page.locator('textarea').first()).toBeVisible({ timeout: 15000 });

    const storageState = await page.evaluate(() => ({
      taskId: localStorage.getItem('inspiration_task_id'),
      step: localStorage.getItem('inspiration_current_step'),
      generationData: localStorage.getItem('inspiration_generation_data'),
      projectId: localStorage.getItem('inspiration_project_id'),
    }));

    expect(storageState.taskId).toBeNull();
    expect(storageState.step).toBeNull();
    expect(storageState.generationData).toBeNull();
    expect(storageState.projectId).toBeNull();
    expect(staleRequests).toEqual([]);
  });
});
