import { expect, test } from '@playwright/test';

import {
  assertSingleBackgroundTaskWindow,
  buildTaskStatusRequestPredicate,
  extractBackgroundTaskIdFromUrl,
  prepareAuthenticatedPage,
  requiresRealBackend,
  trackBackgroundTaskRequests,
  submitSelector,
} from './helpers/backgroundTaskSmoke';

test.describe('wizard background task smoke', () => {
  test.beforeEach(async ({ page, context }) => {
    test.skip(requiresRealBackend, 'requires E2E_REAL_BACKEND=1 and a reachable real backend');
    await prepareAuthenticatedPage(page, context);
  });

  test('creates one world-building task without polling storm', async ({ page }) => {
    const tracker = trackBackgroundTaskRequests(page);

    await page.goto('/wizard');
    await page.waitForLoadState('networkidle');

    const uniqueSuffix = Date.now().toString().slice(-6);
    await page.locator('#title').fill(`Smoke Novel ${uniqueSuffix}`);
    await page.locator('#description').fill('Playwright wizard smoke test for background tasks');
    await page.locator('#theme').fill('fantasy adventure');

    const observationStart = Date.now();
    const statusRequestPromise = page.waitForRequest(buildTaskStatusRequestPredicate(), { timeout: 30000 });

    await page.locator(submitSelector).click();
    const statusRequest = await statusRequestPromise;
    const taskId = extractBackgroundTaskIdFromUrl(statusRequest.url());
    await page.waitForTimeout(8000);
    tracker.detach();

    assertSingleBackgroundTaskWindow(tracker.requests, observationStart, 20, taskId);
  });
});
