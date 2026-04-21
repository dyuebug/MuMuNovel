import { ssePost } from '../../utils/sseClient';
import type { SSEClientOptions } from '../../utils/sseClient';
import type {
  CreativeMode,
  GenerateCharactersResponse,
  GenerateOutlineResponse,
  PlotStage,
  QualityPreset,
  ResearchAssetSummary,
  StoryFocus,
  WorldBuildingResponse,
} from '../../types';
import { runBackgroundTaskWithPolling } from './wizardBackgroundPolling';

export const wizardStreamApi = {
  generateWorldBuildingStream: (
    data: {
      title: string;
      description: string;
      theme: string;
      genre: string | string[];
      narrative_perspective?: string;
      target_words?: number;
      chapter_count?: number;
      character_count?: number;
      outline_mode?: 'one-to-one' | 'one-to-many';
      default_creative_mode?: CreativeMode;
      default_story_focus?: StoryFocus;
      default_plot_stage?: PlotStage;
      default_story_creation_brief?: string;
      default_quality_preset?: QualityPreset;
      default_quality_notes?: string;
      provider?: string;
      model?: string;
      enable_web_research?: boolean;
      web_research_query?: string;
      reference_research_assets?: ResearchAssetSummary[];
    },
    options?: SSEClientOptions<WorldBuildingResponse>,
  ) => runBackgroundTaskWithPolling<WorldBuildingResponse>(
    'wizard_world_building',
    undefined,
    data as Record<string, unknown>,
    options,
  ),

  generateCharactersStream: (
    data: {
      project_id: string;
      count?: number;
      world_context?: Record<string, string>;
      theme?: string;
      genre?: string;
      requirements?: string;
      provider?: string;
      model?: string;
      enable_web_research?: boolean;
      web_research_query?: string;
      reference_research_assets?: ResearchAssetSummary[];
    },
    options?: SSEClientOptions<GenerateCharactersResponse>,
  ) => runBackgroundTaskWithPolling<GenerateCharactersResponse>(
    'wizard_characters',
    data.project_id,
    data as Record<string, unknown>,
    options,
  ),

  generateCareerSystemStream: (
    data: {
      project_id: string;
      provider?: string;
      model?: string;
      enable_web_research?: boolean;
      web_research_query?: string;
      reference_research_assets?: ResearchAssetSummary[];
    },
    options?: SSEClientOptions<{
      project_id: string;
      main_careers_count: number;
      sub_careers_count: number;
      main_careers: string[];
      sub_careers: string[];
      research_query?: string;
      research_assets?: Array<{
        title: string;
        source?: string;
        summary?: string;
        usage_hint?: string;
        asset_type?: string;
      }>;
    }>,
  ) => runBackgroundTaskWithPolling<{
    project_id: string;
    main_careers_count: number;
    sub_careers_count: number;
    main_careers: string[];
    sub_careers: string[];
    research_query?: string;
    research_assets?: Array<{
      title: string;
      source?: string;
      summary?: string;
      usage_hint?: string;
      asset_type?: string;
    }>;
  }>(
    'wizard_career_system',
    data.project_id,
    data as Record<string, unknown>,
    options,
  ),

  generateCompleteOutlineStream: (
    data: {
      project_id: string;
      chapter_count: number;
      narrative_perspective: string;
      target_words?: number;
      requirements?: string;
      provider?: string;
      model?: string;
      creative_mode?: CreativeMode;
      story_focus?: StoryFocus;
      plot_stage?: PlotStage;
      story_creation_brief?: string;
      quality_preset?: QualityPreset;
      quality_notes?: string;
      enable_web_research?: boolean;
      web_research_query?: string;
    },
    options?: SSEClientOptions<GenerateOutlineResponse>,
  ) => runBackgroundTaskWithPolling<GenerateOutlineResponse>(
    'wizard_outline',
    data.project_id,
    data as Record<string, unknown>,
    options,
  ),

  updateWorldBuildingStream: (
    projectId: string,
    data: {
      time_period?: string;
      location?: string;
      atmosphere?: string;
      rules?: string;
    },
    options?: SSEClientOptions<WorldBuildingResponse>,
  ) => ssePost<WorldBuildingResponse>(
    `/api/wizard-stream/world-building/${projectId}`,
    data,
    options,
  ),

  regenerateWorldBuildingStream: (
    projectId: string,
    data?: {
      provider?: string;
      model?: string;
    },
    options?: SSEClientOptions<WorldBuildingResponse>,
  ) => ssePost<WorldBuildingResponse>(
    `/api/wizard-stream/world-building/${projectId}/regenerate`,
    data || {},
    options,
  ),

  cleanupWizardDataStream: (
    projectId: string,
    options?: SSEClientOptions,
  ) => ssePost<{ message: string; deleted: { characters: number; outlines: number; chapters: number } }>(
    `/api/wizard-stream/cleanup/${projectId}`,
    {},
    options,
  ),
};