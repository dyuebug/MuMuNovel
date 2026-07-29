import { expect, test, type Page } from '@playwright/test';

type SeedTask = {
  taskId: string;
  taskType: string;
  status: 'pending' | 'running';
  progress: number;
  message: string;
  checkpoint?: Record<string, unknown>;
  createdAt: number;
  updatedAt: number;
};

const recoveredTasks = {
  restartable: {
    taskId: 'recovery-restartable',
    taskType: 'polish_text',
    terminalReason: 'restart_required',
    reviewRequired: false,
    canResume: false,
  },
  resumable: {
    taskId: 'recovery-resumable',
    taskType: 'chapters_batch_generate',
    terminalReason: 'resume_available',
    reviewRequired: false,
    canResume: true,
  },
  manualReview: {
    taskId: 'recovery-manual-review',
    taskType: 'world_regenerate',
    terminalReason: 'manual_review',
    reviewRequired: true,
    canResume: false,
  },
  nonResumable: {
    taskId: 'recovery-non-resumable',
    taskType: 'unknown',
    terminalReason: 'non_resumable',
    reviewRequired: false,
    canResume: false,
  },
} as const;

const seedPersistedActiveTasks = async (page: Page) => {
  const now = Date.now();
  const tasks: Record<string, SeedTask> = Object.fromEntries(
    Object.values(recoveredTasks).map((task) => [
      task.taskId,
      {
        taskId: task.taskId,
        taskType: task.taskType,
        status: 'running',
        progress: 45,
        message: '服务重启前仍在执行',
        checkpoint: task.taskId === recoveredTasks.resumable.taskId
          ? { completed_chapters: [1] }
          : undefined,
        createdAt: now - 60_000,
        updatedAt: now,
      },
    ]),
  );

  await page.addInitScript((persistedTasks) => {
    localStorage.clear();
    sessionStorage.clear();
    localStorage.setItem('announcement_hide_forever', 'true');
    localStorage.setItem('background-task-store', JSON.stringify({
      state: { tasks: persistedTasks },
      version: 0,
    }));
  }, tasks);
};

const fulfillJson = async (route: Parameters<Parameters<Page['route']>[1]>[0], body: unknown) => {
  await route.fulfill({
    status: 200,
    contentType: 'application/json; charset=utf-8',
    body: JSON.stringify(body),
  });
};

const installApiMocks = async (page: Page) => {
  const recoveredAt = new Date().toISOString();
  const startedAt = new Date(Date.now() - 60_000).toISOString();
  await page.route('**/api/**', async (route) => {
    await fulfillJson(route, {});
  });

  await page.route('**/api/auth/user', async (route) => {
    await fulfillJson(route, {
      id: 'recovery-contract-user',
      username: 'recovery-contract-user',
      is_admin: true,
    });
  });

  await page.route('**/api/projects', async (route) => {
    await fulfillJson(route, []);
  });

  await page.route('**/api/background-tasks?**', async (route) => {
    await fulfillJson(route, { total: 0, items: [] });
  });

  await page.route('**/api/chapters/batch-generate/active-tasks?**', async (route) => {
    await fulfillJson(route, { total: 0, items: [] });
  });

  for (const task of [
    recoveredTasks.restartable,
    recoveredTasks.manualReview,
    recoveredTasks.nonResumable,
  ]) {
    await page.route(`**/api/background-tasks/${task.taskId}`, async (route) => {
      await fulfillJson(route, {
        task_id: task.taskId,
        task_type: task.taskType,
        project_id: '',
        status: 'failed',
        progress: 45,
        message: '服务重启后已按恢复策略安全终止',
        error: 'orphan_task_recovered',
        terminal_reason: task.terminalReason,
        review_required: task.reviewRequired,
        can_resume: task.canResume,
        created_at: startedAt,
        updated_at: recoveredAt,
        completed_at: recoveredAt,
      });
    });
  }

  await page.route(
    `**/api/chapters/batch-generate/${recoveredTasks.resumable.taskId}/status`,
    async (route) => {
      await fulfillJson(route, {
        batch_id: recoveredTasks.resumable.taskId,
        status: 'failed',
        total: 3,
        completed: 1,
        current_chapter_number: 2,
        error_message: '服务重启后可从检查点继续',
        checkpoint: { completed_chapters: [1] },
        terminal_reason: recoveredTasks.resumable.terminalReason,
        review_required: recoveredTasks.resumable.reviewRequired,
        can_resume: recoveredTasks.resumable.canResume,
        created_at: startedAt,
        completed_at: recoveredAt,
      });
    },
  );
};

test.describe('background task recovery semantics', () => {
  test('projects orphan recovery results into actionable task-center guidance', async ({ page }) => {
    await seedPersistedActiveTasks(page);
    await installApiMocks(page);

    await page.goto('/projects');
    await expect(page).toHaveURL(/\/projects$/);

    await expect.poll(async () => page.evaluate(() => {
      const persisted = localStorage.getItem('background-task-store');
      if (!persisted) return [];
      const state = JSON.parse(persisted) as {
        state?: { tasks?: Record<string, { status?: string }> };
      };
      return Object.values(state.state?.tasks ?? {}).map((task) => task.status);
    })).toEqual(['failed', 'failed', 'failed', 'failed']);

    const resumableTask = await page.evaluate((taskId) => {
      const persisted = localStorage.getItem('background-task-store');
      if (!persisted) return null;
      const state = JSON.parse(persisted) as {
        state?: { tasks?: Record<string, unknown> };
      };
      return state.state?.tasks?.[taskId] ?? null;
    }, recoveredTasks.resumable.taskId);
    expect(resumableTask).toMatchObject({
      taskId: recoveredTasks.resumable.taskId,
      taskType: recoveredTasks.resumable.taskType,
      status: 'failed',
      terminalReason: recoveredTasks.resumable.terminalReason,
      reviewRequired: recoveredTasks.resumable.reviewRequired,
      canResume: recoveredTasks.resumable.canResume,
      checkpoint: { completed_chapters: [1] },
    });

    await page.getByRole('button', { name: 'unordered-list', exact: true }).click();
    const drawer = page.locator('.ant-drawer-content');
    await expect(drawer).toBeVisible();
    await expect(drawer.getByText('后台任务 (4)', { exact: true })).toBeVisible();

    await expect(drawer.getByText('可从原业务入口重新发起', { exact: true }).first()).toBeVisible();
    await expect(drawer.getByText('可从检查点恢复', { exact: true }).first()).toBeVisible();
    await expect(drawer.getByText('需要人工确认', { exact: true }).first()).toBeVisible();
    await expect(drawer.getByText('当前任务不可恢复', { exact: true }).first()).toBeVisible();

    const resumableTaskItem = drawer.getByRole('listitem').filter({
      hasText: '当前任务保留了有效检查点，可使用“继续”从原业务恢复入口处理。',
    }).first();
    await expect(resumableTaskItem).toBeVisible();
    await expect(resumableTaskItem.getByRole('button', { name: /继续$/ })).toHaveCount(1);
  });
});
