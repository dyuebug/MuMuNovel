import { api } from '../core/httpClient';

type InspirationStep = 'title' | 'description' | 'theme' | 'genre';

type InspirationContext = {
  initial_idea?: string;
  title?: string;
  description?: string;
  theme?: string;
};

type InspirationResearchAsset = {
  title: string;
  source?: string;
  summary?: string;
};

type InspirationOptionResponse = {
  prompt?: string;
  options: string[];
  error?: string;
  research_query?: string;
  research_assets?: InspirationResearchAsset[];
};

export const inspirationApi = {
  generateOptions: (data: {
    step: InspirationStep;
    context: InspirationContext;
    enable_web_research?: boolean;
    web_research_query?: string;
  }) =>
    api.post<unknown, InspirationOptionResponse>('/inspiration/generate-options', data),

  refineOptions: (data: {
    step: InspirationStep;
    context: InspirationContext;
    feedback: string;
    previous_options?: string[];
    enable_web_research?: boolean;
    web_research_query?: string;
  }) =>
    api.post<unknown, InspirationOptionResponse>('/inspiration/refine-options', data),

  quickGenerate: (data: {
    title?: string;
    description?: string;
    theme?: string;
    genre?: string | string[];
    narrative_perspective?: string;
  }) =>
    api.post<unknown, {
      title: string;
      description: string;
      theme: string;
      genre: string[];
      narrative_perspective: string;
      error?: string;
    }>('/inspiration/quick-generate', data),
};