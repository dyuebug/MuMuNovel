import axios from 'axios';
import { message } from 'antd';
import { ssePost } from '../utils/sseClient';
import type { SSEClientOptions } from '../utils/sseClient';
import { useBackgroundTaskStore } from '../store/backgroundTasks';
import type {
  User,
  AuthUrlResponse,
  Project,
  ProjectCreate,
  ProjectUpdate,
  WorldBuildingResponse,
  Outline,
  OutlineCreate,
  OutlineUpdate,
  OutlineReorderRequest,
  OutlineExpansionRequest,
  OutlineExpansionResponse,
  BatchOutlineExpansionRequest,
  BatchOutlineExpansionResponse,
  Character,
  CharacterUpdate,
  Chapter,
  ChapterCreate,
  ChapterUpdate,
  GenerateOutlineRequest,
  GenerateCharacterRequest,
  PolishTextRequest,
  GenerateCharactersResponse,
  GenerateOutlineResponse,
  Settings,
  SettingsUpdate,
  WritingStyle,
  WritingStyleCreate,
  WritingStyleUpdate,
  PresetStyle,
  WritingStyleListResponse,
  PromptWorkshopListResponse,
  PromptWorkshopItem,
  PromptSubmission,
  PromptSubmissionCreate,
  MCPPlugin,
  MCPPluginCreate,
  MCPPluginUpdate,
  MCPTestResult,
  MCPTool,
  MCPToolCallRequest,
  MCPToolCallResponse,
  APIKeyPreset,
  PresetCreateRequest,
  PresetUpdateRequest,
  PresetListResponse,
  ChapterPlanItem,
} from '../types';

interface MCPPluginSimpleCreate {
  config_json: string;
  enabled: boolean;
}

interface RequestConfigWithToastControl {
  suppressErrorToast?: boolean;
  params?: Record<string, unknown>;
}

const ERROR_TOAST_THROTTLE_MS = 3000;
const ERROR_TOAST_CACHE_RETENTION_MS = 120000;
const errorToastTimestamps = new Map<string, number>();

const pruneErrorToastCache = (now: number) => {
  for (const [messageText, timestamp] of errorToastTimestamps.entries()) {
    if (now - timestamp > ERROR_TOAST_CACHE_RETENTION_MS) {
      errorToastTimestamps.delete(messageText);
    }
  }
};

const showErrorToastWithThrottle = (errorMessage: string) => {
  const now = Date.now();
  const lastTimestamp = errorToastTimestamps.get(errorMessage);
  if (lastTimestamp && now - lastTimestamp < ERROR_TOAST_THROTTLE_MS) {
    return;
  }

  errorToastTimestamps.set(errorMessage, now);
  pruneErrorToastCache(now);
  message.error(errorMessage);
};

const silentRequestConfig = <T extends RequestConfigWithToastControl>(config?: T): T =>
  ({ ...(config || {}), suppressErrorToast: true } as T);

const formatChapterAnalysisError = (
  errorCode?: import('../types').AnalysisTask['error_code'],
  errorMessage?: string | null
): string | null => {
  if (!errorCode && !errorMessage) {
    return null;
  }

  if (errorCode === 'json_parse_failed') {
    return '章节分析失败：AI 返回结果格式异常，系统已自动重试；如果仍失败，请再次重试分析';
  }

  if (errorCode === 'ai_empty') {
    return '章节分析失败：AI 返回内容不足，未能生成有效分析结果，请稍后重试';
  }

  if (errorCode === 'stream_interrupted') {
    return '章节分析失败：AI 流式响应中断，可能是模型服务波动或代理中断，请稍后重试';
  }

  if (errorCode === 'timeout') {
    return '章节分析超时：后台长时间未完成分析，请稍后刷新后重试';
  }

  if (errorCode === 'chapter_empty') {
    return '章节分析失败：当前章节内容为空，无法进行分析';
  }

  if (errorCode === 'project_missing') {
    return '章节分析失败：关联项目不存在，请刷新页面后重试';
  }

  if (errorCode === 'retrying') {
    return errorMessage;
  }

  if (errorMessage?.includes('JSON解析失败') || errorMessage?.includes('AI返回格式异常')) {
    return '章节分析失败：AI 返回结果格式异常，系统已自动重试；如果仍失败，请再次重试分析';
  }

  if (errorMessage?.includes('AI响应为空或过短')) {
    return '章节分析失败：AI 返回内容不足，未能生成有效分析结果，请稍后重试';
  }

  if (errorMessage?.includes('流式响应中断') || errorMessage?.includes('流式生成出错')) {
    return '章节分析失败：AI 流式响应中断，可能是模型服务波动或代理中断，请稍后重试';
  }

  if (errorMessage?.includes('任务超时')) {
    return '章节分析超时：后台长时间未完成分析，请稍后刷新后重试';
  }

  return errorMessage;
};

const api = axios.create({
  baseURL: '/api',
  timeout: 120000,
  headers: {
    'Content-Type': 'application/json',
  },
  withCredentials: true,
});

api.interceptors.request.use(
  (config) => {
    return config;
  },
  (error) => {
    return Promise.reject(error);
  }
);

api.interceptors.response.use(
  (response) => {
    return response.data;
  },
  (error) => {
    const requestConfig = (error?.config || {}) as RequestConfigWithToastControl;
    const suppressErrorToast = Boolean(requestConfig.suppressErrorToast);
    let errorMessage = '请求失败';

    if (error.response) {
      const status = error.response.status;
      const data = error.response.data;

      switch (status) {
        case 400:
          errorMessage = data?.detail || '请求参数错误';
          break;
        case 401:
          errorMessage = '未授权，请先登录';
          if (window.location.pathname !== '/login') {
            window.location.href = '/login';
          }
          break;
        case 403:
          errorMessage = '没有权限访问';
          break;
        case 404:
          errorMessage = data?.detail || '请求的资源不存在';
          break;
        case 422:
          errorMessage = data?.detail || '请求参数验证失败';
          if (data?.errors) {
            console.error('验证错误详情:', data.errors);
          }
          break;
        case 500:
          errorMessage = data?.detail || '服务器内部错误';
          break;
        case 503:
          errorMessage = '服务暂时不可用，请稍后重试';
          break;
        default:
          errorMessage = data?.detail || data?.message || `请求失败 (${status})`;
      }
    } else if (error.request) {
      const errorCode = typeof error.code === 'string' ? error.code : '';
      const rawMessage = typeof error.message === 'string' ? error.message : '';
      const isOffline = typeof navigator !== 'undefined' && navigator.onLine === false;
      const isTimeout = errorCode === 'ECONNABORTED' || /timeout/i.test(rawMessage);

      if (isOffline) {
        errorMessage = '网络连接已断开，请检查当前网络';
      } else if (isTimeout) {
        errorMessage = '请求超时，服务响应较慢，请稍后重试';
      } else {
        errorMessage = '服务暂时无响应，可能是网络波动、后端异常或代理中断，请稍后重试';
      }
    } else {
      errorMessage = error.message || '请求失败';
    }

    if (typeof error === 'object' && error !== null) {
      error.message = errorMessage;
    }

    if (!suppressErrorToast) {
      showErrorToastWithThrottle(errorMessage);
    }
    console.error('API Error:', errorMessage, error);

    return Promise.reject(error);
  }
);

export const authApi = {
  getAuthConfig: () => api.get<unknown, { local_auth_enabled: boolean; linuxdo_enabled: boolean }>('/auth/config'),

  localLogin: (username: string, password: string) =>
    api.post<unknown, { success: boolean; message: string; user: User }>('/auth/local/login', { username, password }),

  bindAccountLogin: (username: string, password: string) =>
    api.post<unknown, { success: boolean; message: string; user: User }>('/auth/bind/login', { username, password }),

  getLinuxDOAuthUrl: () => api.get<unknown, AuthUrlResponse>('/auth/linuxdo/url'),

  getCurrentUser: () => api.get<unknown, User>('/auth/user'),

  getPasswordStatus: () => api.get<unknown, {
    has_password: boolean;
    has_custom_password: boolean;
    username: string | null;
    default_password: string | null;
  }>('/auth/password/status'),

  setPassword: (password: string) =>
    api.post<unknown, { success: boolean; message: string }>('/auth/password/set', { password }),

  initializePassword: (password: string) =>
    api.post<unknown, { success: boolean; message: string }>('/auth/password/initialize', { password }),

  refreshSession: () => api.post<unknown, { message: string; expire_at: number; remaining_minutes: number }>('/auth/refresh'),

  logout: () => api.post('/auth/logout'),
};

export const userApi = {
  getCurrentUser: () => api.get<unknown, User>('/users/current'),

  listUsers: () => api.get<unknown, User[]>('/users'),

  setAdmin: (userId: string, isAdmin: boolean) =>
    api.post('/users/set-admin', { user_id: userId, is_admin: isAdmin }),

  deleteUser: (userId: string) => api.delete(`/users/${userId}`),

  getUser: (userId: string) => api.get<unknown, User>(`/users/${userId}`),

  resetPassword: (userId: string, newPassword?: string) =>
    api.post<unknown, {
      message: string;
      user_id: string;
      username: string;
      default_password?: string;
    }>('/users/reset-password', { user_id: userId, new_password: newPassword }),
};

export const settingsApi = {
  getSettings: () => api.get<unknown, Settings>('/settings'),

  saveSettings: (data: SettingsUpdate) =>
    api.post<unknown, Settings>('/settings', data),

  updateSettings: (data: SettingsUpdate) =>
    api.put<unknown, Settings>('/settings', data),

  deleteSettings: () => api.delete<unknown, { message: string; user_id: string }>('/settings'),

  getAvailableModels: (params: { api_key: string; api_base_url: string; provider: string }) =>
    api.get<unknown, { provider: string; models: Array<{ value: string; label: string; description: string }>; count?: number }>('/settings/models', { params }),

  testApiConnection: (params: { api_key: string; api_base_url: string; provider: string; llm_model: string; temperature?: number; max_tokens?: number }) =>
    api.post<unknown, {
      success: boolean;
      message: string;
      response_time_ms?: number;
      provider?: string;
      model?: string;
      response_preview?: string;
      details?: Record<string, boolean | number>;
      error?: string;
      error_type?: string;
      suggestions?: string[];
    }>('/settings/test', params),

  checkFunctionCalling: (params: { api_key: string; api_base_url: string; provider: string; llm_model: string }) =>
    api.post<unknown, {
      success: boolean;
      supported: boolean;
      message: string;
      response_time_ms?: number;
      provider?: string;
      model?: string;
      details?: {
        finish_reason?: string;
        has_tool_calls?: boolean;
        tool_call_count?: number;
        test_tool?: string;
        test_prompt?: string;
        response_type?: string;
      };
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

  // API配置预设管理
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

export const projectApi = {
  getProjects: () => api.get<unknown, Project[]>('/projects'),

  getProject: (id: string) => api.get<unknown, Project>(`/projects/${id}`),

  createProject: (data: ProjectCreate) => api.post<unknown, Project>('/projects', data),

  updateProject: (id: string, data: ProjectUpdate) =>
    api.put<unknown, Project>(`/projects/${id}`, data),

  deleteProject: (id: string) => api.delete(`/projects/${id}`),

  exportProject: (id: string) => {
    window.open(`/api/projects/${id}/export`, '_blank');
  },

  // 导出项目数据为JSON
  exportProjectData: async (id: string, options: {
    include_generation_history?: boolean;
    include_writing_styles?: boolean;
    include_careers?: boolean;
    include_memories?: boolean;
    include_plot_analysis?: boolean;
  }) => {
    const response = await axios.post(
      `/api/projects/${id}/export-data`,
      options,
      {
        responseType: 'blob',
        headers: {
          'Content-Type': 'application/json',
        },
      }
    );

    // 从响应头获取文件名
    const contentDisposition = response.headers['content-disposition'];
    let filename = 'project_export.json';
    if (contentDisposition) {
      const matches = /filename\*=UTF-8''(.+)/.exec(contentDisposition);
      if (matches && matches[1]) {
        filename = decodeURIComponent(matches[1]);
      }
    }

    // 创建下载链接
    const url = window.URL.createObjectURL(new Blob([response.data]));
    const link = document.createElement('a');
    link.href = url;
    link.setAttribute('download', filename);
    document.body.appendChild(link);
    link.click();
    link.remove();
    window.URL.revokeObjectURL(url);
  },

  // 验证导入文件
  validateImportFile: (file: File) => {
    const formData = new FormData();
    formData.append('file', file);
    return api.post<unknown, {
      valid: boolean;
      version: string;
      project_name?: string;
      statistics: Record<string, number>;
      errors: string[];
      warnings: string[];
    }>('/projects/validate-import', formData, {
      headers: { 'Content-Type': 'multipart/form-data' },
    });
  },

  // 导入项目
  importProject: (file: File) => {
    const formData = new FormData();
    formData.append('file', file);
    return api.post<unknown, {
      success: boolean;
      project_id?: string;
      message: string;
      statistics: Record<string, number>;
      warnings: string[];
    }>('/projects/import', formData, {
      headers: { 'Content-Type': 'multipart/form-data' },
    });
  },
};

export const outlineApi = {
  getOutlines: (projectId: string) =>
    api.get<unknown, { total: number; items: Outline[] }>(`/outlines/project/${projectId}`).then(res => res.items),

  getOutline: (id: string) => api.get<unknown, Outline>(`/outlines/${id}`),

  createOutline: (data: OutlineCreate) => api.post<unknown, Outline>('/outlines', data),

  updateOutline: (id: string, data: OutlineUpdate) =>
    api.put<unknown, Outline>(`/outlines/${id}`, data),

  deleteOutline: (id: string) => api.delete(`/outlines/${id}`),

  reorderOutlines: (data: OutlineReorderRequest) =>
    api.post<unknown, { message: string; updated_outlines: number; updated_chapters: number }>('/outlines/reorder', data),

  generateOutline: (data: GenerateOutlineRequest) =>
    api.post<unknown, { total: number; items: Outline[] }>('/outlines/generate', data).then(res => res.items),

  // 获取大纲关联的章节
  getOutlineChapters: (outlineId: string) =>
    api.get<unknown, {
      has_chapters: boolean;
      outline_id: string;
      outline_title: string;
      chapter_count: number;
      chapters: Array<{
        id: string;
        chapter_number: number;
        title: string;
        summary: string;
        sub_index: number;
        status: string;
        word_count: number;
      }>;
      expansion_plans: Array<{
        sub_index: number;
        title: string;
        plot_summary: string;
        key_events: string[];
        character_focus: string[];
        emotional_tone: string;
        narrative_goal: string;
        conflict_type: string;
        estimated_words: number;
        scenes?: Array<{
          location: string;
          characters: string[];
          purpose: string;
        }> | null;
      }> | null;
    }>(`/outlines/${outlineId}/chapters`),

  // 单个大纲展开为多章
  expandOutline: (outlineId: string, data: OutlineExpansionRequest) =>
    api.post<unknown, OutlineExpansionResponse>(`/outlines/${outlineId}/expand`, data),

  // 根据已有规划创建章节（避免重复AI调用）
  createChaptersFromPlans: (outlineId: string, chapterPlans: ChapterPlanItem[]) =>
    api.post<unknown, {
      outline_id: string;
      outline_title: string;
      chapters_created: number;
      created_chapters: Array<{
        id: string;
        chapter_number: number;
        title: string;
        summary: string;
        outline_id: string;
        sub_index: number;
        status: string;
      }>;
    }>(`/outlines/${outlineId}/create-chapters-from-plans`, { chapter_plans: chapterPlans }),

  // 批量展开大纲
  batchExpandOutlines: (data: BatchOutlineExpansionRequest) =>
    api.post<unknown, BatchOutlineExpansionResponse>('/outlines/batch-expand', data),
};

export const characterApi = {
  getCharacters: (projectId: string) =>
    api.get<unknown, { total: number; items: Character[] }>(`/characters/project/${projectId}`).then(res => res.items),

  getCharacter: (id: string) => api.get<unknown, Character>(`/characters/${id}`),

  createCharacter: (data: {
    project_id: string;
    name: string;
    age?: string;
    gender?: string;
    is_organization?: boolean;
    role_type?: string;
    personality?: string;
    background?: string;
    appearance?: string;
    relationships?: string;
    organization_type?: string;
    organization_purpose?: string;
    organization_members?: string;
    traits?: string;
    avatar_url?: string;
    power_level?: number;
    location?: string;
    motto?: string;
    color?: string;
  }) =>
    api.post<unknown, Character>('/characters', data),

  updateCharacter: (id: string, data: CharacterUpdate) =>
    api.put<unknown, Character>(`/characters/${id}`, data),

  deleteCharacter: (id: string) => api.delete(`/characters/${id}`),

  generateCharacter: (data: GenerateCharacterRequest) =>
    api.post<unknown, Character>('/characters/generate', data),

  // 导出角色/组织
  exportCharacters: async (characterIds: string[]) => {
    const response = await axios.post(
      '/api/characters/export',
      { character_ids: characterIds },
      {
        responseType: 'blob',
        headers: {
          'Content-Type': 'application/json',
        },
      }
    );

    // 从响应头获取文件名
    const contentDisposition = response.headers['content-disposition'];
    let filename = 'characters_export.json';
    if (contentDisposition) {
      const matches = /filename=(.+)/.exec(contentDisposition);
      if (matches && matches[1]) {
        filename = matches[1];
      }
    }

    // 创建下载链接
    const url = window.URL.createObjectURL(new Blob([response.data]));
    const link = document.createElement('a');
    link.href = url;
    link.setAttribute('download', filename);
    document.body.appendChild(link);
    link.click();
    link.remove();
    window.URL.revokeObjectURL(url);
  },

  // 验证导入文件
  validateImportCharacters: (file: File) => {
    const formData = new FormData();
    formData.append('file', file);
    return api.post<unknown, {
      valid: boolean;
      version: string;
      statistics: { characters: number; organizations: number };
      errors: string[];
      warnings: string[];
    }>('/characters/validate-import', formData, {
      headers: { 'Content-Type': 'multipart/form-data' },
    });
  },

  // 导入角色/组织
  importCharacters: (projectId: string, file: File) => {
    const formData = new FormData();
    formData.append('file', file);
    return api.post<unknown, {
      success: boolean;
      message: string;
      statistics: {
        total: number;
        imported: number;
        skipped: number;
        errors: number;
      };
      details: {
        imported_characters: string[];
        imported_organizations: string[];
        skipped: string[];
        errors: string[];
      };
      warnings: string[];
    }>(`/characters/import?project_id=${projectId}`, formData, {
      headers: { 'Content-Type': 'multipart/form-data' },
    });
  },
};

export const chapterApi = {
  upsertChapterAnalysisTaskToStore: (
    task: import('../types').AnalysisTask,
    projectId?: string,
    messageOverride?: string
  ) => {
    if (!task?.has_task || !task.task_id || task.status === 'none') return;
    const status = task.status === 'running' || task.status === 'completed' || task.status === 'failed'
      ? task.status
      : 'pending';
    const messageText =
      messageOverride ??
      (status === 'completed'
        ? '章节分析已完成'
        : status === 'failed'
          ? (task.error_message || '章节分析失败')
          : `章节分析进行中 (${task.progress ?? 0}%)`);

    useBackgroundTaskStore.getState().upsertTask({
      task_id: task.task_id,
      task_type: 'chapter_analysis',
      project_id: projectId,
      status,
      progress: task.progress ?? 0,
      message: messageText,
      error: task.error_message ?? null,
      stage_code: 'analysis',
      execution_mode: 'interactive',
      checkpoint: { chapter_id: task.chapter_id },
      created_at: task.created_at ?? undefined,
      updated_at: new Date().toISOString(),
      completed_at: task.completed_at ?? undefined,
    });
  },

  getChapters: (projectId: string) =>
    api.get<unknown, { total: number; items: Chapter[] }>(`/chapters/project/${projectId}`).then(res => res.items),

  getChapter: (id: string) => api.get<unknown, Chapter>(`/chapters/${id}`),

  createChapter: (data: ChapterCreate) => api.post<unknown, Chapter>('/chapters', data),

  updateChapter: (id: string, data: ChapterUpdate) =>
    api.put<unknown, Chapter>(`/chapters/${id}`, data),

  deleteChapter: (id: string) => api.delete(`/chapters/${id}`),

  checkCanGenerate: (chapterId: string) =>
    api.get<unknown, import('../types').ChapterCanGenerateResponse>(`/chapters/${chapterId}/can-generate`),

  getChapterQualityMetrics: (chapterId: string) =>
    api.get<unknown, import('../types').ChapterQualityMetricsResponse>(`/chapters/${chapterId}/quality-metrics`),

  getChapterAnalysis: (chapterId: string, includeFullDraft = false) =>
    api.get<unknown, import('../types').ChapterAnalysisResponse>(
      `/chapters/${chapterId}/analysis`,
      { params: { include_full_draft: includeFullDraft } }
    ),

  getChapterAnalysisStatus: async (chapterId: string, projectId?: string) => {
    const status = await api.get<unknown, import('../types').AnalysisTask>(
      `/chapters/${chapterId}/analysis/status`,
      silentRequestConfig()
    );
    status.error_message = formatChapterAnalysisError(status.error_code, status.error_message);
    chapterApi.upsertChapterAnalysisTaskToStore(status, projectId);
    return status;
  },

  triggerChapterAnalysis: async (chapterId: string, projectId?: string) => {
    const created = await api.post<unknown, import('../types').TriggerAnalysisResponse>(`/chapters/${chapterId}/analyze`);
    useBackgroundTaskStore.getState().upsertTask({
      task_id: created.task_id,
      task_type: 'chapter_analysis',
      project_id: projectId,
      status: 'pending',
      progress: 0,
      message: created.message || '章节分析任务已创建',
      stage_code: 'analysis',
      execution_mode: 'interactive',
      checkpoint: { chapter_id: chapterId },
      created_at: new Date().toISOString(),
      updated_at: new Date().toISOString(),
    });
    return created;
  },

  getAutoRevisionDraft: (chapterId: string, historyId?: string) =>
    api.get<unknown, import('../types').ChapterAutoRevisionDraftResponse>(
      `/chapters/${chapterId}/analysis/auto-revision-draft`,
      { params: { history_id: historyId } }
    ),

  applyAutoRevisionDraft: (
    chapterId: string,
    data: import('../types').ApplyAutoRevisionDraftRequest = {}
  ) =>
    api.post<unknown, import('../types').ApplyAutoRevisionDraftResponse>(
      `/chapters/${chapterId}/analysis/auto-revision-draft/apply`,
      data
    ),

  // 章节重新生成相关
  getRegenerationTasks: (chapterId: string, limit?: number) =>
    api.get<unknown, {
      chapter_id: string;
      total: number;
      tasks: Array<{
        task_id: string;
        status: string;
        version_number: number | null;
        version_note: string | null;
        original_word_count: number | null;
        regenerated_word_count: number | null;
        created_at: string | null;
        completed_at: string | null;
      }>;
    }>(`/chapters/${chapterId}/regeneration/tasks`, { params: { limit } }),

  // 局部重写相关
  partialRegenerateStream: (
    chapterId: string,
    data: {
      selected_text: string;
      start_position: number;
      end_position: number;
      user_instructions: string;
      context_chars?: number;
      style_id?: number;
      length_mode?: 'similar' | 'expand' | 'condense' | 'custom';
      target_word_count?: number;
    },
    options?: SSEClientOptions
  ) => ssePost<{
    new_text: string;
    word_count: number;
    original_word_count: number;
    start_position: number;
    end_position: number;
  }>(
    `/api/chapters/${chapterId}/partial-regenerate-stream`,
    data,
    options
  ),

  applyPartialRegenerate: (chapterId: string, data: {
    new_text: string;
    start_position: number;
    end_position: number;
  }) =>
    api.post<unknown, {
      success: boolean;
      chapter_id: string;
      word_count: number;
      old_word_count: number;
      message: string;
    }>(`/chapters/${chapterId}/apply-partial-regenerate`, data),
};

type BatchTaskRuntimeStatus = 'pending' | 'running' | 'completed' | 'failed' | 'cancelled';

export interface ChapterBatchGenerateResponse {
  batch_id: string;
  message: string;
  chapters_to_generate: Array<{ id: string; chapter_number: number; title: string }>;
  estimated_time_minutes: number;
}

export interface ChapterBatchGenerateStatusResponse {
  batch_id: string;
  status: string;
  stage_code?: string | null;
  execution_mode?: 'interactive' | 'auto' | null;
  total: number;
  completed: number;
  current_chapter_id?: string | null;
  current_chapter_number?: number | null;
  current_retry_count?: number | null;
  max_retries?: number | null;
  checkpoint?: Record<string, unknown> | null;
  failed_chapters?: Array<Record<string, unknown>>;
  created_at?: string | null;
  started_at?: string | null;
  completed_at?: string | null;
  error_message?: string | null;
  latest_quality_metrics?: Record<string, unknown> | null;
  quality_metrics_summary?: Record<string, unknown> | null;
}

export interface ChapterBatchActiveTask {
  batch_id: string;
  status: string;
  stage_code?: string | null;
  execution_mode?: 'interactive' | 'auto' | null;
  total: number;
  completed: number;
  current_chapter_id?: string | null;
  current_chapter_number?: number | null;
  checkpoint?: Record<string, unknown> | null;
  latest_quality_metrics?: Record<string, unknown> | null;
  quality_metrics_summary?: Record<string, unknown> | null;
  created_at?: string | null;
  started_at?: string | null;
}

export interface ChapterBatchActiveResponse {
  has_active_task: boolean;
  task: ChapterBatchActiveTask | null;
}

export interface ChapterBatchCancelResponse {
  message: string;
  batch_id: string;
  completed_chapters: number;
  total_chapters: number;
}

export interface ChapterBatchResumeResponse {
  message: string;
  batch_id: string;
  project_id?: string;
  task_type?: ChapterGenerationTaskType;
  status: string;
  stage_code?: string | null;
  execution_mode?: 'interactive' | 'auto' | null;
  checkpoint?: Record<string, unknown> | null;
  resumed_from_batch_id?: string;
  total_chapters: number;
  completed_chapters: number;
  created_at?: string | null;
}

export interface ChapterSingleGenerateResponse {
  task_id: string;
  chapter_id: string;
  status: string;
  message: string;
  estimated_time_minutes?: number;
}

type ChapterGenerationTaskType = 'chapters_batch_generate' | 'chapter_single_generate';

const normalizeBatchTaskStatus = (status: string): BatchTaskRuntimeStatus => {
  if (status === 'running' || status === 'completed' || status === 'failed' || status === 'cancelled') {
    return status;
  }
  return 'pending';
};

const buildChapterGenerateTaskMessage = (
  taskType: ChapterGenerationTaskType,
  status: BatchTaskRuntimeStatus,
  total: number,
  completed: number,
  currentChapterNumber?: number | null,
  errorMessage?: string | null
) => {
  const taskName = taskType === 'chapter_single_generate' ? '单章生成' : '批量生成';
  if (status === 'failed') return errorMessage || `${taskName}失败`;
  if (status === 'cancelled') return `${taskName}已取消`;
  if (status === 'completed') return `${taskName}完成 (${completed}/${total})`;
  if (currentChapterNumber) return `${taskName}中：第 ${currentChapterNumber} 章 (${completed}/${total})`;
  if (status === 'running') return `${taskName}中 (${completed}/${total})`;
  return `${taskName}排队中 (${completed}/${total})`;
};

const upsertChapterTaskToStore = (data: {
  taskType: ChapterGenerationTaskType;
  taskId: string;
  status: string;
  total: number;
  completed: number;
  projectId?: string;
  currentChapterNumber?: number | null;
  errorMessage?: string | null;
  stageCode?: string | null;
  executionMode?: 'interactive' | 'auto' | null;
  checkpoint?: Record<string, unknown> | null;
  createdAt?: string | null;
  completedAt?: string | null;
}) => {
  const normalizedStatus = normalizeBatchTaskStatus(data.status);
  const progress = data.total > 0 ? Math.round((data.completed / data.total) * 100) : 0;
  const now = new Date().toISOString();
  useBackgroundTaskStore.getState().upsertTask({
    task_id: data.taskId,
    task_type: data.taskType,
    project_id: data.projectId,
    status: normalizedStatus,
    progress,
    message: buildChapterGenerateTaskMessage(
      data.taskType,
      normalizedStatus,
      data.total,
      data.completed,
      data.currentChapterNumber,
      data.errorMessage
    ),
    error: data.errorMessage ?? null,
    stage_code: data.stageCode ?? undefined,
    execution_mode: data.executionMode ?? undefined,
    checkpoint: data.checkpoint ?? undefined,
    created_at: data.createdAt ?? now,
    updated_at: now,
    completed_at: data.completedAt ?? null,
  });
};

export const chapterBatchTaskApi = {
  createBatchGenerateTask: async (
    projectId: string,
    payload: {
      start_chapter_number: number;
      count: number;
      enable_analysis: boolean;
      style_id: number;
      target_word_count: number;
      model?: string;
    }
  ) => {
    const created = await api.post<unknown, ChapterBatchGenerateResponse>(
      `/chapters/project/${projectId}/batch-generate`,
      payload
    );
    useBackgroundTaskStore.getState().upsertTask({
      task_id: created.batch_id,
      task_type: 'chapters_batch_generate',
      project_id: projectId,
      status: 'pending',
      progress: 0,
      stage_code: '6.writing',
      execution_mode: 'interactive',
      message: created.message || '批量生成任务已创建',
    });
    return created;
  },

  getBatchGenerateStatus: async (batchId: string, projectId?: string) => {
    const status = await api.get<unknown, ChapterBatchGenerateStatusResponse>(
      `/chapters/batch-generate/${batchId}/status`,
      silentRequestConfig()
    );
    upsertChapterTaskToStore({
      taskType: 'chapters_batch_generate',
      taskId: status.batch_id,
      status: status.status,
      total: status.total,
      completed: status.completed,
      projectId,
      currentChapterNumber: status.current_chapter_number,
      errorMessage: status.error_message,
      stageCode: status.stage_code ?? '6.writing',
      executionMode: status.execution_mode ?? 'interactive',
      checkpoint: status.checkpoint ?? undefined,
      createdAt: status.created_at,
      completedAt: status.completed_at,
    });
    return status;
  },

  getActiveBatchGenerateTask: async (projectId: string) => {
    const active = await api.get<unknown, ChapterBatchActiveResponse>(
      `/chapters/project/${projectId}/batch-generate/active`
    );
    if (active.has_active_task && active.task) {
      upsertChapterTaskToStore({
        taskType: 'chapters_batch_generate',
        taskId: active.task.batch_id,
        status: active.task.status,
        total: active.task.total,
        completed: active.task.completed,
        projectId,
        currentChapterNumber: active.task.current_chapter_number,
        stageCode: active.task.stage_code ?? '6.writing',
        executionMode: active.task.execution_mode ?? 'interactive',
        checkpoint: active.task.checkpoint ?? undefined,
        createdAt: active.task.created_at,
      });
    }
    return active;
  },

  listActiveTasks: async (limit = 20) => {
    const response = await api.get<
      unknown,
      {
        total: number;
        items: Array<{
          task_type: ChapterGenerationTaskType;
          stage_code?: string | null;
          execution_mode?: 'interactive' | 'auto' | null;
          project_id: string;
          batch_id: string;
          status: string;
          total: number;
          completed: number;
          current_chapter_number?: number | null;
          checkpoint?: Record<string, unknown> | null;
          error_message?: string | null;
          created_at?: string | null;
          completed_at?: string | null;
        }>;
      }
    >('/chapters/batch-generate/active-tasks', {
      params: { limit },
    });

    for (const task of response.items || []) {
      upsertChapterTaskToStore({
        taskType: task.task_type,
        taskId: task.batch_id,
        status: task.status,
        total: task.total,
        completed: task.completed,
        projectId: task.project_id,
        currentChapterNumber: task.current_chapter_number ?? null,
        errorMessage: task.error_message ?? null,
        stageCode: task.stage_code ?? '6.writing',
        executionMode: task.execution_mode ?? 'interactive',
        checkpoint: task.checkpoint ?? undefined,
        createdAt: task.created_at,
        completedAt: task.completed_at,
      });
    }
    return response;
  },

  cancelBatchGenerateTask: async (batchId: string, projectId?: string) => {
    const cancelled = await api.post<unknown, ChapterBatchCancelResponse>(
      `/chapters/batch-generate/${batchId}/cancel`
    );
    useBackgroundTaskStore.getState().upsertTask({
      task_id: cancelled.batch_id,
      task_type: 'chapters_batch_generate',
      project_id: projectId,
      status: 'cancelled',
      progress: 100,
      message: cancelled.message || '批量生成任务已取消',
    });
    return cancelled;
  },

  resumeBatchGenerateTask: async (batchId: string, projectId?: string) => {
    const resumed = await api.post<unknown, ChapterBatchResumeResponse>(
      `/chapters/batch-generate/${batchId}/resume`
    );
    upsertChapterTaskToStore({
      taskType: resumed.task_type ?? 'chapters_batch_generate',
      taskId: resumed.batch_id,
      status: resumed.status,
      total: resumed.total_chapters,
      completed: resumed.completed_chapters,
      projectId: resumed.project_id ?? projectId,
      stageCode: resumed.stage_code ?? '6.writing.loading',
      executionMode: resumed.execution_mode ?? 'interactive',
      checkpoint: resumed.checkpoint ?? undefined,
      createdAt: resumed.created_at,
    });
    return resumed;
  },
};

export const chapterSingleTaskApi = {
  createSingleGenerateTask: async (
    chapterId: string,
    payload: {
      style_id?: number;
      target_word_count?: number;
      model?: string;
      narrative_perspective?: string;
    },
    projectId?: string
  ) => {
    const created = await api.post<unknown, ChapterSingleGenerateResponse>(
      `/chapters/${chapterId}/generate-background`,
      payload
    );
    useBackgroundTaskStore.getState().upsertTask({
      task_id: created.task_id,
      task_type: 'chapter_single_generate',
      project_id: projectId,
      status: normalizeBatchTaskStatus(created.status),
      progress: created.status === 'pending' ? 0 : 10,
      stage_code: '6.writing',
      execution_mode: 'interactive',
      message: created.message || '单章后台任务已创建',
    });
    return created;
  },

  getSingleGenerateTaskStatus: async (taskId: string, projectId?: string) => {
    const status = await api.get<unknown, ChapterBatchGenerateStatusResponse>(
      `/chapters/batch-generate/${taskId}/status`,
      silentRequestConfig()
    );
    upsertChapterTaskToStore({
      taskType: 'chapter_single_generate',
      taskId: status.batch_id,
      status: status.status,
      total: status.total,
      completed: status.completed,
      projectId,
      currentChapterNumber: status.current_chapter_number,
      errorMessage: status.error_message,
      stageCode: status.stage_code ?? '6.writing',
      executionMode: status.execution_mode ?? 'interactive',
      checkpoint: status.checkpoint ?? undefined,
      createdAt: status.created_at,
      completedAt: status.completed_at,
    });
    return status;
  },

  cancelSingleGenerateTask: async (taskId: string, projectId?: string) => {
    const cancelled = await api.post<unknown, ChapterBatchCancelResponse>(
      `/chapters/batch-generate/${taskId}/cancel`
    );
    useBackgroundTaskStore.getState().upsertTask({
      task_id: cancelled.batch_id,
      task_type: 'chapter_single_generate',
      project_id: projectId,
      status: 'cancelled',
      progress: 100,
      message: cancelled.message || '单章生成任务已取消',
    });
    return cancelled;
  },

  resumeSingleGenerateTask: async (taskId: string, projectId?: string) => {
    const resumed = await api.post<unknown, ChapterBatchResumeResponse>(
      `/chapters/batch-generate/${taskId}/resume`
    );
    upsertChapterTaskToStore({
      taskType: 'chapter_single_generate',
      taskId: resumed.batch_id,
      status: resumed.status,
      total: resumed.total_chapters,
      completed: resumed.completed_chapters,
      projectId: resumed.project_id ?? projectId,
      stageCode: resumed.stage_code ?? '6.writing.loading',
      executionMode: resumed.execution_mode ?? 'interactive',
      checkpoint: resumed.checkpoint ?? undefined,
      createdAt: resumed.created_at,
    });
    return resumed;
  },
};

export const writingStyleApi = {
  // 获取预设风格列表
  getPresetStyles: () =>
    api.get<unknown, PresetStyle[]>('/writing-styles/presets/list'),

  // 获取用户的所有风格（新接口）
  getUserStyles: () =>
    api.get<unknown, WritingStyleListResponse>('/writing-styles/user'),

  // 获取项目的所有风格（保留向后兼容）
  getProjectStyles: (projectId: string) =>
    api.get<unknown, WritingStyleListResponse>(`/writing-styles/project/${projectId}`),

  // 创建新风格（基于预设或自定义）
  createStyle: (data: WritingStyleCreate) =>
    api.post<unknown, WritingStyle>('/writing-styles', data),

  // 更新风格
  updateStyle: (styleId: number, data: WritingStyleUpdate) =>
    api.put<unknown, WritingStyle>(`/writing-styles/${styleId}`, data),

  // 删除风格
  deleteStyle: (styleId: number) =>
    api.delete<unknown, { message: string }>(`/writing-styles/${styleId}`),

  // 设置默认风格
  setDefaultStyle: (styleId: number, projectId: string) =>
    api.post<unknown, WritingStyle>(`/writing-styles/${styleId}/set-default`, { project_id: projectId }),

  // 为项目初始化默认风格（如果没有任何风格）
  initializeDefaultStyles: (projectId: string) =>
    api.post<unknown, WritingStyleListResponse>(`/writing-styles/project/${projectId}/initialize`, {}),
};

export const promptWorkshopApi = {
  // 检查服务状态
  getStatus: () =>
    api.get<unknown, { mode: string; instance_id: string; cloud_url?: string; cloud_connected?: boolean }>('/prompt-workshop/status'),

  // 获取工坊提示词列表
  getItems: (params?: {
    category?: string;
    search?: string;
    tags?: string;
    sort?: 'newest' | 'popular' | 'downloads';
    page?: number;
    limit?: number;
  }) => api.get<unknown, PromptWorkshopListResponse>('/prompt-workshop/items', { params }),

  // 获取单个提示词
  getItem: (itemId: string) =>
    api.get<unknown, { success: boolean; data: PromptWorkshopItem }>(`/prompt-workshop/items/${itemId}`),

  // 导入到本地
  importItem: (itemId: string, customName?: string) =>
    api.post<unknown, { success: boolean; message: string; writing_style: WritingStyle }>(
      `/prompt-workshop/items/${itemId}/import`,
      { custom_name: customName }
    ),

  // 点赞
  toggleLike: (itemId: string) =>
    api.post<unknown, { success: boolean; liked: boolean; like_count: number }>(
      `/prompt-workshop/items/${itemId}/like`
    ),

  // 提交提示词
  submit: (data: PromptSubmissionCreate) =>
    api.post<unknown, { success: boolean; message: string; submission: PromptSubmission }>('/prompt-workshop/submit', data),

  // 我的提交
  getMySubmissions: (status?: string) =>
    api.get<unknown, { success: boolean; data: { total: number; items: PromptSubmission[] } }>(
      '/prompt-workshop/my-submissions',
      { params: { status } }
    ),

  // 撤回提交（pending状态）
  withdrawSubmission: (submissionId: string) =>
    api.delete<unknown, { success: boolean; message: string }>(`/prompt-workshop/submissions/${submissionId}`),

  // 删除提交记录（所有状态，需要 force=true）
  deleteSubmission: (submissionId: string) =>
    api.delete<unknown, { success: boolean; message: string }>(`/prompt-workshop/submissions/${submissionId}`, {
      params: { force: true }
    }),

  // ========== 管理员 API（仅服务端模式可用） ==========
  
  // 获取待审核列表
  adminGetSubmissions: (params?: { status?: string; source?: string; page?: number; limit?: number }) =>
    api.get<unknown, {
      success: boolean;
      data: {
        total: number;
        pending_count: number;
        page: number;
        limit: number;
        items: PromptSubmission[];
      };
    }>('/prompt-workshop/admin/submissions', { params }),

  // 审核提交
  adminReviewSubmission: (submissionId: string, data: { action: 'approve' | 'reject'; review_note?: string; category?: string; tags?: string[] }) =>
    api.post<unknown, { success: boolean; message: string; workshop_item?: PromptWorkshopItem; submission?: PromptSubmission }>(
      `/prompt-workshop/admin/submissions/${submissionId}/review`,
      data
    ),

  // 添加官方提示词
  adminCreateItem: (data: { name: string; description?: string; prompt_content: string; category: string; tags?: string[] }) =>
    api.post<unknown, { success: boolean; item: PromptWorkshopItem }>('/prompt-workshop/admin/items', data),

  // 编辑提示词
  adminUpdateItem: (itemId: string, data: { name?: string; description?: string; prompt_content?: string; category?: string; tags?: string[]; status?: string }) =>
    api.put<unknown, { success: boolean; item: PromptWorkshopItem }>(`/prompt-workshop/admin/items/${itemId}`, data),

  // 删除提示词
  adminDeleteItem: (itemId: string) =>
    api.delete<unknown, { success: boolean; message: string }>(`/prompt-workshop/admin/items/${itemId}`),

  // 获取统计数据
  adminGetStats: () =>
    api.get<unknown, {
      success: boolean;
      data: {
        total_items: number;
        total_official: number;
        total_pending: number;
        total_downloads: number;
        total_likes: number;
      };
    }>('/prompt-workshop/admin/stats'),
};

export const polishApi = {
  polishText: (data: PolishTextRequest) =>
    api.post<unknown, { polished_text: string }>('/polish', data),

  polishBatch: (texts: string[]) =>
    api.post<unknown, { polished_texts: string[] }>('/polish/batch', { texts }),
};

export interface BackgroundTaskStatus {
  task_id: string;
  task_type:
    | 'careers_generate_system'
    | 'character_generate'
    | 'organization_generate'
    | 'world_regenerate'
    | 'outline_generate'
    | 'outline_expand'
    | 'outline_batch_expand'
    | 'wizard_world_building'
    | 'wizard_career_system'
    | 'wizard_characters'
    | 'wizard_outline';
  project_id: string;
  status: 'pending' | 'running' | 'completed' | 'failed' | 'cancelled';
  progress: number;
  message: string;
  result?: Record<string, unknown> | null;
  error?: string | null;
  stage_code?: string | null;
  execution_mode?: 'interactive' | 'auto' | null;
  workflow_scope?: string | null;
  checkpoint?: Record<string, unknown> | null;
  created_at?: string | null;
  updated_at?: string | null;
  started_at?: string | null;
  completed_at?: string | null;
}

export interface BackgroundTaskListResponse {
  total: number;
  items: BackgroundTaskStatus[];
}

export const backgroundTaskApi = {
  createTask: async (data: {
    task_type: BackgroundTaskStatus['task_type'];
    project_id?: string;
    payload?: Record<string, unknown>;
    stage_code?: string;
    execution_mode?: 'interactive' | 'auto';
    workflow_scope?: string;
    checkpoint?: Record<string, unknown>;
  }) => {
    const created = await api.post<unknown, BackgroundTaskStatus>('/background-tasks', data);
    useBackgroundTaskStore.getState().upsertTask(created);
    return created;
  },

  getTaskStatus: async (taskId: string) => {
    const status = await api.get<unknown, BackgroundTaskStatus>(
      `/background-tasks/${taskId}`,
      silentRequestConfig()
    );
    useBackgroundTaskStore.getState().upsertTask(status);
    return status;
  },

  listTasks: async (params?: {
    project_id?: string;
    statuses?: string;
    active_only?: boolean;
    limit?: number;
  }) => {
    const data = await api.get<unknown, BackgroundTaskListResponse>(
      '/background-tasks',
      silentRequestConfig({ params })
    );
    for (const item of data.items || []) {
      useBackgroundTaskStore.getState().upsertTask(item);
    }
    return data;
  },

  updateWorkflowState: async (
    taskId: string,
    payload: {
      stage_code?: string;
      execution_mode?: 'interactive' | 'auto';
      workflow_scope?: string;
      checkpoint?: Record<string, unknown>;
      message?: string;
      progress?: number;
    }
  ) => {
    const status = await api.patch<unknown, BackgroundTaskStatus>(`/background-tasks/${taskId}/workflow-state`, payload);
    useBackgroundTaskStore.getState().upsertTask(status);
    return status;
  },

  cancelTask: async (taskId: string) => {
    const cancelled = await api.post<unknown, BackgroundTaskStatus>(`/background-tasks/${taskId}/cancel`);
    useBackgroundTaskStore.getState().upsertTask(cancelled);
    return cancelled;
  },
};

export const inspirationApi = {
  // 生成选项建议
  generateOptions: (data: {
    step: 'title' | 'description' | 'theme' | 'genre';
    context: {
      title?: string;
      description?: string;
      theme?: string;
    };
  }) =>
    api.post<unknown, {
      prompt?: string;
      options: string[];
      error?: string;
    }>('/inspiration/generate-options', data),

  // 基于用户反馈重新生成选项（新增）
  refineOptions: (data: {
    step: 'title' | 'description' | 'theme' | 'genre';
    context: {
      initial_idea?: string;
      title?: string;
      description?: string;
      theme?: string;
    };
    feedback: string;
    previous_options?: string[];
  }) =>
    api.post<unknown, {
      prompt?: string;
      options: string[];
      error?: string;
    }>('/inspiration/refine-options', data),

  // 智能补全缺失信息
  quickGenerate: (data: {
    title?: string;
    description?: string;
    theme?: string;
    genre?: string | string[];
  }) =>
    api.post<unknown, {
      title: string;
      description: string;
      theme: string;
      genre: string[];
      narrative_perspective: string;
    }>('/inspiration/quick-generate', data),
};

export default api;

const runBackgroundTaskWithPolling = async <T>(
  taskType: BackgroundTaskStatus['task_type'],
  projectId: string | undefined,
  payload: Record<string, unknown>,
  options?: SSEClientOptions
): Promise<T> => {
  const createdTask = await backgroundTaskApi.createTask({
    task_type: taskType,
    project_id: projectId,
    payload,
  });

  options?.onTaskCreated?.(createdTask.task_id);
  options?.onProgress?.('后台任务已创建', 0, 'processing');

  return new Promise<T>((resolve, reject) => {
    let timer: number | null = null;

    const stopPolling = () => {
      if (timer) {
        window.clearInterval(timer);
        timer = null;
      }
    };

    const poll = async () => {
      try {
        const task = await backgroundTaskApi.getTaskStatus(createdTask.task_id);
        options?.onProgress?.(task.message || '', task.progress || 0, task.status);

        if (task.status === 'completed') {
          stopPolling();
          if (task.result !== undefined && task.result !== null) {
            options?.onResult?.(task.result);
          }
          options?.onComplete?.();
          resolve((task.result as T) ?? (true as T));
          return;
        }

        if (task.status === 'failed') {
          stopPolling();
          const errorMsg = task.error || task.message || '后台任务执行失败';
          options?.onError?.(errorMsg);
          reject(new Error(errorMsg));
          return;
        }

        if (task.status === 'cancelled') {
          stopPolling();
          const errorMsg = task.message || '后台任务已取消';
          options?.onCancelled?.(errorMsg);
          const cancelledError = new Error(errorMsg) as Error & { code?: string };
          cancelledError.name = 'TaskCancelledError';
          cancelledError.code = 'TASK_CANCELLED';
          reject(cancelledError);
        }
      } catch (error) {
        stopPolling();
        const errorMsg = error instanceof Error ? error.message : '轮询后台任务失败';
        options?.onError?.(errorMsg);
        reject(error);
      }
    };

    void poll();
    timer = window.setInterval(() => {
      void poll();
    }, 1500);
  });
};


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
      outline_mode?: 'one-to-one' | 'one-to-many';  // 添加大纲模式参数
      provider?: string;
      model?: string;
    },
    options?: SSEClientOptions
  ) => runBackgroundTaskWithPolling<WorldBuildingResponse>(
    'wizard_world_building',
    undefined,
    data as Record<string, unknown>,
    options
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
    },
    options?: SSEClientOptions
  ) => runBackgroundTaskWithPolling<GenerateCharactersResponse>(
    'wizard_characters',
    data.project_id,
    data as Record<string, unknown>,
    options
  ),

  generateCareerSystemStream: (
    data: {
      project_id: string;
      provider?: string;
      model?: string;
    },
    options?: SSEClientOptions
  ) => runBackgroundTaskWithPolling<{
    project_id: string;
    main_careers_count: number;
    sub_careers_count: number;
    main_careers: string[];
    sub_careers: string[];
  }>(
    'wizard_career_system',
    data.project_id,
    data as Record<string, unknown>,
    options
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
    },
    options?: SSEClientOptions
  ) => runBackgroundTaskWithPolling<GenerateOutlineResponse>(
    'wizard_outline',
    data.project_id,
    data as Record<string, unknown>,
    options
  ),

  updateWorldBuildingStream: (
    projectId: string,
    data: {
      time_period?: string;
      location?: string;
      atmosphere?: string;
      rules?: string;
    },
    options?: SSEClientOptions
  ) => ssePost<WorldBuildingResponse>(
    `/api/wizard-stream/world-building/${projectId}`,
    data,
    options
  ),

  regenerateWorldBuildingStream: (
    projectId: string,
    data?: {
      provider?: string;
      model?: string;
    },
    options?: SSEClientOptions
  ) => ssePost<WorldBuildingResponse>(
    `/api/wizard-stream/world-building/${projectId}/regenerate`,
    data || {},
    options
  ),

  cleanupWizardDataStream: (
    projectId: string,
    options?: SSEClientOptions
  ) => ssePost<{ message: string; deleted: { characters: number; outlines: number; chapters: number } }>(
    `/api/wizard-stream/cleanup/${projectId}`,
    {},
    options
  ),
};

export const mcpPluginApi = {
  // 获取所有插件
  getPlugins: () =>
    api.get<unknown, MCPPlugin[]>('/mcp/plugins'),

  // 获取单个插件
  getPlugin: (id: string) =>
    api.get<unknown, MCPPlugin>(`/mcp/plugins/${id}`),

  // 创建插件
  createPlugin: (data: MCPPluginCreate) =>
    api.post<unknown, MCPPlugin>('/mcp/plugins', data),

  // 简化创建插件（通过标准MCP配置JSON）
  createPluginSimple: (data: MCPPluginSimpleCreate) =>
    api.post<unknown, MCPPlugin>('/mcp/plugins/simple', data),

  // 更新插件
  updatePlugin: (id: string, data: MCPPluginUpdate) =>
    api.put<unknown, MCPPlugin>(`/mcp/plugins/${id}`, data),

  // 删除插件
  deletePlugin: (id: string) =>
    api.delete<unknown, { message: string }>(`/mcp/plugins/${id}`),

  // 启用/禁用插件
  togglePlugin: (id: string, enabled: boolean) =>
    api.post<unknown, MCPPlugin>(`/mcp/plugins/${id}/toggle`, null, { params: { enabled } }),

  // 测试插件连接
  testPlugin: (id: string) =>
    api.post<unknown, MCPTestResult>(`/mcp/plugins/${id}/test`),

  // 获取插件工具列表
  getPluginTools: (id: string) =>
    api.get<unknown, { tools: MCPTool[] }>(`/mcp/plugins/${id}/tools`),

  // 调用工具
  callTool: (data: MCPToolCallRequest) =>
    api.post<unknown, MCPToolCallResponse>('/mcp/call', data),
};

// 管理员API
export const adminApi = {
  // 获取用户列表
  getUsers: () =>
    api.get<unknown, { total: number; users: User[] }>('/admin/users'),

  // 添加用户
  createUser: (data: {
    username: string;
    display_name: string;
    password?: string;
    avatar_url?: string;
    trust_level?: number;
    is_admin?: boolean;
  }) =>
    api.post<unknown, {
      success: boolean;
      message: string;
      user: User;
      default_password?: string;
    }>('/admin/users', data),

  // 编辑用户
  updateUser: (userId: string, data: {
    display_name?: string;
    avatar_url?: string;
    trust_level?: number;
  }) =>
    api.put<unknown, {
      success: boolean;
      message: string;
      user: User;
    }>(`/admin/users/${userId}`, data),

  // 切换用户状态（启用/禁用）
  toggleUserStatus: (userId: string, isActive: boolean) =>
    api.post<unknown, {
      success: boolean;
      message: string;
      is_active: boolean;
    }>(`/admin/users/${userId}/toggle-status`, { is_active: isActive }),

  // 重置密码
  resetPassword: (userId: string, newPassword?: string) =>
    api.post<unknown, {
      success: boolean;
      message: string;
      new_password: string;
    }>(`/admin/users/${userId}/reset-password`, { new_password: newPassword }),

  // 删除用户
  deleteUser: (userId: string) =>
    api.delete<unknown, {
      success: boolean;
      message: string;
    }>(`/admin/users/${userId}`),
};

// 伏笔管理API
export const foreshadowApi = {
  // 获取项目伏笔列表
  getProjectForeshadows: (projectId: string, params?: {
    status?: string;
    category?: string;
    source_type?: string;
    is_long_term?: boolean;
    page?: number;
    limit?: number;
  }) =>
    api.get<unknown, import('../types').ForeshadowListResponse>(
      `/foreshadows/projects/${projectId}`,
      { params }
    ),

  // 获取伏笔统计
  getForeshadowStats: (projectId: string, currentChapter?: number) =>
    api.get<unknown, import('../types').ForeshadowStats>(
      `/foreshadows/projects/${projectId}/stats`,
      { params: { current_chapter: currentChapter } }
    ),

  // 获取章节伏笔上下文
  getChapterContext: (projectId: string, chapterNumber: number, params?: {
    include_pending?: boolean;
    include_overdue?: boolean;
    lookahead?: number;
  }) =>
    api.get<unknown, import('../types').ForeshadowContextResponse>(
      `/foreshadows/projects/${projectId}/context/${chapterNumber}`,
      { params }
    ),

  // 获取待回收伏笔
  getPendingResolveForeshadows: (projectId: string, currentChapter: number, lookahead?: number) =>
    api.get<unknown, { total: number; items: import('../types').Foreshadow[] }>(
      `/foreshadows/projects/${projectId}/pending-resolve`,
      { params: { current_chapter: currentChapter, lookahead } }
    ),

  // 获取单个伏笔
  getForeshadow: (foreshadowId: string) =>
    api.get<unknown, import('../types').Foreshadow>(`/foreshadows/${foreshadowId}`),

  // 创建伏笔
  createForeshadow: (data: import('../types').ForeshadowCreate) =>
    api.post<unknown, import('../types').Foreshadow>('/foreshadows', data),

  // 更新伏笔
  updateForeshadow: (foreshadowId: string, data: import('../types').ForeshadowUpdate) =>
    api.put<unknown, import('../types').Foreshadow>(`/foreshadows/${foreshadowId}`, data),

  // 删除伏笔
  deleteForeshadow: (foreshadowId: string) =>
    api.delete<unknown, { message: string; id: string }>(`/foreshadows/${foreshadowId}`),

  // 标记伏笔为已埋入
  plantForeshadow: (foreshadowId: string, data: import('../types').PlantForeshadowRequest) =>
    api.post<unknown, import('../types').Foreshadow>(`/foreshadows/${foreshadowId}/plant`, data),

  // 标记伏笔为已回收
  resolveForeshadow: (foreshadowId: string, data: import('../types').ResolveForeshadowRequest) =>
    api.post<unknown, import('../types').Foreshadow>(`/foreshadows/${foreshadowId}/resolve`, data),

  // 标记伏笔为已废弃
  abandonForeshadow: (foreshadowId: string, reason?: string) =>
    api.post<unknown, import('../types').Foreshadow>(
      `/foreshadows/${foreshadowId}/abandon`,
      null,
      { params: { reason } }
    ),

  // 从分析结果同步伏笔
  syncFromAnalysis: (projectId: string, data: import('../types').SyncFromAnalysisRequest) =>
    api.post<unknown, import('../types').SyncFromAnalysisResponse>(
      `/foreshadows/projects/${projectId}/sync-from-analysis`,
      data
    ),
};
