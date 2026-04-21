import type {
  APIKeyPreset,
  PresetCreateRequest,
  PresetListResponse,
  PresetUpdateRequest,
  Settings,
  SettingsUpdate,
} from '../../types';
import { api } from '../core/httpClient';

export const settingsApi = {
  getSettings: () => api.get<unknown, Settings>('/settings'),

  saveSettings: (data: SettingsUpdate) =>
    api.post<unknown, Settings>('/settings', data),

  updateSettings: (data: SettingsUpdate) =>
    api.put<unknown, Settings>('/settings', data),

  deleteSettings: () => api.delete<unknown, { message: string; user_id: string }>('/settings'),

  getAvailableModels: (params: { api_key: string; api_base_url: string; provider: string }) =>
    api.get<unknown, { provider: string; models: Array<{ value: string; label: string; description: string }>; count?: number }>('/settings/models', { params }),

  testApiConnection: (params: { api_key: string; api_base_url: string; provider: string; llm_model: string; temperature?: number; max_tokens?: number; api_backup_urls?: string[]; fallback_strategy?: 'auto' | 'manual' }) =>
    api.post<unknown, {
      success: boolean;
      message: string;
      response_time_ms?: number;
      provider?: string;
      model?: string;
      response_preview?: string;
      details?: Record<string, unknown>;
      error?: string;
      error_type?: string;
      suggestions?: string[];
    }>('/settings/test', params),

  testWebResearchConnection: (params: {
    provider: 'exa' | 'grok';
    exa_api_key?: string;
    exa_base_url?: string;
    grok_api_key?: string;
    grok_base_url?: string;
    grok_model?: string;
    grok_search_enabled?: boolean;
    query?: string;
  }) =>
    api.post<unknown, {
      success: boolean;
      provider: string;
      message: string;
      response_preview?: string;
      result_count?: number;
      source_count?: number;
      search_status?: 'success_with_sources' | 'success_without_sources' | 'failed';
      status_note?: string;
      sources_backfilled?: boolean;
      error?: string;
      error_type?: string;
      suggestions?: string[];
    }>('/settings/test-web-research', params),

  checkFunctionCalling: (params: { api_key: string; api_base_url: string; provider: string; llm_model: string; api_backup_urls?: string[]; fallback_strategy?: 'auto' | 'manual' }) =>
    api.post<unknown, {
      success: boolean;
      supported: boolean | null;
      message: string;
      http_status?: number;
      response_time_ms?: number;
      provider?: string;
      model?: string;
      details?: Record<string, unknown>;
      tool_calls?: Array<{
        id?: string;
        type?: string;
        function?: {
          name: string;
          arguments: string;
        };
      }>;
      response_preview?: string;
      error?: string;
      error_type?: string;
      suggestions?: string[];
    }>('/settings/check-function-calling', params),

  getPresets: () =>
    api.get<unknown, PresetListResponse>('/settings/presets'),

  createPreset: (data: PresetCreateRequest) =>
    api.post<unknown, APIKeyPreset>('/settings/presets', data),

  updatePreset: (presetId: string, data: PresetUpdateRequest) =>
    api.put<unknown, APIKeyPreset>(`/settings/presets/${presetId}`, data),

  deletePreset: (presetId: string) =>
    api.delete<unknown, { message: string; preset_id: string }>(`/settings/presets/${presetId}`),

  activatePreset: (presetId: string) =>
    api.post<unknown, { message: string; preset_id: string; preset_name: string }>(`/settings/presets/${presetId}/activate`),

  testPreset: (presetId: string) =>
    api.post<unknown, {
      success: boolean;
      message: string;
      response_time_ms?: number;
      provider?: string;
      model?: string;
      response_preview?: string;
      details?: Record<string, boolean>;
      error?: string;
      error_type?: string;
      suggestions?: string[];
    }>(`/settings/presets/${presetId}/test`),

  createPresetFromCurrent: (name: string, description?: string) =>
    api.post<unknown, APIKeyPreset>('/settings/presets/from-current', null, {
      params: { name, description }
    }),
};
