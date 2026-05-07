import type {
  APIKeyPreset,
  PresetCreateRequest,
  PresetListResponse,
  PresetUpdateRequest,
  Settings,
  SettingsUpdate,
} from '../../types';
import { api } from '../core/httpClient';
import { isPlaceholderApiKey } from '../../utils/apiKey';

export type StoredApiKeyResponse = {
  api_key: string;
  has_api_key: boolean;
};

export type SettingsWithStoredApiKey = {
  settings: Settings;
  storedApiKey: string;
  hasStoredApiKey: boolean;
};

const sanitizeSettingsUpdate = (data: SettingsUpdate): SettingsUpdate => {
  const normalized: SettingsUpdate = { ...data };
  if (typeof normalized.api_key === 'string') {
    const trimmed = normalized.api_key.trim();
    if (!trimmed || isPlaceholderApiKey(trimmed)) {
      delete normalized.api_key;
    } else {
      normalized.api_key = trimmed;
    }
  }
  return normalized;
};

const getStoredApiKey = () =>
  api.get<unknown, StoredApiKeyResponse>('/settings/api-key');

const getSettingsWithStoredApiKey = async (): Promise<SettingsWithStoredApiKey> => {
  const [settings, storedApiKeyResponse] = await Promise.all([
    api.get<unknown, Settings>('/settings'),
    getStoredApiKey(),
  ]);

  const storedApiKey = String(storedApiKeyResponse.api_key || '').trim();

  return {
    settings,
    storedApiKey,
    hasStoredApiKey: Boolean(storedApiKeyResponse.has_api_key && storedApiKey),
  };
};

export const settingsApi = {
  getSettings: () => api.get<unknown, Settings>('/settings'),

  getStoredApiKey,

  getSettingsWithStoredApiKey,

  saveSettings: (data: SettingsUpdate) =>
    api.post<unknown, Settings>('/settings', sanitizeSettingsUpdate(data)),

  updateSettings: (data: SettingsUpdate) =>
    api.put<unknown, Settings>('/settings', sanitizeSettingsUpdate(data)),

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

  fetchModels: (params: { api_key: string; api_base_url: string; provider: string; models_url?: string }) =>
    api.post<unknown, {
      success: boolean;
      models: Array<{ id: string; owned_by: string | null }>;
      message?: string;
      error?: string;
      error_type?: string;
    }>('/settings/fetch-models', params),
};
