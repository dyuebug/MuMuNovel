import { expect, test } from '@playwright/test';

import {
  buildCreateTaskRequestPredicate,
  buildTaskStatusRequestPredicate,
  createOutline,
  createProject,
  deleteProject,
  openProjectSubPage,
  prepareAuthenticatedPage,
  requiresRealBackend,
  seedProjectForPageTasks,
  waitForOutlinePreviewDismiss,
} from './helpers/backgroundTaskSmoke';

const mockSingleOutlineExpandTask = async (
  page: import('@playwright/test').Page,
  outline: { id: string; title: string },
) => {
  const taskId = `mock-outline-expand-${outline.id}`;
  const now = new Date().toISOString();
  const result = {
    outline_id: outline.id,
    outline_title: outline.title,
    target_chapter_count: 3,
    actual_chapter_count: 2,
    expansion_strategy: 'balanced',
    chapter_plans: [
      {
        sub_index: 1,
        title: '初探风雪城',
        plot_summary: '主角初入边城，发现局势比传闻更加复杂。',
        key_events: ['进入城门', '遭遇试探'],
        character_focus: ['主角'],
        emotional_tone: '紧张',
        narrative_goal: '建立边城危机感',
        conflict_type: '外部冲突',
        estimated_words: 3200,
        scenes: [{ location: '风雪城南门', characters: ['主角'], purpose: '铺垫危机' }],
      },
      {
        sub_index: 2,
        title: '夜访旧巷',
        plot_summary: '主角循着线索潜入旧巷，与关键人物发生第一次交锋。',
        key_events: ['追查线索', '正面交锋'],
        character_focus: ['主角', '神秘人'],
        emotional_tone: '压抑',
        narrative_goal: '推进主线谜团',
        conflict_type: '人物冲突',
        estimated_words: 3400,
        scenes: [{ location: '旧巷', characters: ['主角', '神秘人'], purpose: '推进调查' }],
      },
    ],
    created_chapters: null,
  };

  await page.route('**/api/background-tasks**', async (route) => {
    const request = route.request();
    const url = request.url();

    if (
      request.method() === 'POST'
      && url.includes('/api/background-tasks')
      && (request.postData()?.includes('outline_expand') ?? false)
    ) {
      await route.fulfill({
        status: 200,
        contentType: 'application/json; charset=utf-8',
        body: JSON.stringify({
          task_id: taskId,
          task_type: 'outline_expand',
          project_id: null,
          status: 'pending',
          progress: 0,
          message: '正在生成章节规划...',
          created_at: now,
          updated_at: now,
        }),
      });
      return;
    }

    if (request.method() === 'GET' && url.includes(`/api/background-tasks/${taskId}`)) {
      await route.fulfill({
        status: 200,
        contentType: 'application/json; charset=utf-8',
        body: JSON.stringify({
          task_id: taskId,
          task_type: 'outline_expand',
          project_id: null,
          status: 'completed',
          progress: 100,
          message: '章节规划生成完成',
          result,
          created_at: now,
          updated_at: now,
          completed_at: now,
        }),
      });
      return;
    }

    await route.continue();
  });
};

test.describe('outline expand full flow', () => {
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
        // best-effort cleanup for E2E fixtures
      }
    }
  });

  test('shows single outline expansion preview after background task completes', async ({ page, context }) => {
    test.setTimeout(300000);

    const project = await createProject(context, `${Date.now()}-outline-expand-flow`);
    createdProjectIds.push(project.id);
    await seedProjectForPageTasks(context, project.id);
    const outline = await createOutline(context, project.id, 1, '第一卷：风雪入城');

    await mockSingleOutlineExpandTask(page, outline);
    await openProjectSubPage(page, project.id, 'outline');

    const expandButton = page.getByRole('button', { name: /展开$/ }).first();
    await expect(expandButton).toBeVisible({ timeout: 15000 });
    await expandButton.click();

    const previewButton = page.getByRole('button', { name: '生成规划预览', exact: true }).last();
    await expect(previewButton).toBeVisible({ timeout: 15000 });

    const createRequestPromise = page.waitForRequest(
      buildCreateTaskRequestPredicate('outline_expand'),
      { timeout: 30000 },
    );
    const statusRequestPromise = page.waitForRequest(buildTaskStatusRequestPredicate(), { timeout: 30000 });

    await previewButton.click();
    await createRequestPromise;
    await statusRequestPromise;

    const dismissButton = await waitForOutlinePreviewDismiss(page, 180000);
    await expect(page.getByRole('button', { name: '确认并创建章节', exact: true })).toBeVisible({ timeout: 15000 });
    await expect(page.getByText('预览模式（未创建章节）', { exact: true })).toBeVisible({ timeout: 15000 });
    await dismissButton.click();
  });

  test('shows batch outline expansion preview after background task completes', async ({ page, context }) => {
    test.setTimeout(300000);

    const project = await createProject(context, `${Date.now()}-outline-batch-flow`);
    createdProjectIds.push(project.id);
    await seedProjectForPageTasks(context, project.id);
    await createOutline(context, project.id, 1, '第一卷：暗河回响');

    await openProjectSubPage(page, project.id, 'outline');

    const batchExpandButton = page.getByRole('button', { name: /批量展开/ }).first();
    await expect(batchExpandButton).toBeVisible({ timeout: 15000 });
    await batchExpandButton.click();

    const submitButton = page.getByRole('button', { name: '开始展开', exact: true }).last();
    await expect(submitButton).toBeVisible({ timeout: 15000 });

    const createRequestPromise = page.waitForRequest(
      buildCreateTaskRequestPredicate('outline_batch_expand'),
      { timeout: 30000 },
    );
    const statusRequestPromise = page.waitForRequest(buildTaskStatusRequestPredicate(), { timeout: 30000 });

    await submitButton.click();
    await createRequestPromise;
    await statusRequestPromise;

    const dismissButton = await waitForOutlinePreviewDismiss(page, 180000);
    await expect(page.getByRole('button', { name: '确认创建章节', exact: true })).toBeVisible({ timeout: 15000 });
    await expect(page.getByText('批量展开预览', { exact: true })).toBeVisible({ timeout: 15000 });
    await dismissButton.click();
  });
});