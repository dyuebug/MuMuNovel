import { api } from '../core/httpClient';
import { waitForBackgroundTaskCompletion } from '../../utils/taskPolling';
import { backgroundTaskApi } from './backgroundTasks';

export type InspirationStep = 'title' | 'description' | 'theme' | 'genre';

export type InspirationContext = {
  initial_idea?: string;
  title?: string;
  description?: string;
  theme?: string;
};

export type InspirationResearchAsset = {
  title: string;
  source?: string;
  summary?: string;
};

export type InspirationOptionResponse = {
  prompt?: string;
  options: string[];
  error?: string;
  research_query?: string;
  research_assets?: InspirationResearchAsset[];
};

export type InspirationQuickGenerateResponse = {
  title: string;
  description: string;
  theme: string;
  genre: string[];
  narrative_perspective: string;
  error?: string;
};

type GenerateOptionsPayload = {
  step: InspirationStep;
  context: InspirationContext;
  enable_web_research?: boolean;
  web_research_query?: string;
};

type RefineOptionsPayload = {
  step: InspirationStep;
  context: InspirationContext;
  feedback: string;
  previous_options?: string[];
  enable_web_research?: boolean;
  web_research_query?: string;
};

type QuickGeneratePayload = {
  title?: string;
  description?: string;
  theme?: string;
  genre?: string | string[];
  narrative_perspective?: string;
};

const isInspirationStep = (value: unknown): value is InspirationStep => (
  value === 'title' || value === 'description' || value === 'theme' || value === 'genre'
);

const buildInspirationTaskCheckpoint = (
  taskType: 'inspiration_generate_options' | 'inspiration_refine_options' | 'inspiration_quick_generate',
  payload: Record<string, unknown>,
) => {
  const checkpoint: Record<string, unknown> = {
    source: 'inspiration',
    inspiration_action: taskType.replace('inspiration_', ''),
  };

  if (isInspirationStep(payload.step)) {
    checkpoint.inspiration_step = payload.step;
  }

  return checkpoint;
};

const runInspirationBackgroundTask = async <T>(
  taskType: 'inspiration_generate_options' | 'inspiration_refine_options' | 'inspiration_quick_generate',
  payload: Record<string, unknown>,
) => {
  const createdTask = await backgroundTaskApi.createTask({
    task_type: taskType,
    payload,
    checkpoint: buildInspirationTaskCheckpoint(taskType, payload),
  });

  return waitForBackgroundTaskCompletion<typeof createdTask, T>(createdTask, {
    pollTask: backgroundTaskApi.getTaskStatus,
    progressMessage: '任务已创建，正在生成中',
    resolveValue: (task) => ((task.result as T) ?? ({} as T)),
  });
};

export const inspirationApi = {
  generateOptions: (data: GenerateOptionsPayload) =>
    api.post<unknown, InspirationOptionResponse>('/inspiration/generate-options', data),

  generateOptionsInBackground: (data: GenerateOptionsPayload) =>
    runInspirationBackgroundTask<InspirationOptionResponse>(
      'inspiration_generate_options',
      data as Record<string, unknown>,
    ),

  refineOptions: (data: RefineOptionsPayload) =>
    api.post<unknown, InspirationOptionResponse>('/inspiration/refine-options', data),

  refineOptionsInBackground: (data: RefineOptionsPayload) =>
    runInspirationBackgroundTask<InspirationOptionResponse>(
      'inspiration_refine_options',
      data as Record<string, unknown>,
    ),

  quickGenerate: (data: QuickGeneratePayload) =>
    api.post<unknown, InspirationQuickGenerateResponse>('/inspiration/quick-generate', data),

  quickGenerateInBackground: (data: QuickGeneratePayload) =>
    runInspirationBackgroundTask<InspirationQuickGenerateResponse>(
      'inspiration_quick_generate',
      data as Record<string, unknown>,
    ),
};
