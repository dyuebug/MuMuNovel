import { expect, test } from '@playwright/test';

import {
  assertBackgroundTaskPollingCadence,
  assertSingleBackgroundTaskWindow,
  buildCreateTaskRequestPredicate,
  buildTaskStatusRequestPredicate,
  createOutline,
  createProject,
  deleteProject,
  extractBackgroundTaskIdFromUrl,
  openProjectSubPage,
  prepareAuthenticatedPage,
  requiresRealBackend,
  seedProjectForPageTasks,
  trackBackgroundTaskRequests,
  waitForOptionalOutlinePreviewDismiss,
} from './helpers/backgroundTaskSmoke';

test.describe('background task page smoke', () => {
  let createdProjectIds: string[] = [];

  test.beforeEach(async ({ page, context }) => {
    test.skip(requiresRealBackend, 'requires E2E_REAL_BACKEND=1 and a reachable real backend');
    createdProjectIds = [];
    await prepareAuthenticatedPage(page, context);
  });

  test.afterEach(async ({ context }) => {
    for (const projectId of createdProjectIds.reverse()) {
      try {
        await deleteProject(context, projectId);
      } catch {
        // best-effort cleanup for smoke fixtures
      }
    }
  });

  test('creates one world-regenerate task without polling storm', async ({ page, context }) => {
    const project = await createProject(context, `${Date.now()}-world`);
    createdProjectIds.push(project.id);
    await seedProjectForPageTasks(context, project.id);

    const tracker = trackBackgroundTaskRequests(page);
    await openProjectSubPage(page, project.id, 'world-setting');

    const regenerateButton = page.locator('main button:has(.anticon-sync)').first();
    await expect(regenerateButton).toBeVisible({ timeout: 15000 });

    const observationStart = Date.now();
    await regenerateButton.click();
    await expect(page.locator('.ant-modal-confirm')).toBeVisible();

    const createRequestPromise = page.waitForRequest(
      buildCreateTaskRequestPredicate('world_regenerate'),
      { timeout: 30000 },
    );
    const statusRequestPromise = page.waitForRequest(buildTaskStatusRequestPredicate(), { timeout: 30000 });

    await page.locator('.ant-modal-confirm .ant-btn-primary').click();
    await createRequestPromise;
    const statusRequest = await statusRequestPromise;
    const taskId = extractBackgroundTaskIdFromUrl(statusRequest.url());
    await page.waitForTimeout(5000);
    tracker.detach();

    assertSingleBackgroundTaskWindow(tracker.requests, observationStart, 20, taskId);
  });

  test('creates one outline-generate task without polling storm', async ({ page, context }) => {
    const project = await createProject(context, `${Date.now()}-outline`);
    createdProjectIds.push(project.id);
    await seedProjectForPageTasks(context, project.id);

    const tracker = trackBackgroundTaskRequests(page);
    await openProjectSubPage(page, project.id, 'outline');

    const generateButton = page.locator('main button:has(.anticon-thunderbolt)').first();
    await expect(generateButton).toBeVisible({ timeout: 15000 });

    const observationStart = Date.now();
    await generateButton.click();
    await expect(page.locator('.ant-modal')).toBeVisible();

    const submitButton = page.locator('.ant-modal-confirm-btns .ant-btn-primary').last();
    await expect(submitButton).toBeVisible({ timeout: 15000 });

    const createRequestPromise = page.waitForRequest(
      buildCreateTaskRequestPredicate('outline_generate'),
      { timeout: 30000 },
    );
    const statusRequestPromise = page.waitForRequest(buildTaskStatusRequestPredicate(), { timeout: 30000 });

    await submitButton.click();
    await createRequestPromise;
    const statusRequest = await statusRequestPromise;
    const taskId = extractBackgroundTaskIdFromUrl(statusRequest.url());
    await page.waitForTimeout(5000);
    tracker.detach();

    assertSingleBackgroundTaskWindow(tracker.requests, observationStart, 20, taskId);
  });

  test('creates one outline-expand task without polling storm', async ({ page, context }) => {
    test.setTimeout(120000);

    const project = await createProject(context, `${Date.now()}-outline-expand`);
    createdProjectIds.push(project.id);
    await seedProjectForPageTasks(context, project.id);
    await createOutline(context, project.id, 1, '第一卷：迷雾初现');

    const tracker = trackBackgroundTaskRequests(page);
    await openProjectSubPage(page, project.id, 'outline');

    const outlineItem = page.locator('.ant-list-item').filter({ hasText: '第一卷：迷雾初现' });
    await expect(outlineItem).toHaveCount(1);

    const expandButton = outlineItem.getByRole('button', { name: /展开$/ });
    await expect(expandButton).toBeVisible({ timeout: 15000 });
    await expect(expandButton).toBeEnabled();

    const observationStart = Date.now();
    await expandButton.focus();
    await expect(expandButton).toBeFocused();
    await expandButton.press('Enter');
    await expect(page.getByRole('button', { name: '生成规划预览', exact: true }).last()).toBeVisible({ timeout: 15000 });

    const createRequestPromise = page.waitForRequest(
      buildCreateTaskRequestPredicate('outline_expand'),
      { timeout: 30000 },
    );
    const statusRequestPromise = page.waitForRequest(buildTaskStatusRequestPredicate(), { timeout: 30000 });

    await page.getByRole('button', { name: '生成规划预览', exact: true }).last().click();
    await createRequestPromise;
    const statusRequest = await statusRequestPromise;
    const taskId = extractBackgroundTaskIdFromUrl(statusRequest.url());

    const observationEnd = await waitForOptionalOutlinePreviewDismiss(page);
    tracker.detach();

    assertBackgroundTaskPollingCadence(tracker.requests, observationStart, observationEnd, { taskId });
  });

  test('creates one outline-batch-expand task without polling storm', async ({ page, context }) => {
    test.setTimeout(120000);

    const project = await createProject(context, `${Date.now()}-outline-batch-expand`);
    createdProjectIds.push(project.id);
    await seedProjectForPageTasks(context, project.id);
    await createOutline(context, project.id, 1, '第一卷：暗潮涌动');

    const tracker = trackBackgroundTaskRequests(page);
    await openProjectSubPage(page, project.id, 'outline');
    await expect(page.getByRole('button', { name: /展开$/ }).first()).toBeVisible({ timeout: 15000 });

    const batchExpandButton = page.getByRole('button', { name: /批量展开/ }).first();
    await expect(batchExpandButton).toBeVisible({ timeout: 15000 });

    const observationStart = Date.now();
    await batchExpandButton.click();
    await expect(page.getByRole('button', { name: '开始展开', exact: true }).last()).toBeVisible({ timeout: 15000 });

    const createRequestPromise = page.waitForRequest(
      buildCreateTaskRequestPredicate('outline_batch_expand'),
      { timeout: 30000 },
    );
    const statusRequestPromise = page.waitForRequest(buildTaskStatusRequestPredicate(), { timeout: 30000 });

    await page.getByRole('button', { name: '开始展开', exact: true }).last().click();
    await createRequestPromise;
    const statusRequest = await statusRequestPromise;
    const taskId = extractBackgroundTaskIdFromUrl(statusRequest.url());

    const observationEnd = await waitForOptionalOutlinePreviewDismiss(page);
    tracker.detach();

    assertBackgroundTaskPollingCadence(tracker.requests, observationStart, observationEnd, { taskId });
  });

  test('creates one careers-generate-system task without polling storm', async ({ page, context }) => {
    const project = await createProject(context, `${Date.now()}-careers`);
    createdProjectIds.push(project.id);
    await seedProjectForPageTasks(context, project.id);

    const tracker = trackBackgroundTaskRequests(page);
    await openProjectSubPage(page, project.id, 'careers');

    const generateButton = page.locator('main button:has(.anticon-thunderbolt)').first();
    await expect(generateButton).toBeVisible({ timeout: 15000 });

    const observationStart = Date.now();
    await generateButton.click();
    await expect(page.locator('.ant-modal')).toBeVisible();

    const submitButton = page.locator('.ant-modal button[type="submit"]').last();
    await expect(submitButton).toBeVisible({ timeout: 15000 });

    const createRequestPromise = page.waitForRequest(
      buildCreateTaskRequestPredicate('careers_generate_system'),
      { timeout: 30000 },
    );
    const statusRequestPromise = page.waitForRequest(buildTaskStatusRequestPredicate(), { timeout: 30000 });

    await submitButton.click();
    await createRequestPromise;
    const statusRequest = await statusRequestPromise;
    const taskId = extractBackgroundTaskIdFromUrl(statusRequest.url());
    await page.waitForTimeout(5000);
    tracker.detach();

    assertSingleBackgroundTaskWindow(tracker.requests, observationStart, 20, taskId);
  });

  test('creates one character-generate task without polling storm', async ({ page, context }) => {
    const project = await createProject(context, `${Date.now()}-characters`);
    createdProjectIds.push(project.id);
    await seedProjectForPageTasks(context, project.id);

    const tracker = trackBackgroundTaskRequests(page);
    await openProjectSubPage(page, project.id, 'characters');

    const generateButton = page.locator('main button:has(.anticon-thunderbolt)').first();
    await expect(generateButton).toBeVisible({ timeout: 15000 });

    const observationStart = Date.now();
    await generateButton.click();
    await expect(page.locator('.ant-modal-confirm')).toBeVisible();

    const roleSelect = page.locator('.ant-modal-confirm .ant-select').first();
    await roleSelect.click();
    await page.locator('.ant-select-item-option').first().click();

    const createRequestPromise = page.waitForRequest(
      buildCreateTaskRequestPredicate('character_generate'),
      { timeout: 30000 },
    );
    const statusRequestPromise = page.waitForRequest(buildTaskStatusRequestPredicate(), { timeout: 30000 });

    await page.locator('.ant-modal-confirm-btns .ant-btn-primary').last().click();
    await createRequestPromise;
    const statusRequest = await statusRequestPromise;
    const taskId = extractBackgroundTaskIdFromUrl(statusRequest.url());
    await page.waitForTimeout(5000);
    tracker.detach();

    assertSingleBackgroundTaskWindow(tracker.requests, observationStart, 20, taskId);
  });
  test('creates one organization-generate task without polling storm', async ({ page, context }) => {
    const project = await createProject(context, `${Date.now()}-organizations`);
    createdProjectIds.push(project.id);
    await seedProjectForPageTasks(context, project.id);

    const tracker = trackBackgroundTaskRequests(page);
    await openProjectSubPage(page, project.id, 'characters');

    const generateOrgButton = page.getByRole('button', { name: /智能生成组织/i });
    await expect(generateOrgButton).toBeVisible({ timeout: 15000 });

    const observationStart = Date.now();
    await generateOrgButton.click();
    await expect(page.locator('.ant-modal-confirm')).toBeVisible();

    const createRequestPromise = page.waitForRequest(
      buildCreateTaskRequestPredicate('organization_generate'),
      { timeout: 30000 },
    );
    const statusRequestPromise = page.waitForRequest(buildTaskStatusRequestPredicate(), { timeout: 30000 });

    await page.locator('.ant-modal-confirm-btns .ant-btn-primary').last().click();
    await createRequestPromise;
    const statusRequest = await statusRequestPromise;
    const taskId = extractBackgroundTaskIdFromUrl(statusRequest.url());
    await page.waitForTimeout(5000);
    tracker.detach();

    assertSingleBackgroundTaskWindow(tracker.requests, observationStart, 26, taskId);
  });
});
