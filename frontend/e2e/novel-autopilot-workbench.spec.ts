import { expect, test } from '@playwright/test';
import type { Page, Route } from '@playwright/test';

import type {
  CreateNovelAutopilotRunRequest,
  NovelAutopilotRun,
  NovelAutopilotRunStatus,
  NovelAutopilotStepRun,
} from '../src/features/novel-autopilot/types';

const projectId = 'novel-autopilot-workbench-e2e-project';
const runId = 'novel-autopilot-run-e2e';
const taskId = 'novel-autopilot-task-e2e';
const longStepErrorCode = 'novel_autopilot_outline_expansion_provider_temporarily_unavailable';

const fulfillJson = async (route: Route, body: unknown, status = 200) => {
  await route.fulfill({
    status,
    contentType: 'application/json; charset=utf-8',
    body: JSON.stringify(body),
  });
};

const buildRun = (
  status: NovelAutopilotRunStatus = 'running',
  overrides: Partial<NovelAutopilotRun> = {},
): NovelAutopilotRun => ({
  id: runId,
  project_id: projectId,
  schema_version: 'novel-autopilot-run/v1',
  status,
  current_phase: status === 'completed' ? 'completed' : 'chapter_loop',
  current_step: status === 'completed' ? null : 'chapter_generate',
  current_chapter_id: status === 'completed' ? null : 'chapter-3',
  current_chapter_number: status === 'completed' ? null : 3,
  total_chapters: 12,
  completed_chapters: status === 'completed' ? 12 : 2,
  failed_chapter_count: 1,
  pending_rewrite_count: 2,
  total_word_count: status === 'completed' ? 86000 : 12400,
  execution_scope: 'complete_book',
  human_gate_mode: 'high_risk_only',
  gate_interval: 5,
  max_chapters: 20,
  max_tokens: 4000000,
  max_estimated_cost: 12.5,
  max_runtime_seconds: 604800,
  next_chapter_count: null,
  max_step_attempts: 3,
  max_consecutive_provider_failures: 5,
  max_consecutive_quality_failures: 4,
  regenerate_existing: false,
  run_book_review: true,
  run_book_polish: true,
  export_format: 'txt',
  used_tokens: 123456,
  estimated_cost: 1.2345,
  epoch: 4,
  version: 11,
  consecutive_provider_failures: 2,
  consecutive_quality_failures: 1,
  last_error_code: null,
  has_guidance: false,
  active_background_task_id: status === 'running' ? taskId : null,
  final_export_ref: null,
  created_at: '2026-07-19T08:00:00+08:00',
  updated_at: '2026-07-19T09:30:00+08:00',
  started_at: '2026-07-19T08:01:00+08:00',
  paused_at: status === 'paused' ? '2026-07-19T09:00:00+08:00' : null,
  completed_at: status === 'completed' ? '2026-07-19T09:30:00+08:00' : null,
  ...overrides,
});

const buildSteps = (): NovelAutopilotStepRun[] => [
  {
    id: 'step-chapter-analyze',
    run_id: runId,
    step_key: 'chapter:0002:analyze',
    step_type: 'chapter_analyze',
    phase: 'chapter_loop',
    chapter_id: 'chapter-2',
    chapter_number: 2,
    attempt: 1,
    run_epoch: 4,
    status: 'completed',
    background_task_id: 'task-chapter-analyze',
    quality_decision: 'accept',
    error_code: null,
    started_at: '2026-07-19T08:01:00+08:00',
    completed_at: '2026-07-19T08:02:00+08:00',
    created_at: '2026-07-19T08:01:00+08:00',
    updated_at: '2026-07-19T08:02:00+08:00',
  },
  {
    id: 'step-outline-expand',
    run_id: runId,
    step_key: 'planning:outline_expand:0001:outline-1',
    step_type: 'outline_expand',
    phase: 'outline',
    chapter_id: null,
    chapter_number: null,
    attempt: 1,
    run_epoch: 4,
    status: 'failed',
    background_task_id: 'task-outline-expand',
    quality_decision: null,
    error_code: longStepErrorCode,
    started_at: '2026-07-19T08:10:00+08:00',
    completed_at: '2026-07-19T08:11:00+08:00',
    created_at: '2026-07-19T08:10:00+08:00',
    updated_at: '2026-07-19T08:11:00+08:00',
  },
  {
    id: 'step-chapter-3',
    run_id: runId,
    step_key: 'chapter:0003:generate',
    step_type: 'chapter_generate',
    phase: 'chapter_loop',
    chapter_id: 'chapter-3',
    chapter_number: 3,
    attempt: 2,
    run_epoch: 4,
    status: 'running',
    background_task_id: taskId,
    quality_decision: null,
    error_code: null,
    started_at: '2026-07-19T09:29:00+08:00',
    completed_at: null,
    created_at: '2026-07-19T09:29:00+08:00',
    updated_at: '2026-07-19T09:30:00+08:00',
  },
];

type ApiMockOptions = {
  initialRun?: NovelAutopilotRun | null;
  steps?: NovelAutopilotStepRun[];
  streamOutput?: boolean;
};

type RecordedRequest = {
  action: 'create' | 'pause' | 'resume' | 'cancel' | 'guidance' | 'decision';
  body: Record<string, unknown>;
};

const installApiMocks = async (page: Page, options: ApiMockOptions = {}) => {
  let run = options.initialRun === undefined ? buildRun() : options.initialRun;
  const steps = options.steps ?? buildSteps();
  const requests: RecordedRequest[] = [];
  let streamRequestCount = 0;

  await page.route('**/api/**', async (route) => {
    await fulfillJson(route, {});
  });

  await page.route('**/api/auth/user', async (route) => {
    await fulfillJson(route, {
      id: 'novel-autopilot-e2e-user',
      username: 'novel-autopilot-e2e-user',
      is_admin: true,
    });
  });

  await page.route('**/api/background-tasks**', async (route) => {
    const pathname = new URL(route.request().url()).pathname;
    if (pathname === '/api/background-tasks') {
      await fulfillJson(route, { total: 0, items: [] });
      return;
    }
    await fulfillJson(route, { detail: 'unexpected background task request' }, 404);
  });

  await page.route(`**/api/projects/${projectId}**`, async (route) => {
    const request = route.request();
    const pathname = new URL(request.url()).pathname;
    const runsPath = `/api/projects/${projectId}/novel-autopilot-runs`;
    const currentRunPath = `${runsPath}/${runId}`;

    if (pathname === `/api/projects/${projectId}` && request.method() === 'GET') {
      await fulfillJson(route, {
        id: projectId,
        title: 'Durable Autopilot E2E',
        description: '自动完整成书工作台测试项目',
        theme: '持久化自动创作',
        genre: 'fantasy',
        target_words: 100000,
        current_words: run?.total_word_count ?? 0,
        status: run?.status === 'completed' ? 'completed' : 'writing',
        wizard_status: 'completed',
        wizard_step: 4,
        outline_mode: 'one-to-many',
        chapter_count: run?.completed_chapters ?? 0,
        character_count: 3,
        created_at: '2026-07-19T08:00:00+08:00',
        updated_at: '2026-07-19T09:30:00+08:00',
      });
      return;
    }

    if (pathname === `/api/projects/${projectId}/workflow-state` && request.method() === 'GET') {
      await fulfillJson(route, {
        schema_version: 1,
        project_id: projectId,
        phase: run?.status === 'completed' ? 'completed' : 'writing',
        allowed_transitions: [],
        can_rollback: true,
        suggested_next_phase: null,
        updated_at: '2026-07-19T09:30:00+08:00',
        source: 'projects.status',
      });
      return;
    }

    if (pathname === `/api/projects/${projectId}/runtime-metrics` && request.method() === 'GET') {
      await fulfillJson(route, {
        schema_version: 'runtime-metrics/v1',
        read_model: 'derived_readonly',
        workflow: { state: 'available', schema_version: 1, phase: 'writing', updated_at: '2026-07-19T09:30:00+08:00' },
        tasks: { state: 'empty', observed_limit: 100, observed_count: 0, pending_count: 0, running_count: 0, completed_count: 0, failed_count: 0, cancelled_count: 0 },
        quality: { state: 'unavailable', observed_limit: 0, total_chapters: null, analyzed_chapters: null, latest_overall_score: null, overall_score_delta: null, overall_score_trend: null, last_generated_at: null },
        autopilot_audits: { state: 'empty', observed_limit: 100, observed_count: 0, queued_count: 0, running_count: 0, succeeded_count: 0, failed_count: 0, cancelled_count: 0 },
      });
      return;
    }

    if (pathname === runsPath && request.method() === 'GET') {
      await fulfillJson(route, { items: run ? [run] : [] });
      return;
    }

    if (pathname === runsPath && request.method() === 'POST') {
      const body = request.postDataJSON() as unknown as CreateNovelAutopilotRunRequest;
      requests.push({ action: 'create', body: body as unknown as Record<string, unknown> });
      run = buildRun('queued', {
        version: 1,
        epoch: 1,
        current_phase: 'validate',
        current_step: 'validate',
        current_chapter_id: null,
        current_chapter_number: null,
        completed_chapters: 0,
        total_word_count: 0,
        active_background_task_id: taskId,
      });
      await fulfillJson(route, {
        created: true,
        run,
        background_task: {
          task_id: taskId,
          task_type: 'novel_autopilot_run_tick',
          status: 'pending',
          progress: 0,
          message: 'Durable Run 已进入队列',
        },
      });
      return;
    }

    if (pathname === `${currentRunPath}/steps` && request.method() === 'GET') {
      await fulfillJson(route, { items: run ? steps : [] });
      return;
    }

    if (pathname === currentRunPath && request.method() === 'GET') {
      await fulfillJson(route, { run });
      return;
    }

    const mutation = async (
      action: RecordedRequest['action'],
      nextStatus: NovelAutopilotRunStatus,
      overrides: Partial<NovelAutopilotRun> = {},
    ) => {
      const body = request.postDataJSON() as Record<string, unknown>;
      requests.push({ action, body });
      if (!run) {
        await fulfillJson(route, { detail: 'run not found' }, 404);
        return;
      }
      run = {
        ...run,
        status: nextStatus,
        version: run.version + 1,
        active_background_task_id: nextStatus === 'running' ? taskId : null,
        paused_at: nextStatus === 'paused' ? '2026-07-19T09:31:00+08:00' : run.paused_at,
        updated_at: '2026-07-19T09:31:00+08:00',
        ...overrides,
      };
      await fulfillJson(route, { run });
    };

    if (pathname === `${currentRunPath}/pause` && request.method() === 'POST') {
      await mutation('pause', 'paused');
      return;
    }
    if (pathname === `${currentRunPath}/resume` && request.method() === 'POST') {
      await mutation('resume', 'running', { paused_at: null });
      return;
    }
    if (pathname === `${currentRunPath}/cancel` && request.method() === 'POST') {
      await mutation('cancel', 'cancelled');
      return;
    }
    if (pathname === `${currentRunPath}/guidance` && request.method() === 'POST') {
      await mutation('guidance', run?.status ?? 'paused', { has_guidance: true, epoch: (run?.epoch ?? 0) + 1 });
      return;
    }
    if (pathname === `${currentRunPath}/decision` && request.method() === 'POST') {
      const decision = (request.postDataJSON() as Record<string, unknown>).decision;
      await mutation('decision', decision === 'stop' ? 'cancelled' : 'running');
      return;
    }

    await fulfillJson(route, { detail: `unexpected project API request: ${request.method()} ${pathname}` }, 500);
  });

  if (options.streamOutput) {
    await page.route(`**/api/background-tasks/${taskId}/stream`, async (route) => {
      streamRequestCount += 1;
      await route.fulfill({
        status: 200,
        headers: {
          'Content-Type': 'text/event-stream; charset=utf-8',
          'Cache-Control': 'no-cache',
        },
        body: [
          `data: ${JSON.stringify({ type: 'reasoning_chunk', content: '先核对章节大纲，再生成候选正文。' })}`,
          '',
          `data: ${JSON.stringify({ type: 'chunk', content: '第三章正文实时预览。' })}`,
          '',
          `data: ${JSON.stringify({ type: 'done' })}`,
          '',
        ].join('\n'),
      });
    });
  }

  return {
    requests,
    getRun: () => run,
    streamRequestCount: () => streamRequestCount,
  };
};

const preparePage = async (page: Page, preferences?: Record<string, boolean>) => {
  await page.addInitScript((values: Record<string, boolean>) => {
    localStorage.setItem('announcement_hide_forever', 'true');
    Object.entries(values).forEach(([key, value]) => {
      localStorage.setItem(key, String(value));
    });
  }, preferences ?? {});
};

const preferenceSwitch = (page: Page, label: '运行状态' | 'Provider 思考' | '生成内容') => (
  page.getByTestId('novel-autopilot-workbench').getByRole('switch', { name: label })
);

test.describe('durable novel autopilot workbench', () => {
  test('creates a complete-book Run from the empty state', async ({ page }) => {
    await preparePage(page);
    const api = await installApiMocks(page, { initialRun: null, steps: [] });

    await page.goto(`/project/${projectId}/autopilot`);

    await expect(page).toHaveURL(new RegExp(`/project/${projectId}/autopilot$`));
    await expect(page.getByRole('heading', { name: '自动创作工作台' })).toBeVisible();
    await expect(page.getByText('当前项目还没有自动创作 Run，请配置后启动。')).toBeVisible();
    await expect(page.getByText('当前只生成缺失章节')).toBeVisible();
    await expect(page.getByText(/不支持覆盖或重新生成既有章节/)).toBeVisible();
    await expect(page.getByText('重生成既有章节')).toHaveCount(0);

    await page.locator('.ant-form-item').filter({ hasText: '人工门策略' }).locator('.ant-select-selector').click();
    await expect(page.getByRole('option', { name: '每卷确认' })).toHaveCount(0);
    await page.keyboard.press('Escape');

    await page.getByRole('button', { name: '启动自动创作' }).click();

    await expect(page.getByText('自动创作 Run 已创建')).toBeVisible();
    await expect(page.getByText('排队中', { exact: true }).first()).toBeVisible();
    expect(api.requests).toHaveLength(1);
    expect(api.requests[0]).toMatchObject({
      action: 'create',
      body: {
        total_chapters: 100,
        config: {
          execution_scope: 'complete_book',
          human_gate_mode: 'high_risk_only',
          max_chapters: 200,
          max_tokens: 4000000,
          regenerate_existing: false,
          run_book_review: true,
          run_book_polish: true,
          export_format: 'txt',
        },
      },
    });
  });


  test('renders legacy Run values without exposing them as create options', async ({ page }) => {
    await preparePage(page);
    await installApiMocks(page, {
      initialRun: buildRun('completed', {
        human_gate_mode: 'every_volume',
        export_format: 'markdown',
      }),
    });

    await page.goto(`/project/${projectId}/autopilot`);

    await expect(page.getByText('每卷确认')).toBeVisible();
    await expect(page.getByText('Markdown')).toBeVisible();
  });

  test('restores an active Run with sticky metrics, limits, timeline, and live model output', async ({ page }) => {
    await preparePage(page, {
      'mumu-novel-autopilot-show-provider-reasoning': false,
      'mumu-novel-autopilot-show-generated-content': false,
    });
    const api = await installApiMocks(page, { streamOutput: true });

    await page.goto(`/project/${projectId}/autopilot`);

    await expect(page.getByText('运行中', { exact: true }).first()).toBeVisible();
    await expect(page.getByText('Provider 连续失败：2 / 5')).toBeVisible();
    await expect(page.getByText('质量连续失败：1 / 4')).toBeVisible();
    await expect(page.getByText('当前 Step 尝试：2 / 3')).toBeVisible();
    await expect(page.getByText('已观察输出 Token（估算）')).toBeVisible();
    await expect(page.getByText('成本预算（需 Provider 计价）')).toBeVisible();
    await expect(page.getByText('步骤时间线（3）')).toBeVisible();
    await expect(page.getByText('章节分析', { exact: true })).toBeVisible();
    await expect(page.getByText('一纲多章展开')).toBeVisible();
    await expect(page.getByText('planning:outline_expand:0001:outline-1')).toBeVisible();
    await expect(page.getByText('chapter:0003:generate', { exact: true })).toHaveCount(0);

    const timelineCard = page.locator('.ant-card').filter({ hasText: '步骤时间线（3）' });
    const stepHeaderBox = await timelineCard.getByRole('columnheader', { name: '步骤' }).boundingBox();
    const chapterHeaderBox = await timelineCard.getByRole('columnheader', { name: '章节' }).boundingBox();
    const attemptHeader = timelineCard.getByRole('columnheader', { name: '尝试' });
    const attemptHeaderBox = await attemptHeader.boundingBox();
    const statusHeader = timelineCard.getByRole('columnheader', { name: '状态' });
    const statusHeaderBox = await statusHeader.boundingBox();
    const qualityDecisionHeader = timelineCard.getByRole('columnheader', { name: '质量决定' });
    const qualityDecisionHeaderBox = await qualityDecisionHeader.boundingBox();
    const updatedHeaderBox = await timelineCard.getByRole('columnheader', { name: '更新时间' }).boundingBox();
    const analysisRow = timelineCard.getByRole('row').filter({ hasText: '章节分析' });
    const analysisRowBox = await analysisRow.boundingBox();
    const timelineViewport = await timelineCard.locator('.ant-table-content').evaluate((element) => ({
      clientWidth: element.clientWidth,
      scrollWidth: element.scrollWidth,
    }));
    expect(stepHeaderBox?.width).toBeGreaterThanOrEqual(210);
    expect(chapterHeaderBox?.width).toBeGreaterThanOrEqual(65);
    expect(chapterHeaderBox?.width).toBeLessThanOrEqual(90);
    expect(attemptHeaderBox?.width).toBeGreaterThanOrEqual(55);
    expect(attemptHeaderBox?.width).toBeLessThanOrEqual(75);
    expect(statusHeaderBox?.width).toBeGreaterThanOrEqual(84);
    expect(statusHeaderBox?.width).toBeLessThanOrEqual(104);
    expect(qualityDecisionHeaderBox?.width).toBeGreaterThanOrEqual(96);
    expect(qualityDecisionHeaderBox?.width).toBeLessThanOrEqual(116);
    expect(updatedHeaderBox?.width).toBeGreaterThanOrEqual(145);
    expect(updatedHeaderBox?.width).toBeLessThanOrEqual(190);
    expect((stepHeaderBox?.width ?? 0) / (updatedHeaderBox?.width ?? 1)).toBeGreaterThanOrEqual(1.25);
    expect(timelineViewport.scrollWidth).toBeLessThanOrEqual(timelineViewport.clientWidth + 1);
    expect(analysisRowBox?.height).toBeLessThanOrEqual(64);
    await expect(analysisRow.getByText('第2章', { exact: true })).toHaveCSS('white-space', 'nowrap');
    const analysisAttemptCell = analysisRow.getByRole('cell').nth(2);
    await expect(analysisAttemptCell).toHaveText('1次');
    await expect(analysisAttemptCell.locator('span')).toHaveCSS('white-space', 'nowrap');
    await expect(attemptHeader).toHaveCSS('text-align', 'center');
    await expect(analysisAttemptCell).toHaveCSS('text-align', 'center');
    const analysisStatusCell = analysisRow.getByRole('cell').nth(3);
    const analysisStatusTag = analysisStatusCell.locator('.ant-tag');
    await expect(analysisStatusCell).toHaveText('完成');
    await expect(statusHeader).toHaveCSS('text-align', 'center');
    await expect(analysisStatusCell).toHaveCSS('text-align', 'center');
    await expect(analysisStatusTag).toHaveCSS('white-space', 'nowrap');
    await expect(analysisStatusTag).toHaveCSS('margin-right', '0px');
    const analysisQualityDecisionCell = analysisRow.getByRole('cell').nth(4);
    const analysisQualityDecisionTag = analysisQualityDecisionCell.locator('.ant-tag');
    await expect(analysisQualityDecisionCell).toHaveText('通过');
    await expect(qualityDecisionHeader).toHaveCSS('text-align', 'center');
    await expect(analysisQualityDecisionCell).toHaveCSS('text-align', 'center');
    await expect(analysisQualityDecisionTag).toHaveCSS('white-space', 'nowrap');
    await expect(analysisQualityDecisionTag).toHaveCSS('margin-right', '0px');
    const errorCodeHeader = timelineCard.getByRole('columnheader', { name: '错误代码' });
    const errorCodeHeaderBox = await errorCodeHeader.boundingBox();
    const failedOutlineRow = timelineCard.getByRole('row').filter({ hasText: longStepErrorCode });
    const failedOutlineErrorCell = failedOutlineRow.getByRole('cell').nth(5);
    const failedOutlineErrorText = failedOutlineErrorCell.locator('.ant-typography');
    expect(errorCodeHeaderBox?.width).toBeGreaterThanOrEqual(130);
    expect(errorCodeHeaderBox?.width).toBeLessThanOrEqual(170);
    await expect(errorCodeHeader).toHaveCSS('text-align', 'left');
    await expect(failedOutlineErrorCell).toHaveCSS('text-align', 'left');
    await expect(failedOutlineErrorText).toHaveCSS('white-space', 'nowrap');
    await expect(failedOutlineErrorText).toHaveCSS('text-overflow', 'ellipsis');
    await failedOutlineErrorText.hover();
    await expect(page.getByRole('tooltip', { name: longStepErrorCode })).toHaveText(longStepErrorCode);

    const analysisStepLabel = analysisRow.getByText('章节分析', { exact: true });
    await expect(analysisRow.getByText('chapter:0002:analyze', { exact: true })).toHaveCount(0);
    await analysisStepLabel.hover();
    await expect(page.getByRole('tooltip', { name: 'chapter:0002:analyze' })).toHaveText('chapter:0002:analyze');
    await expect(page.locator('div[style*="position: sticky"]').filter({ hasText: '运行指标' })).toBeVisible();

    await expect.poll(api.streamRequestCount).toBe(1);
    await expect(page.getByText('Provider 思考过程（临时）')).toHaveCount(0);
    await expect(page.getByText('模型生成内容（临时预览）')).toHaveCount(0);

    await preferenceSwitch(page, 'Provider 思考').click();
    await preferenceSwitch(page, '生成内容').click();

    await expect(page.getByText('Provider 思考过程（临时）')).toBeVisible();
    await expect(page.getByText('先核对章节大纲，再生成候选正文。')).toBeVisible();
    await expect(page.getByText('模型生成内容（临时预览）')).toBeVisible();
    await expect(page.getByText('第三章正文实时预览。')).toBeVisible();
    expect(api.streamRequestCount()).toBe(1);

    await page.getByRole('button', { name: /暂停$/ }).click();
    await expect(page.getByText('已请求暂停自动创作')).toBeVisible();
    await expect(page.getByText('先核对章节大纲，再生成候选正文。')).toBeVisible();
    await expect(page.getByText('第三章正文实时预览。')).toBeVisible();
  });

  test('explains a fail-closed cost budget gate without claiming fabricated pricing', async ({ page }) => {
    await preparePage(page);
    await installApiMocks(page, {
      initialRun: buildRun('waiting_human', {
        active_background_task_id: null,
        last_error_code: 'novel_autopilot_cost_estimation_unavailable',
      }),
    });

    await page.goto(`/project/${projectId}/autopilot`);

    await expect(page.getByText('运行已记录错误')).toBeVisible();
    await expect(page.getByText(/当前 Provider 尚无统一计价来源/)).toBeVisible();
    await expect(page.getByText(/novel_autopilot_cost_estimation_unavailable/)).toBeVisible();

    await page.getByText('启动新的完整成书 Run').click();
    await expect(page.getByText(/不会使用伪造价格继续运行/)).toBeVisible();
  });

  test('sends CAS versions for pause, resume, and cancel', async ({ page }) => {
    await preparePage(page);
    const api = await installApiMocks(page, { initialRun: buildRun('running', { version: 11 }) });

    await page.goto(`/project/${projectId}/autopilot`);
    await page.getByRole('button', { name: /暂停$/ }).click();
    await expect(page.getByText('已请求暂停自动创作')).toBeVisible();

    await page.getByRole('button', { name: /恢复$/ }).click();
    await expect(page.getByText('已恢复自动创作')).toBeVisible();

    await page.getByRole('button', { name: /取消$/ }).click();
    await page.getByRole('button', { name: '确 定' }).click();
    await expect(page.getByText('自动创作已取消')).toBeVisible();

    expect(api.requests.filter(({ action }) => ['pause', 'resume', 'cancel'].includes(action))).toEqual([
      { action: 'pause', body: { expected_version: 11 } },
      { action: 'resume', body: { expected_version: 12 } },
      { action: 'cancel', body: { expected_version: 13 } },
    ]);
  });

  test('updates guidance only while paused and advances the public fence', async ({ page }) => {
    await preparePage(page);
    const api = await installApiMocks(page, {
      initialRun: buildRun('paused', { version: 12, epoch: 7, active_background_task_id: null }),
    });

    await page.goto(`/project/${projectId}/autopilot`);
    await page.getByPlaceholder('例如：后续三章降低战斗密度，加强人物关系推进，并保持既有世界观约束。')
      .fill('后续三章降低战斗密度，加强人物关系推进。');
    await page.getByRole('button', { name: '保存后续指导' }).click();

    await expect(page.getByText('后续指导已更新')).toBeVisible();
    expect(api.requests.at(-1)).toEqual({
      action: 'guidance',
      body: {
        expected_version: 12,
        guidance: '后续三章降低战斗密度，加强人物关系推进。',
      },
    });
    expect(api.getRun()).toMatchObject({ version: 13, epoch: 8, has_guidance: true });
  });

  test('submits a waiting-human decision with optional guidance', async ({ page }) => {
    await preparePage(page);
    const api = await installApiMocks(page, {
      initialRun: buildRun('waiting_human', { version: 12, active_background_task_id: null }),
    });

    await page.goto(`/project/${projectId}/autopilot`);
    await page.getByPlaceholder('例如：后续三章降低战斗密度，加强人物关系推进，并保持既有世界观约束。')
      .fill('保留冲突结果，但补强人物动机。');
    await page.getByRole('button', { name: '接受并继续' }).click();

    await expect(page.getByText('人工决定已提交')).toBeVisible();
    expect(api.requests.at(-1)).toEqual({
      action: 'decision',
      body: {
        expected_version: 12,
        decision: 'accept',
        guidance: '保留冲突结果，但补强人物动机。',
      },
    });
  });

  test('shows a validated final export descriptor for a completed book', async ({ page }) => {
    await preparePage(page);
    const descriptor = {
      schema_version: 'project-export-artifact/v1',
      format: 'txt',
      filename: 'Durable-Autopilot-E2E.txt',
      chapter_count: 12,
      total_word_count: 86000,
      content_digest: 'sha256:durable-autopilot-e2e',
    };
    await installApiMocks(page, {
      initialRun: buildRun('completed', {
        version: 20,
        epoch: 9,
        final_export_ref: JSON.stringify(descriptor),
      }),
      steps: buildSteps().map((step) => ({ ...step, status: 'completed' })),
    });

    await page.goto(`/project/${projectId}/autopilot`);

    await expect(page.getByText('最终导出产物')).toBeVisible();
    await expect(page.getByText('已校验描述符')).toBeVisible();
    await expect(page.getByText('Durable-Autopilot-E2E.txt')).toBeVisible();
    const exportCard = page.getByText('最终导出产物').locator('xpath=ancestor::div[contains(@class, "ant-card")]');
    await expect(exportCard.getByText('86,000')).toBeVisible();
    await expect(page.getByText('sha256:durable-autopilot-e2e')).toBeVisible();
  });
});
