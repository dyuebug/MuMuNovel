import { expect, test, type Page, type Route } from '@playwright/test';

import {
  login,
  requiresRealBackend,
} from './helpers/backgroundTaskSmoke';

type BackgroundTaskCreateRequest = {
  task_type: string;
  project_id?: string;
  payload?: Record<string, unknown>;
};

type ResearchAsset = {
  title: string;
  source?: string;
  summary?: string;
};

type InspirationGenerateRequest = {
  step: 'title' | 'description' | 'theme' | 'genre';
  enable_web_research?: boolean;
  web_research_query?: string;
};

type InspirationGenerateResponse = {
  prompt?: string;
  options: string[];
  error?: string;
  research_query?: string;
  research_assets?: ResearchAsset[];
};

const fakeProjectId = 'mock-inspiration-project-id';
const carriedQuery = '法医职业细节与2026女频悬疑趋势';
const carriedAssets: ResearchAsset[] = [
  {
    title: '法医流程参考',
    source: 'https://example.com/forensics',
    summary: '法医到场后会先固定现场，再做尸表检验与时间判断。',
  },
  {
    title: '女频悬疑节奏参考',
    source: 'https://example.com/trends',
    summary: '开场前 3 章要尽快抛出身份反差、强钩子和倒计时压力。',
  },
];

const multiRoundResearch = {
  title: {
    query: '法医职业细节检索',
    assets: [
      {
        title: '法医术语参考',
        source: 'https://example.com/autopsy',
        summary: '尸表检验、死亡时间判断和现场固定是关键流程。',
      },
    ],
  },
  description: {
    query: '女频悬疑开篇节奏',
    assets: [
      {
        title: '悬疑开篇钩子参考',
        source: 'https://example.com/hooks',
        summary: '前三章要尽快抛出身份反差、悬念和明确倒计时。',
      },
    ],
  },
  theme: {
    query: '真相与自证主题表达',
    assets: [
      {
        title: '主题表达参考',
        source: 'https://example.com/theme',
        summary: '主题最好落到代价、信任与自我证明的撕扯。',
      },
    ],
  },
  genre: {
    query: '都市女性悬疑题材偏好',
    assets: [
      {
        title: '题材偏好参考',
        source: 'https://example.com/genre',
        summary: '都市悬疑常结合职业真实感与女性成长副线。',
      },
    ],
  },
} as const;

const buildTaskStatusResponse = (
  taskType: string,
  researchQuery: string,
  researchAssets: ResearchAsset[],
) => {
  const researchPayload = {
    research_query: researchQuery,
    research_assets: researchAssets,
  };

  switch (taskType) {
    case 'wizard_world_building':
      return {
        project_id: fakeProjectId,
        time_period: '现代都市',
        location: '临江旧城',
        atmosphere: '压迫而潮湿',
        rules: '真相每推进一步都要付出代价',
        ...researchPayload,
      };
    case 'wizard_career_system':
      return {
        project_id: fakeProjectId,
        main_careers_count: 1,
        sub_careers_count: 0,
        main_careers: ['法医'],
        sub_careers: [],
        ...researchPayload,
      };
    case 'wizard_characters':
      return {
        project_id: fakeProjectId,
        characters: [],
        ...researchPayload,
      };
    case 'wizard_outline':
      return {
        project_id: fakeProjectId,
        outlines: [],
        ...researchPayload,
      };
    default:
      return {
        project_id: fakeProjectId,
      };
  }
};

const setupMockBackgroundTasks = async (
  page: Page,
  options?: {
    researchQuery?: string;
    researchAssets?: ResearchAsset[];
  },
) => {
  const createdTasks: BackgroundTaskCreateRequest[] = [];
  const taskTypeById = new Map<string, string>();
  const researchQuery = options?.researchQuery ?? carriedQuery;
  const researchAssets = options?.researchAssets ?? carriedAssets;

  await page.route('**/api/background-tasks', async (route: Route) => {
    if (route.request().method() !== 'POST') {
      await route.continue();
      return;
    }

    const body = route.request().postDataJSON() as BackgroundTaskCreateRequest;
    createdTasks.push(body);
    const taskType = body.task_type;
    const taskId = `mock-${taskType}`;
    taskTypeById.set(taskId, taskType);

    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({
        task_id: taskId,
        task_type: taskType,
        project_id: body.project_id ?? (taskType === 'wizard_world_building' ? '' : fakeProjectId),
        status: 'pending',
        progress: 0,
        message: '后台任务已创建',
        result: null,
        error: null,
      }),
    });
  });

  await page.route('**/api/background-tasks/*', async (route: Route) => {
    if (route.request().method() !== 'GET') {
      await route.continue();
      return;
    }

    const taskId = route.request().url().split('/').pop() || '';
    const taskType = taskTypeById.get(taskId) || 'unknown';
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({
        task_id: taskId,
        task_type: taskType,
        project_id: fakeProjectId,
        status: 'completed',
        progress: 100,
        message: '任务完成',
        result: buildTaskStatusResponse(taskType, researchQuery, researchAssets),
        error: null,
      }),
    });
  });

  return createdTasks;
};

const ensureWebResearchEnabled = async (page: Page) => {
  const webResearchSwitch = page.getByRole('switch').first();
  await expect(webResearchSwitch).toBeVisible({ timeout: 15000 });
  const checked = await webResearchSwitch.getAttribute('aria-checked');
  if (checked !== 'true') {
    await webResearchSwitch.click();
  }
};

const openExecutionSettingsDialogFromConfirmState = async (page: Page) => {
  await page.getByText('✅ 确认创建', { exact: true }).click();
  const dialog = page.getByRole('dialog', { name: '执行设置' });
  await expect(dialog).toBeVisible({ timeout: 15000 });
  return dialog;
};

const openGenerationModalFromConfirmState = async (page: Page) => {
  const dialog = await openExecutionSettingsDialogFromConfirmState(page);
  await dialog.getByRole('button', { name: '开始生成', exact: true }).click();
};

const ideaInputPlaceholder = /女法医穿回案发前一天/;

test.describe('inspiration web research payload smoke', () => {
  test.beforeEach(async ({ page, context }) => {
    test.skip(requiresRealBackend, 'requires E2E_REAL_BACKEND=1 and a reachable real backend');
    await context.clearCookies();
    await page.goto('/login');
    await login(page);
    await page.evaluate(() => {
      localStorage.setItem('announcement_hide_forever', 'true');
    });
  });

  test('carries inspiration research assets into wizard background task payloads', async ({ page }) => {
    const createdTasks = await setupMockBackgroundTasks(page);

    const cacheData = {
      messages: [
        {
          type: 'ai',
          content: '你好！我是你的AI创作助手。',
        },
        {
          type: 'ai',
          content: '已整理你的灵感信息，请确认是否创建项目。',
          options: ['✅ 确认创建', '🔄 重新开始'],
        },
      ],
      currentStep: 'confirm',
      wizardData: {
        title: '雨夜尸检报告',
        description: '女法医在命案前一天醒来，必须在 24 小时内证明自己不是凶手。',
        theme: '真相与自证的代价',
        genre: ['悬疑', '都市', '女性成长'],
        narrative_perspective: '第三人称',
        outline_mode: 'one-to-many',
      },
      initialIdea: '女法医穿回案发前一天，必须在 24 小时内洗清自己杀人嫌疑。',
      selectedOptions: [],
      executionEnableWebResearch: true,
      executionWebResearchQuery: '',
      inspirationResearch: {
        query: carriedQuery,
        assets: carriedAssets,
      },
      lastFailedRequest: null,
      timestamp: Date.now(),
    };

    await page.goto('/inspiration');
    await page.waitForLoadState('networkidle');

    await page.evaluate((payload) => {
      localStorage.setItem('inspiration_conversation_cache', JSON.stringify(payload));
    }, cacheData);
    await page.reload();
    await page.waitForLoadState('networkidle');

    const carriedPreview = page.getByTestId('inspiration-research-preview');
    await expect(carriedPreview).toContainText(carriedQuery);
    await expect(carriedPreview).toContainText('法医流程参考');

    const executionDialog = await openExecutionSettingsDialogFromConfirmState(page);
    const executionSettingsPanel = executionDialog.getByTestId('generation-execution-settings-panel');
    const executionSettingsInfo = executionDialog.getByTestId('generation-execution-settings-info');
    await expect(executionSettingsPanel).toBeVisible();
    await expect(executionSettingsInfo).toContainText('联网搜索或研究增强');
    await expect(executionSettingsInfo).toContainText('页面侧配置');
    await executionDialog.getByRole('button', { name: '开始生成', exact: true }).click();

    const wizardResearchSummary = page.getByTestId('project-generator-research-summary');
    await expect(wizardResearchSummary).toContainText('本次联网研究摘要');
    await expect(wizardResearchSummary).toContainText('世界观设定');
    await expect(wizardResearchSummary).toContainText('职业体系');
    await expect(wizardResearchSummary).toContainText('角色设定');
    await expect(wizardResearchSummary).toContainText('大纲');
    await expect(wizardResearchSummary).toContainText(`检索词：${finalQuery}`);
    await expect(wizardResearchSummary).toContainText('来源：https://example.com/autopsy');
    await expect(wizardResearchSummary).toContainText('来源：https://example.com/hooks');
    await expect(wizardResearchSummary).toContainText('来源：https://example.com/theme');

    await expect.poll(() => createdTasks.length, { timeout: 15000 }).toBe(4);

    expect(createdTasks.map((item) => item.task_type)).toEqual([
      'wizard_world_building',
      'wizard_career_system',
      'wizard_characters',
      'wizard_outline',
    ]);

    for (const request of createdTasks) {
      expect(request.payload?.web_research_query).toBe(carriedQuery);
      expect(request.payload?.reference_research_assets).toEqual(carriedAssets);
    }

    expect(createdTasks[0].project_id).toBeUndefined();
    expect(createdTasks[1].project_id).toBe(fakeProjectId);
    expect(createdTasks[2].project_id).toBe(fakeProjectId);
    expect(createdTasks[3].project_id).toBe(fakeProjectId);
  });

  test('keeps cumulative inspiration research assets after reload and forwards them to generation tasks', async ({ page }) => {
    const generateRequests: InspirationGenerateRequest[] = [];
    const aggregatedAssets = [
      ...multiRoundResearch.title.assets,
      ...multiRoundResearch.description.assets,
      ...multiRoundResearch.theme.assets,
      ...multiRoundResearch.genre.assets,
    ];
    const finalQuery = multiRoundResearch.genre.query;
    const createdTasks = await setupMockBackgroundTasks(page, {
      researchQuery: finalQuery,
      researchAssets: aggregatedAssets,
    });

    await page.route('**/api/inspiration/generate-options', async (route: Route) => {
      const body = route.request().postDataJSON() as InspirationGenerateRequest;
      generateRequests.push(body);

      const responseMap: Record<InspirationGenerateRequest['step'], InspirationGenerateResponse> = {
        title: {
          prompt: '请选择更有记忆点的书名',
          options: ['雨夜尸检报告', '死者在明天醒来', '法医在黎明前翻案'],
          research_query: multiRoundResearch.title.query,
          research_assets: multiRoundResearch.title.assets,
        },
        description: {
          prompt: '请选择冲突更强的简介',
          options: [
            '她在案发前一天醒来，必须在 24 小时内证明自己不是凶手。',
            '死者还活着时，她已经拿到了指向自己的物证。',
            '如果这一次仍找不到真凶，她会成为明天唯一的嫌疑人。',
          ],
          research_query: multiRoundResearch.description.query,
          research_assets: multiRoundResearch.description.assets,
        },
        theme: {
          prompt: '请选择价值冲突最清晰的主题',
          options: ['真相与自证的代价', '信任比证据更晚到场', '每一次翻案都要重伤自己'],
          research_query: multiRoundResearch.theme.query,
          research_assets: multiRoundResearch.theme.assets,
        },
        genre: {
          prompt: '请选择类型标签',
          options: ['悬疑', '都市', '女性成长'],
          research_query: multiRoundResearch.genre.query,
          research_assets: multiRoundResearch.genre.assets,
        },
      };

      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify(responseMap[body.step]),
      });
    });

    await page.goto('/inspiration');
    await page.waitForLoadState('networkidle');
    await ensureWebResearchEnabled(page);

    await page.getByPlaceholder(ideaInputPlaceholder).fill('女法医穿回案发前一天，必须在24小时内洗清自己杀人嫌疑。');
    await page.getByRole('button', { name: /发送/ }).click();

    await page.getByText('雨夜尸检报告', { exact: true }).click();
    await expect(page.getByText('她在案发前一天醒来，必须在 24 小时内证明自己不是凶手。', { exact: true })).toBeVisible({ timeout: 15000 });
    await expect.poll(async () => page.evaluate(() => {
      const raw = localStorage.getItem('inspiration_conversation_cache');
      if (!raw) {
        return null;
      }
      const parsed = JSON.parse(raw) as {
        currentStep?: string;
        inspirationResearch?: { assets?: Array<unknown> };
      };
      return {
        currentStep: parsed.currentStep ?? null,
        assetsCount: Array.isArray(parsed.inspirationResearch?.assets)
          ? parsed.inspirationResearch.assets.length
          : 0,
      };
    }), { timeout: 15000 }).toEqual({ currentStep: 'description', assetsCount: 2 });

    await page.reload();
    await page.waitForLoadState('networkidle');
    await expect(page.getByText('她在案发前一天醒来，必须在 24 小时内证明自己不是凶手。', { exact: true })).toBeVisible({ timeout: 15000 });

    const aggregatedPreview = page.getByTestId('inspiration-research-preview');
    await expect(aggregatedPreview).toContainText(finalQuery);
    await expect(aggregatedPreview).toContainText('法医术语参考');
    await expect(aggregatedPreview).toContainText('悬疑开篇钩子参考');
    await expect(aggregatedPreview).toContainText('主题表达参考');
    await expect(aggregatedPreview).toContainText('还有 1 条已缓存资料会一并带入');

    await page.getByText('她在案发前一天醒来，必须在 24 小时内证明自己不是凶手。', { exact: true }).click();
    await page.getByText('真相与自证的代价', { exact: true }).click();
    await page.getByText('悬疑', { exact: true }).click();
    await page.getByRole('button', { name: /确认选择 \(1\)/ }).click();
    await page.getByText('第三人称', { exact: true }).click();
    await page.getByText('📚 一对多模式', { exact: true }).click();

    await openGenerationModalFromConfirmState(page);

    await expect.poll(() => createdTasks.length, { timeout: 15000 }).toBe(4);

    expect(generateRequests).toHaveLength(4);
    for (const request of generateRequests) {
      expect(request.enable_web_research).toBe(true);
    }

    for (const request of createdTasks) {
      expect(request.payload?.web_research_query).toBe(finalQuery);
      expect(request.payload?.reference_research_assets).toEqual(aggregatedAssets);
    }
  });
});
