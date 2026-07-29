import { expect, test } from '@playwright/test';
import type { Page, Route } from '@playwright/test';

const projectId = 'workflow-state-e2e-project';

const fulfillJson = async (route: Route, body: unknown, status = 200) => {
  await route.fulfill({
    status,
    contentType: 'application/json; charset=utf-8',
    body: JSON.stringify(body),
  });
};

type WorkflowPhase =
  | 'inspiration'
  | 'foundation'
  | 'world_building'
  | 'character_design'
  | 'outline'
  | 'writing'
  | 'reviewing'
  | 'polishing'
  | 'completed';

type WorkflowState = {
  schema_version: 1;
  project_id: string;
  phase: WorkflowPhase;
  allowed_transitions: WorkflowPhase[];
  can_rollback: boolean;
  suggested_next_phase: WorkflowPhase | null;
  updated_at: string;
  source: 'projects.status';
};

const buildWorkflowState = (
  phase: WorkflowPhase,
  allowedTransitions: WorkflowPhase[],
  suggestedNextPhase: WorkflowPhase | null,
): WorkflowState => ({
  schema_version: 1,
  project_id: projectId,
  phase,
  allowed_transitions: allowedTransitions,
  can_rollback: phase !== 'inspiration',
  suggested_next_phase: suggestedNextPhase,
  updated_at: '2026-07-14T12:00:00',
  source: 'projects.status',
});

type InvocationHistoryMock = {
  body: unknown;
  status?: number;
};

type RuntimeMetricsMock = {
  body: unknown;
  status?: number;
};

const defaultRuntimeMetrics: RuntimeMetricsMock = {
  body: {
    schema_version: 'runtime-metrics/v1',
    read_model: 'derived_readonly',
    workflow: {
      state: 'available',
      schema_version: 1,
      phase: 'foundation',
      updated_at: '2026-07-16T12:00:00',
    },
    tasks: {
      state: 'empty',
      observed_limit: 100,
      observed_count: 0,
      pending_count: 0,
      running_count: 0,
      completed_count: 0,
      failed_count: 0,
      cancelled_count: 0,
    },
    quality: {
      state: 'unavailable',
      observed_limit: 0,
      total_chapters: null,
      analyzed_chapters: null,
      latest_overall_score: null,
      overall_score_delta: null,
      overall_score_trend: null,
      last_generated_at: null,
    },
    autopilot_audits: {
      state: 'empty',
      observed_limit: 100,
      observed_count: 0,
      queued_count: 0,
      running_count: 0,
      succeeded_count: 0,
      failed_count: 0,
      cancelled_count: 0,
    },
    prompt: '不应显示的 R8 Prompt',
    provider_name: '不应显示的 R8 Provider',
    model_name: '不应显示的 R8 Model',
    digest: '不应显示的 R8 摘要',
  },
};

const defaultInvocationHistory: InvocationHistoryMock = {
  body: {
    items: [
      {
        audit_id: 'audit-succeeded',
        tool_name: 'transition_project_workflow',
        tool_schema_version: 'autopilot-tool-contract/v1',
        confirmed_by_user: true,
        execution_mode: 'direct_business_tool',
        input_summary: {
          expected_phase: 'foundation',
          target_phase: 'world_building',
          reason_provided: true,
          related_task_id_provided: true,
        },
        status: 'succeeded',
        result_summary: {
          changed: true,
          previous_phase: 'foundation',
          current_phase: 'world_building',
        },
        error_code: null,
        created_at: '2026-07-16T12:00:00',
        started_at: '2026-07-16T12:00:02',
        completed_at: '2026-07-16T12:00:04',
        arguments: '{"reason":"不应显示的原始参数"}',
        reason: '不应显示的原因',
        prompt: '不应显示的 Prompt',
        provider_name: '不应显示的 Provider',
        model_name: '不应显示的 Model',
        prompt_digest: '不应显示的摘要',
      },
      {
        audit_id: 'audit-failed',
        tool_name: 'transition_project_workflow',
        tool_schema_version: 'autopilot-tool-contract/v1',
        confirmed_by_user: true,
        execution_mode: 'direct_business_tool',
        input_summary: {
          expected_phase: 'world_building',
          target_phase: 'character_design',
          reason_provided: false,
          related_task_id_provided: false,
        },
        status: 'failed',
        result_summary: null,
        error_code: 'stale_expected_phase',
        created_at: '2026-07-16T11:00:00',
        started_at: '2026-07-16T11:00:01',
        completed_at: '2026-07-16T11:00:02',
      },
    ],
  },
};

const installApiMocks = async (
  page: Page,
  options: {
    invocationHistory?: InvocationHistoryMock;
    runtimeMetrics?: RuntimeMetricsMock;
  } = {},
) => {
  let workflowState = buildWorkflowState(
    'foundation',
    ['inspiration', 'world_building', 'writing'],
    'world_building',
  );
  const transitionPayloads: Array<Record<string, unknown>> = [];
  const autopilotPayloads: Array<Record<string, unknown>> = [];
  const backgroundTasks: Array<Record<string, unknown>> = [];
  const invocationHistory = options.invocationHistory ?? defaultInvocationHistory;
  const runtimeMetrics = options.runtimeMetrics ?? defaultRuntimeMetrics;
  let invocationHistoryRequestCount = 0;
  let runtimeMetricsRequestCount = 0;

  await page.route('**/api/**', async (route) => {
    await fulfillJson(route, {});
  });

  await page.route('**/api/auth/user', async (route) => {
    await fulfillJson(route, {
      id: 'workflow-state-user',
      username: 'workflow-state-user',
      is_admin: true,
    });
  });

  await page.route('**/api/background-tasks**', async (route) => {
    const request = route.request();
    const pathname = new URL(request.url()).pathname;

    if (pathname === '/api/background-tasks' && request.method() === 'GET') {
      await fulfillJson(route, { total: backgroundTasks.length, items: backgroundTasks });
      return;
    }

    const task = backgroundTasks.find((item) => pathname.endsWith(`/${item.task_id}`));
    if (task && request.method() === 'GET') {
      await fulfillJson(route, task);
      return;
    }

    await fulfillJson(route, { detail: 'unexpected background task API request' }, 500);
  });

  await page.route(`**/api/projects/${projectId}**`, async (route) => {
    const request = route.request();
    const pathname = new URL(request.url()).pathname;

    if (pathname === `/api/projects/${projectId}` && request.method() === 'GET') {
      await fulfillJson(route, {
        id: projectId,
        title: 'Workflow State E2E',
        description: 'Project used by the workflow state contract test',
        theme: 'contract testing',
        genre: 'fantasy',
        target_words: 100000,
        current_words: 25000,
        status: workflowState.phase,
        wizard_status: 'completed',
        wizard_step: 4,
        outline_mode: 'one-to-many',
        chapter_count: 3,
        character_count: 2,
        created_at: '2026-07-14T10:00:00',
        updated_at: '2026-07-14T12:00:00',
      });
      return;
    }

    if (
      pathname === `/api/projects/${projectId}/workflow-state`
      && request.method() === 'GET'
    ) {
      await fulfillJson(route, workflowState);
      return;
    }

    if (
      pathname === `/api/projects/${projectId}/runtime-metrics`
      && request.method() === 'GET'
    ) {
      runtimeMetricsRequestCount += 1;
      await fulfillJson(route, runtimeMetrics.body, runtimeMetrics.status);
      return;
    }

    if (
      pathname === `/api/projects/${projectId}/autopilot/invocations`
      && request.method() === 'GET'
    ) {
      invocationHistoryRequestCount += 1;
      await fulfillJson(route, invocationHistory.body, invocationHistory.status);
      return;
    }

    if (
      pathname === `/api/projects/${projectId}/autopilot/actions`
      && request.method() === 'POST'
    ) {
      const payload = request.postDataJSON() as Record<string, unknown>;
      autopilotPayloads.push(payload);
      const task = {
        task_id: 'workflow-state-autopilot-task',
        task_type: 'novel_autopilot',
        project_id: projectId,
        status: 'pending',
        progress: 0,
        message: '后台受控切换任务已进入队列',
        result: null,
        error: null,
        created_at: '2026-07-16T12:00:00',
        updated_at: '2026-07-16T12:00:00',
      };
      backgroundTasks.push(task);
      await fulfillJson(route, task);
      return;
    }

    if (
      pathname === `/api/projects/${projectId}/workflow-state/transition`
      && request.method() === 'POST'
    ) {
      const payload = request.postDataJSON() as Record<string, unknown>;
      transitionPayloads.push(payload);

      if (transitionPayloads.length === 1) {
        workflowState = buildWorkflowState(
          'world_building',
          ['foundation', 'character_design'],
          'character_design',
        );
        await fulfillJson(route, {
          schema_version: 1,
          changed: true,
          previous_phase: 'foundation',
          state: workflowState,
        });
        return;
      }

      workflowState = buildWorkflowState(
        'character_design',
        ['world_building', 'outline'],
        'outline',
      );
      await fulfillJson(route, { detail: '创作阶段已被其他操作更新' }, 409);
      return;
    }

    await fulfillJson(route, { detail: 'unexpected project API request' }, 500);
  });

  return {
    autopilotPayloads,
    invocationHistoryRequestCount: () => invocationHistoryRequestCount,
    runtimeMetricsRequestCount: () => runtimeMetricsRequestCount,
    transitionPayloads,
  };
};

test.describe('project workflow state', () => {
  test('uses server transitions, requires rollback reason, and refreshes after conflict', async ({ page }) => {
    await page.addInitScript(() => {
      localStorage.setItem('announcement_hide_forever', 'true');
    });
    const { transitionPayloads } = await installApiMocks(page);

    await page.goto(`/project/${projectId}/world-setting`);

    await expect(page.getByText('当前：项目奠基', { exact: true })).toBeVisible();
    await expect(page.getByText('建议下一步：世界构建', { exact: true })).toBeVisible();

    await page.getByRole('button', { name: '切换阶段' }).click();
    await expect(page.getByText('灵感构思', { exact: true })).toBeVisible();
    await expect(page.getByText('世界构建', { exact: true })).toBeVisible();
    await expect(page.getByText('正文创作', { exact: true })).toBeVisible();
    await expect(page.getByText('角色设计', { exact: true })).toHaveCount(0);
    await page.getByText('世界构建', { exact: true }).click();

    await expect(page.getByText('当前：世界构建', { exact: true })).toBeVisible();
    expect(transitionPayloads[0]).toEqual({
      target_phase: 'world_building',
      expected_phase: 'foundation',
    });

    await page.getByRole('button', { name: '切换阶段' }).click();
    await page.getByText('项目奠基', { exact: true }).click();

    const rollbackDialog = page.getByRole('dialog', { name: '确认回退创作阶段' });
    await expect(rollbackDialog).toBeVisible();
    const confirmButton = rollbackDialog.getByRole('button', { name: '确认切换' });
    await expect(confirmButton).toBeDisabled();
    await rollbackDialog.getByPlaceholder('请输入回退原因（必填）').fill('修正世界观设定冲突');
    await expect(confirmButton).toBeEnabled();
    await confirmButton.click();

    await expect(page.getByText('当前：角色设计', { exact: true })).toBeVisible();
    await expect(page.getByText('创作阶段已被其他操作更新，已刷新最新状态')).toBeVisible();
    expect(transitionPayloads[1]).toEqual({
      target_phase: 'foundation',
      expected_phase: 'world_building',
      reason: '修正世界观设定冲突',
    });
  });

  test('shows readonly runtime metrics with safe empty and unavailable states', async ({ page }) => {
    await page.addInitScript(() => {
      localStorage.setItem('announcement_hide_forever', 'true');
    });
    const { runtimeMetricsRequestCount } = await installApiMocks(page);

    await page.goto(`/project/${projectId}/world-setting`);

    const projectPageContent = page.getByTestId('project-page-content');
    const metricsButton = page.getByRole('button', { name: '打开运行指标' });
    await expect(projectPageContent).toBeVisible();
    await expect(metricsButton).toBeVisible();
    expect(runtimeMetricsRequestCount()).toBe(0);

    const contentBeforeOpen = await projectPageContent.boundingBox();
    await metricsButton.click();

    const metricsDrawer = page.getByRole('dialog', { name: '运行指标' });
    const metricsPanel = metricsDrawer.locator('.ant-card').filter({ hasText: '派生只读' });
    await expect(metricsDrawer).toBeVisible();
    await expect(metricsPanel).toBeVisible();
    await expect(metricsPanel.getByText('派生只读', { exact: true })).toBeVisible();
    await expect(metricsPanel.getByText('不自动刷新', { exact: true })).toBeVisible();
    await expect(metricsPanel.getByText('阶段：foundation', { exact: true })).toBeVisible();
    await expect(metricsPanel.getByText('暂无记录', { exact: true })).toHaveCount(2);
    await expect(metricsPanel.getByText('暂不可用', { exact: true })).toHaveCount(1);
    await expect(metricsPanel.getByText(/固定上限的运行时观测样本/)).toBeVisible();
    await expect(metricsPanel.getByRole('button')).toHaveCount(0);
    await expect(metricsPanel.getByText('不应显示的 R8 Prompt')).toHaveCount(0);
    await expect(metricsPanel.getByText('不应显示的 R8 Provider')).toHaveCount(0);
    await expect(metricsPanel.getByText('不应显示的 R8 Model')).toHaveCount(0);
    await expect(metricsPanel.getByText('不应显示的 R8 摘要')).toHaveCount(0);

    const contentAfterOpen = await projectPageContent.boundingBox();
    expect(contentBeforeOpen).not.toBeNull();
    expect(contentAfterOpen).not.toBeNull();
    expect(contentAfterOpen?.height).toBeCloseTo(contentBeforeOpen?.height ?? 0, 0);
    expect(runtimeMetricsRequestCount()).toBe(1);

    await page.keyboard.press('Escape');
    await expect(metricsDrawer).toBeHidden();
  });

  test('shows a privacy-safe readonly invocation audit history without task controls', async ({ page }) => {
    await page.addInitScript(() => {
      localStorage.setItem('announcement_hide_forever', 'true');
    });
    const { autopilotPayloads, invocationHistoryRequestCount, transitionPayloads } = await installApiMocks(page);

    await page.goto(`/project/${projectId}/world-setting`);
    await page.getByRole('button', { name: '受控调用记录' }).click();

    const dialog = page.getByRole('dialog', { name: '受控调用记录' });
    await expect(dialog).toBeVisible();
    await expect(dialog.getByText('transition_project_workflow', { exact: true })).toHaveCount(2);
    await expect(dialog.getByText('已完成', { exact: true })).toBeVisible();
    await expect(dialog.getByText('执行失败', { exact: true })).toBeVisible();
    await expect(dialog.getByText('已人工确认', { exact: true })).toHaveCount(2);
    await expect(dialog.getByText(/请求阶段：项目奠基\s*→\s*世界构建/)).toBeVisible();
    await expect(dialog.getByText(/执行结果：已变更，\s*项目奠基\s*→\s*世界构建/)).toBeVisible();
    await expect(dialog.getByText('错误码：stale_expected_phase', { exact: true })).toBeVisible();
    await expect(dialog.getByText('不应显示的原始参数')).toHaveCount(0);
    await expect(dialog.getByText('不应显示的原因')).toHaveCount(0);
    await expect(dialog.getByText('不应显示的 Prompt')).toHaveCount(0);
    await expect(dialog.getByText('不应显示的 Provider')).toHaveCount(0);
    await expect(dialog.getByText('不应显示的 Model')).toHaveCount(0);
    await expect(dialog.getByText('不应显示的摘要')).toHaveCount(0);
    await expect(dialog.getByRole('button', { name: /暂停|恢复|重试|重放|引导/ })).toHaveCount(0);
    expect(invocationHistoryRequestCount()).toBe(1);
    expect(transitionPayloads).toEqual([]);
    expect(autopilotPayloads).toEqual([]);
  });

  test('shows an empty state for readonly invocation audit history', async ({ page }) => {
    await page.addInitScript(() => {
      localStorage.setItem('announcement_hide_forever', 'true');
    });
    await installApiMocks(page, { invocationHistory: { body: { items: [] } } });

    await page.goto(`/project/${projectId}/world-setting`);
    await page.getByRole('button', { name: '受控调用记录' }).click();
    await expect(
      page.getByRole('dialog', { name: '受控调用记录' }).getByText('暂无受控调用记录'),
    ).toBeVisible();
  });

  test('shows a safe error state for readonly invocation audit history', async ({ page }) => {
    await page.addInitScript(() => {
      localStorage.setItem('announcement_hide_forever', 'true');
    });
    await installApiMocks(page, {
      invocationHistory: {
        status: 500,
        body: { detail: '受控调用记录暂时无法读取' },
      },
    });

    await page.goto(`/project/${projectId}/world-setting`);
    await page.getByRole('button', { name: '受控调用记录' }).click();
    await expect(page.getByText('受控调用记录暂时无法读取', { exact: true })).toBeVisible();
  });

  test('queues a confirmed background-controlled transition without optimistic workflow mutation', async ({ page }) => {
    await page.addInitScript(() => {
      localStorage.setItem('announcement_hide_forever', 'true');
    });
    const { autopilotPayloads } = await installApiMocks(page);

    await page.goto(`/project/${projectId}/world-setting`);
    await expect(page.getByText('当前：项目奠基', { exact: true })).toBeVisible();

    await page.getByRole('button', { name: '后台受控切换' }).click();
    const autopilotMenu = page.getByRole('menu').last();
    await autopilotMenu.getByText('世界构建', { exact: true }).click();

    const dialog = page.getByRole('dialog', { name: '确认后台受控切换' });
    await expect(dialog).toBeVisible();
    await dialog.getByPlaceholder('可选：补充本次后台受控切换的原因').fill('由人工确认后交给后台执行');
    await dialog.getByRole('button', { name: '确认创建任务' }).click();

    await expect(page.getByText('后台受控切换任务已创建，可在后台任务中心查看进度')).toBeVisible();
    await expect(page.getByText('当前：项目奠基', { exact: true })).toBeVisible();
    await expect(page.getByText('后台任务 · 当前项目优先 (1)', { exact: true })).toBeVisible();
    expect(autopilotPayloads).toEqual([
      {
        tool_name: 'transition_project_workflow',
        arguments: {
          expected_phase: 'foundation',
          target_phase: 'world_building',
          reason: '由人工确认后交给后台执行',
        },
        confirmed_by_user: true,
      },
    ]);
  });

});
