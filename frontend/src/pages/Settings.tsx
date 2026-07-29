import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import {
  Alert,
  Button,
  Card,
  Col,
  Empty,
  Form,
  Input,
  InputNumber,
  List,
  Modal,
  Popconfirm,
  Row,
  Select,
  Space,
  Switch,
  Tabs,
  Tag,
  Typography,
  message,
  theme,
} from 'antd';
import {
  ApiOutlined,
  CheckCircleOutlined,
  CopyOutlined,
  DeleteOutlined,
  EditOutlined,
  ExperimentOutlined,
  PlayCircleOutlined,
  PlusOutlined,
  ReloadOutlined,
  SaveOutlined,
  ThunderboltOutlined,
} from '@ant-design/icons';
import { settingsApi } from '../services/modularApi';
import InlineDeferredPanel from '../components/InlineDeferredPanel';
import { designDisplayFont } from '../theme/themeConfig';
import type {
  APIKeyPreset,
  APIKeyPresetConfig,
  PresetCreateRequest,
  PresetUpdateRequest,
  Settings,
  SettingsUpdate,
} from '../types';
import { isPlaceholderApiKey } from '../utils/apiKey';

const { Text, Paragraph, Title } = Typography;
const { TextArea } = Input;

type ProviderValue = 'openai' | 'anthropic' | 'gemini';
type AlertState = {
  type: 'success' | 'info' | 'warning' | 'error';
  title: string;
  message: string;
  suggestions?: string[];
  extra?: string;
};

type SettingsFormValues = {
  api_provider: ProviderValue;
  provider_type?: ProviderValue;
  api_key?: string;
  api_base_url?: string;
  api_backup_urls_text?: string;
  fallback_strategy?: 'auto' | 'manual';
  azure_api_version?: string;
  llm_model?: string;
  temperature?: number;
  max_tokens?: number;
  system_prompt?: string;
  web_research_enabled?: boolean;
  web_research_exa_enabled?: boolean;
  web_research_grok_enabled?: boolean;
  web_research_exa_api_key?: string;
  web_research_exa_base_url?: string;
  web_research_grok_api_key?: string;
  web_research_grok_base_url?: string;
  web_research_grok_model?: string;
  web_research_grok_search_enabled?: boolean;
};

type WebResearchFormValues = Pick<
  SettingsFormValues,
  | 'web_research_enabled'
  | 'web_research_exa_enabled'
  | 'web_research_grok_enabled'
  | 'web_research_exa_api_key'
  | 'web_research_exa_base_url'
  | 'web_research_grok_api_key'
  | 'web_research_grok_base_url'
  | 'web_research_grok_model'
  | 'web_research_grok_search_enabled'
>;

type PresetFormValues = {
  name: string;
  description?: string;
  api_provider: ProviderValue;
  provider_type?: ProviderValue;
  api_key?: string;
  api_base_url?: string;
  api_backup_urls_text?: string;
  fallback_strategy?: 'auto' | 'manual';
  azure_api_version?: string;
  llm_model: string;
  temperature?: number;
  max_tokens?: number;
  system_prompt?: string;
};

type SnapshotFormValues = {
  name: string;
  description?: string;
};

const providerOptions = [
  { value: 'openai', label: 'OpenAI / DeepSeek / OpenRouter / 兼容网关' },
  { value: 'anthropic', label: 'Claude / Anthropic' },
  { value: 'gemini', label: 'Gemini' },
] as const;

const providerPlaceholders: Record<ProviderValue, { baseUrl: string; model: string }> = {
  openai: {
    baseUrl: 'https://api.openai.com/v1',
    model: 'gpt-4o-mini / deepseek-chat / openrouter model id',
  },
  anthropic: {
    baseUrl: 'https://api.anthropic.com',
    model: 'claude-3-5-sonnet-latest',
  },
  gemini: {
    baseUrl: 'https://generativelanguage.googleapis.com/v1beta/openai',
    model: 'gemini-2.5-pro',
  },
};

const defaultMainSettingsValues: SettingsFormValues = {
  api_provider: 'openai',
  provider_type: 'openai',
  api_key: '',
  api_base_url: '',
  api_backup_urls_text: '',
  fallback_strategy: 'auto',
  azure_api_version: '',
  llm_model: '',
  temperature: 0.7,
  max_tokens: 4096,
  system_prompt: '',
};

const defaultWebResearchValues: WebResearchFormValues = {
  web_research_enabled: false,
  web_research_exa_enabled: true,
  web_research_grok_enabled: true,
  web_research_exa_api_key: '',
  web_research_exa_base_url: '',
  web_research_grok_api_key: '',
  web_research_grok_base_url: '',
  web_research_grok_model: 'grok-4.1-fast',
  web_research_grok_search_enabled: false,
};

const normalizeMultilineUrls = (value?: string): string[] =>
  String(value || '')
    .split(/\r?\n|,/)
    .map((item) => item.trim())
    .filter(Boolean);

const mergeModelOptions = (current: string[], incoming: string[]): string[] => {
  const unique = new Set<string>();
  [...current, ...incoming]
    .map((item) => item.trim())
    .filter(Boolean)
    .forEach((item) => unique.add(item));
  return Array.from(unique);
};

const formatProbeResult = (result: {
  success: boolean;
  message: string;
  response_preview?: string;
  error?: string;
  suggestions?: string[];
}): AlertState => ({
  type: result.success ? 'success' : 'error',
  title: result.success ? '测试成功' : '测试失败',
  message: result.error ? `${result.message}：${result.error}` : result.message,
  suggestions: result.suggestions,
  extra: result.response_preview,
});

const buildSettingsPayload = (values: SettingsFormValues): SettingsUpdate => {
  const provider = (values.api_provider || 'openai') as ProviderValue;
  const normalizedApiKey = String(values.api_key || '').trim();
  const normalizedModel = String(values.llm_model || '').trim() || providerPlaceholders[provider].model.split(' / ')[0];
  return {
    api_provider: provider,
    provider_type: provider,
    ...(normalizedApiKey ? { api_key: normalizedApiKey } : {}),
    api_base_url: String(values.api_base_url || '').trim(),
    api_backup_urls: normalizeMultilineUrls(values.api_backup_urls_text),
    fallback_strategy: values.fallback_strategy || 'auto',
    azure_api_version: String(values.azure_api_version || '').trim() || undefined,
    llm_model: normalizedModel,
    temperature: Number(values.temperature ?? 0.7),
    max_tokens: Number(values.max_tokens ?? 4096),
    system_prompt: String(values.system_prompt || '').trim() || undefined,
    web_research_enabled: Boolean(values.web_research_enabled),
    web_research_exa_enabled: Boolean(values.web_research_exa_enabled),
    web_research_grok_enabled: Boolean(values.web_research_grok_enabled),
    web_research_exa_api_key: String(values.web_research_exa_api_key || '').trim() || undefined,
    web_research_exa_base_url: String(values.web_research_exa_base_url || '').trim() || undefined,
    web_research_grok_api_key: String(values.web_research_grok_api_key || '').trim() || undefined,
    web_research_grok_base_url: String(values.web_research_grok_base_url || '').trim() || undefined,
    web_research_grok_model: String(values.web_research_grok_model || '').trim() || undefined,
    web_research_grok_search_enabled: Boolean(values.web_research_grok_search_enabled),
  };
};

const buildWebResearchPayload = (values: WebResearchFormValues): SettingsUpdate => ({
  web_research_enabled: Boolean(values.web_research_enabled),
  web_research_exa_enabled: Boolean(values.web_research_exa_enabled),
  web_research_grok_enabled: Boolean(values.web_research_grok_enabled),
  web_research_exa_api_key: String(values.web_research_exa_api_key || '').trim() || undefined,
  web_research_exa_base_url: String(values.web_research_exa_base_url || '').trim() || undefined,
  web_research_grok_api_key: String(values.web_research_grok_api_key || '').trim() || undefined,
  web_research_grok_base_url: String(values.web_research_grok_base_url || '').trim() || undefined,
  web_research_grok_model: String(values.web_research_grok_model || '').trim() || undefined,
  web_research_grok_search_enabled: Boolean(values.web_research_grok_search_enabled),
});

const buildPresetConfig = (values: PresetFormValues): APIKeyPresetConfig => {
  const provider = (values.api_provider || 'openai') as ProviderValue;
  return {
    api_provider: provider,
    api_key: String(values.api_key || '').trim(),
    api_base_url: String(values.api_base_url || '').trim() || undefined,
    api_backup_urls: normalizeMultilineUrls(values.api_backup_urls_text),
    provider_type: provider,
    fallback_strategy: values.fallback_strategy || 'auto',
    azure_api_version: String(values.azure_api_version || '').trim() || undefined,
    llm_model: String(values.llm_model || '').trim(),
    temperature: Number(values.temperature ?? 0.7),
    max_tokens: Number(values.max_tokens ?? 4096),
    system_prompt: String(values.system_prompt || '').trim() || undefined,
  };
};

const settingsToMainFormValues = (settings?: Settings | null): SettingsFormValues => {
  const provider = ((settings?.provider_type || settings?.api_provider || 'openai').trim().toLowerCase() || 'openai') as ProviderValue;
  return {
    api_provider: provider,
    provider_type: provider,
    api_key: '',
    api_base_url: settings?.api_base_url || '',
    api_backup_urls_text: (settings?.api_backup_urls || []).join('\n'),
    fallback_strategy: settings?.fallback_strategy || 'auto',
    azure_api_version: settings?.azure_api_version || '',
    llm_model: settings?.llm_model || '',
    temperature: settings?.temperature ?? 0.7,
    max_tokens: settings?.max_tokens ?? 4096,
    system_prompt: settings?.system_prompt || '',
  };
};

const settingsToWebResearchFormValues = (settings?: Settings | null): WebResearchFormValues => {
  return {
    web_research_enabled: settings?.web_research_enabled ?? false,
    web_research_exa_enabled: settings?.web_research_exa_enabled ?? true,
    web_research_grok_enabled: settings?.web_research_grok_enabled ?? true,
    web_research_exa_api_key: isPlaceholderApiKey(settings?.web_research_exa_api_key) ? '' : (settings?.web_research_exa_api_key || ''),
    web_research_exa_base_url: settings?.web_research_exa_base_url || '',
    web_research_grok_api_key: isPlaceholderApiKey(settings?.web_research_grok_api_key) ? '' : (settings?.web_research_grok_api_key || ''),
    web_research_grok_base_url: settings?.web_research_grok_base_url || '',
    web_research_grok_model: settings?.web_research_grok_model || 'grok-4.1-fast',
    web_research_grok_search_enabled: settings?.web_research_grok_search_enabled ?? false,
  };
};

const presetToFormValues = (preset?: APIKeyPreset | null): PresetFormValues => {
  const provider = ((preset?.config.provider_type || preset?.config.api_provider || 'openai').trim().toLowerCase() || 'openai') as ProviderValue;
  return {
    name: preset?.name || '',
    description: preset?.description || '',
    api_provider: provider,
    provider_type: provider,
    api_key: preset?.config.api_key || '',
    api_base_url: preset?.config.api_base_url || '',
    api_backup_urls_text: (preset?.config.api_backup_urls || []).join('\n'),
    fallback_strategy: preset?.config.fallback_strategy || 'auto',
    azure_api_version: preset?.config.azure_api_version || '',
    llm_model: preset?.config.llm_model || '',
    temperature: preset?.config.temperature ?? 0.7,
    max_tokens: preset?.config.max_tokens ?? 4096,
    system_prompt: preset?.config.system_prompt || '',
  };
};

export default function SettingsPage() {
  const [settingsForm] = Form.useForm<SettingsFormValues>();
  const [webResearchForm] = Form.useForm<WebResearchFormValues>();
  const [presetForm] = Form.useForm<PresetFormValues>();
  const [snapshotForm] = Form.useForm<SnapshotFormValues>();
  const { token } = theme.useToken();
  const mountedRef = useRef(true);
  const settingsRequestIdRef = useRef(0);
  const presetRequestIdRef = useRef(0);
  const storedApiKeyRequestIdRef = useRef(0);

  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
      settingsRequestIdRef.current += 1;
      presetRequestIdRef.current += 1;
      storedApiKeyRequestIdRef.current += 1;
    };
  }, []);
  const isMobile = window.innerWidth <= 768;
  const alphaColor = (color: string, alpha: number) => `color-mix(in srgb, ${color} ${(alpha * 100).toFixed(0)}%, transparent)`;
  const editorialInk = '#f7f1e8';
  const pageBackground = `linear-gradient(180deg, ${alphaColor(token.colorPrimary, 0.05)} 0%, ${token.colorBgLayout} 28%, ${token.colorBgLayout} 100%)`;
  const heroBackground = `linear-gradient(135deg, #171411 0%, color-mix(in srgb, #171411 64%, ${token.colorPrimary} 36%) 100%)`;
  const panelBackground = `linear-gradient(180deg, ${token.colorBgContainer} 0%, color-mix(in srgb, ${token.colorBgContainer} 76%, ${token.colorFillAlter} 24%) 100%)`;
  const panelBorder = alphaColor(token.colorPrimary, 0.1);
  const quietPanelBackground = `linear-gradient(180deg, color-mix(in srgb, ${token.colorBgContainer} 92%, ${token.colorFillAlter} 8%) 0%, color-mix(in srgb, ${token.colorBgContainer} 82%, ${token.colorFillAlter} 18%) 100%)`;
  const modalSurfaceStyles = {
    header: { padding: '22px 24px 0', borderBottom: 'none' },
    body: { padding: '0 24px 24px' },
    footer: { padding: '0 24px 24px', borderTop: 'none' },
  } as const;

  const [loadingSettings, setLoadingSettings] = useState(true);
  const [savingSettings, setSavingSettings] = useState(false);
  const [fetchingModels, setFetchingModels] = useState(false);
  const [testingConnection, setTestingConnection] = useState(false);
  const [testingWebResearch, setTestingWebResearch] = useState(false);
  const [checkingFunctionCalling, setCheckingFunctionCalling] = useState(false);
  const [loadingPresets, setLoadingPresets] = useState(false);
  const [submittingPreset, setSubmittingPreset] = useState(false);
  const [creatingSnapshot, setCreatingSnapshot] = useState(false);

  const [settingsRecord, setSettingsRecord] = useState<Settings | null>(null);
  const [presets, setPresets] = useState<APIKeyPreset[]>([]);
  const [modelOptions, setModelOptions] = useState<string[]>([]);
  const [probeAlert, setProbeAlert] = useState<AlertState | null>(null);
  const [webResearchAlert, setWebResearchAlert] = useState<AlertState | null>(null);
  const [functionCallingAlert, setFunctionCallingAlert] = useState<AlertState | null>(null);
  const [editingPreset, setEditingPreset] = useState<APIKeyPreset | null>(null);
  const [presetModalOpen, setPresetModalOpen] = useState(false);
  const [snapshotModalOpen, setSnapshotModalOpen] = useState(false);
  const [showStoredApiKey, setShowStoredApiKey] = useState(false);
  const [storedApiKeyPreview, setStoredApiKeyPreview] = useState('');
  const [loadingStoredApiKey, setLoadingStoredApiKey] = useState(false);
  const [clearStoredApiKey, setClearStoredApiKey] = useState(false);

  const providerValue = Form.useWatch('api_provider', settingsForm) || 'openai';
  const currentModel = Form.useWatch('llm_model', settingsForm) || '';
  const webResearchEnabled = Form.useWatch('web_research_enabled', webResearchForm) ?? false;
  const exaEnabled = Form.useWatch('web_research_exa_enabled', webResearchForm) ?? true;
  const grokEnabled = Form.useWatch('web_research_grok_enabled', webResearchForm) ?? true;

  const hasStoredApiKey = Boolean(settingsRecord?.has_api_key)
  const apiKeyPlaceholder = hasStoredApiKey
    ? '已保存密钥；留空表示保持不变，输入新值可覆盖'
    : 'sk-...';

  const providerHint = providerPlaceholders[(providerValue as ProviderValue) || 'openai'];
  const modelSelectOptions = useMemo(
    () => modelOptions.map((model) => ({ label: model, value: model })),
    [modelOptions],
  );

  const loadPresets = useCallback(async () => {
    const requestId = ++presetRequestIdRef.current;
    try {
      setLoadingPresets(true);
      const response = await settingsApi.getPresets();
      if (!mountedRef.current || presetRequestIdRef.current !== requestId) {
        return;
      }
      setPresets(response.presets || []);
    } catch (error) {
      if (!mountedRef.current || presetRequestIdRef.current !== requestId) {
        return;
      }
      console.error('load presets failed', error);
      message.error('加载预设失败');
    } finally {
      if (mountedRef.current && presetRequestIdRef.current === requestId) {
        setLoadingPresets(false);
      }
    }
  }, []);

  const loadSettings = useCallback(async () => {
    const requestId = ++settingsRequestIdRef.current;
    try {
      setLoadingSettings(true);
      const response = await settingsApi.getSettings();
      if (!mountedRef.current || settingsRequestIdRef.current !== requestId) {
        return;
      }
      const normalizedResponse = {
        ...response,
        llm_model: String(response.llm_model || '').trim(),
        has_api_key: Boolean(response.has_api_key),
      };
      setSettingsRecord(normalizedResponse);
      setShowStoredApiKey(false);
      setStoredApiKeyPreview('');
      setClearStoredApiKey(false);
      settingsForm.setFieldsValue(settingsToMainFormValues(normalizedResponse));
      webResearchForm.setFieldsValue(settingsToWebResearchFormValues(normalizedResponse));
      setModelOptions((current) => mergeModelOptions(current, [normalizedResponse.llm_model || '']));
      setProbeAlert(null);
      setWebResearchAlert(null);
      setFunctionCallingAlert(null);
    } catch (error) {
      if (!mountedRef.current || settingsRequestIdRef.current !== requestId) {
        return;
      }
      console.error('load settings failed', error);
      message.error('加载设置失败');
    } finally {
      if (mountedRef.current && settingsRequestIdRef.current === requestId) {
        setLoadingSettings(false);
      }
    }
  }, [settingsForm, webResearchForm]);

  useEffect(() => {
    void loadSettings();
    void loadPresets();
  }, [loadPresets, loadSettings]);

  const handleToggleStoredApiKey = useCallback(async () => {
    if (showStoredApiKey) {
      setShowStoredApiKey(false);
      setStoredApiKeyPreview('');
      return;
    }

    if (!hasStoredApiKey) {
      message.info('当前没有已保存的 API Key');
      return;
    }

    const requestId = ++storedApiKeyRequestIdRef.current;
    try {
      setLoadingStoredApiKey(true);
      const response = await settingsApi.getStoredApiKey();
      if (!mountedRef.current || storedApiKeyRequestIdRef.current !== requestId) {
        return;
      }
      if (!response.has_api_key || !String(response.api_key || '').trim()) {
        message.warning('没有读取到已保存的 API Key');
        return;
      }
      setStoredApiKeyPreview(String(response.api_key || '').trim());
      setClearStoredApiKey(false);
      setShowStoredApiKey(true);
    } catch (error) {
      if (!mountedRef.current || storedApiKeyRequestIdRef.current !== requestId) {
        return;
      }
      console.error('load stored api key failed', error);
      message.error('读取已保存 API Key 失败');
    } finally {
      if (mountedRef.current && storedApiKeyRequestIdRef.current === requestId) {
        setLoadingStoredApiKey(false);
      }
    }
  }, [hasStoredApiKey, showStoredApiKey]);

  const ensureCurrentModelVisible = useCallback(() => {
    const currentModel = String(settingsForm.getFieldValue('llm_model') || '').trim();
    if (!currentModel) {
      return;
    }
    setModelOptions((current) => mergeModelOptions(current, [currentModel]));
  }, [settingsForm]);

  const resolveApiKeyForAction = useCallback((formValues: SettingsFormValues) => {
    const raw = String(formValues.api_key || '').trim();
    if (raw) {
      return raw;
    }
    return undefined;
  }, []);
  const handleSaveSettings = async () => {
    try {
      const values = await settingsForm.validateFields();
      const webResearchValues = webResearchForm.getFieldsValue(true);
      const mergedValues: SettingsFormValues = {
        ...values,
        ...webResearchValues,
      };
      const payload = buildSettingsPayload(mergedValues);
      const clearApiKey = clearStoredApiKey && !String(values.api_key || '').trim();
      if (clearApiKey) {
        payload.clear_api_key = true;
      } else if (!String(values.api_key || '').trim() && hasStoredApiKey) {
        delete payload.api_key;
      }
      setSavingSettings(true);
      const saved = settingsRecord
        ? await settingsApi.updateSettings(payload)
        : await settingsApi.saveSettings(payload);
      const normalizedSaved = {
        ...saved,
        llm_model: String(saved.llm_model || values.llm_model || providerHint.model || '').trim(),
        has_api_key: Boolean(saved.has_api_key),
      };
      setSettingsRecord(normalizedSaved);
      setShowStoredApiKey(false);
      setStoredApiKeyPreview('');
      setClearStoredApiKey(false);
      settingsForm.setFieldsValue(settingsToMainFormValues(normalizedSaved));
      webResearchForm.setFieldsValue(settingsToWebResearchFormValues(normalizedSaved));
      setModelOptions((current) => mergeModelOptions(current, [normalizedSaved.llm_model || String(values.llm_model || ''), providerHint.model]));
      message.success('设置已保存');
    } catch (error) {
      if (error && typeof error === 'object' && 'errorFields' in error) {
        return;
      }
      console.error('save settings failed', error);
      message.error('保存设置失败');
    } finally {
      setSavingSettings(false);
    }
  };

  const handleSaveWebResearchSettings = async () => {
    try {
      const values = await webResearchForm.validateFields();
      const payload = buildWebResearchPayload(values);
      setSavingSettings(true);
      const saved = settingsRecord
        ? await settingsApi.updateSettings(payload)
        : await settingsApi.saveSettings(payload);
      const normalizedSaved = {
        ...saved,
        llm_model: String(saved.llm_model || settingsForm.getFieldValue('llm_model') || providerHint.model || '').trim(),
        has_api_key: Boolean(saved.has_api_key),
      };
      setSettingsRecord(normalizedSaved);
      webResearchForm.setFieldsValue(settingsToWebResearchFormValues(normalizedSaved));
      settingsForm.setFieldsValue(settingsToMainFormValues(normalizedSaved));
      setWebResearchAlert(null);
      message.success('Web Research 设置已保存');
    } catch (error) {
      if (error && typeof error === 'object' && 'errorFields' in error) {
        return;
      }
      console.error('save web research settings failed', error);
      message.error('保存 Web Research 设置失败');
    } finally {
      setSavingSettings(false);
    }
  };

  const handleFetchModels = async () => {
    try {
      const values = await settingsForm.validateFields(['api_provider', 'api_base_url']);
      const apiKey = resolveApiKeyForAction(settingsForm.getFieldsValue(true));
      if (!apiKey || isPlaceholderApiKey(apiKey)) {
        if (!hasStoredApiKey) {
          message.warning('请先输入 API Key，或先保存一个真实密钥');
          return;
        }
      }

      setFetchingModels(true);
      const result = await settingsApi.fetchModels({
        provider: values.api_provider,
        ...(apiKey ? { api_key: apiKey } : {}),
        api_base_url: String(values.api_base_url || '').trim(),
      });

      if (!result.success) {
        message.error(result.message || result.error || '获取模型失败');
        return;
      }

      const models = (result.models || []).map((item) => item.id).filter(Boolean);
      setModelOptions((current) => mergeModelOptions(current, models));
      ensureCurrentModelVisible();
      message.success(result.message || `已获取 ${models.length} 个模型`);
    } catch (error) {
      if (error && typeof error === 'object' && 'errorFields' in error) {
        return;
      }
      console.error('fetch models failed', error);
      message.error('获取模型失败');
    } finally {
      setFetchingModels(false);
    }
  };

  const handleTestConnection = async () => {
    try {
      const values = await settingsForm.validateFields(['api_provider', 'api_base_url', 'llm_model']);
      const allValues = settingsForm.getFieldsValue(true);
      const apiKey = resolveApiKeyForAction(allValues);
      if (!apiKey || isPlaceholderApiKey(apiKey)) {
        if (!hasStoredApiKey) {
          message.warning('请先输入 API Key，或先保存一个真实密钥');
          return;
        }
      }

      setTestingConnection(true);
      const result = await settingsApi.testApiConnection({
        provider: values.api_provider,
        ...(apiKey ? { api_key: apiKey } : {}),
        api_base_url: String(values.api_base_url || '').trim(),
        llm_model: String(values.llm_model || '').trim(),
        temperature: Number(allValues.temperature ?? 0.7),
        max_tokens: Number(allValues.max_tokens ?? 4096),
        api_backup_urls: normalizeMultilineUrls(allValues.api_backup_urls_text),
        fallback_strategy: allValues.fallback_strategy || 'auto',
      });

      setProbeAlert(formatProbeResult(result));
      if (result.success) {
        message.success('API 测试成功');
      } else {
        message.warning('API 测试失败，请检查诊断信息');
      }
    } catch (error) {
      if (error && typeof error === 'object' && 'errorFields' in error) {
        return;
      }
      console.error('test api failed', error);
      message.error('API 测试失败');
    } finally {
      setTestingConnection(false);
    }
  };

  const handleCheckFunctionCalling = async () => {
    try {
      const values = await settingsForm.validateFields(['api_provider', 'api_base_url', 'llm_model']);
      const allValues = settingsForm.getFieldsValue(true);
      const apiKey = resolveApiKeyForAction(allValues);
      if (!apiKey || isPlaceholderApiKey(apiKey)) {
        if (!hasStoredApiKey) {
          message.warning('请先输入 API Key，或先保存一个真实密钥');
          return;
        }
      }

      setCheckingFunctionCalling(true);
      const result = await settingsApi.checkFunctionCalling({
        provider: values.api_provider,
        ...(apiKey ? { api_key: apiKey } : {}),
        api_base_url: String(values.api_base_url || '').trim(),
        llm_model: String(values.llm_model || '').trim(),
        api_backup_urls: normalizeMultilineUrls(allValues.api_backup_urls_text),
        fallback_strategy: allValues.fallback_strategy || 'auto',
      });

      setFunctionCallingAlert({
        type: result.success ? 'success' : 'warning',
        title: result.supported ? 'Function Calling 可用' : 'Function Calling 不可用或未确认',
        message: result.error ? `${result.message}：${result.error}` : result.message,
        suggestions: result.suggestions,
        extra: result.response_preview,
      });
    } catch (error) {
      if (error && typeof error === 'object' && 'errorFields' in error) {
        return;
      }
      console.error('check function calling failed', error);
      message.error('Function Calling 检测失败');
    } finally {
      setCheckingFunctionCalling(false);
    }
  };

  const handleTestWebResearch = async () => {
    try {
      const values = webResearchForm.getFieldsValue(true);
      const exaApiKey = String(values.web_research_exa_api_key || '').trim();
      const grokApiKey = String(values.web_research_grok_api_key || '').trim();
      const grokBaseUrl = String(values.web_research_grok_base_url || '').trim();

      if (exaEnabled && exaApiKey) {
        setTestingWebResearch(true);
        const result = await settingsApi.testWebResearchConnection({
          provider: 'exa',
          exa_api_key: exaApiKey,
          exa_base_url: String(values.web_research_exa_base_url || '').trim() || undefined,
        });
        setWebResearchAlert(formatProbeResult(result));
        return;
      }

      if (grokEnabled && grokApiKey && grokBaseUrl) {
        setTestingWebResearch(true);
        const result = await settingsApi.testWebResearchConnection({
          provider: 'grok',
          grok_api_key: grokApiKey,
          grok_base_url: grokBaseUrl,
          grok_model: String(values.web_research_grok_model || '').trim() || undefined,
          grok_search_enabled: Boolean(values.web_research_grok_search_enabled),
        });
        setWebResearchAlert(formatProbeResult(result));
        return;
      }

      message.warning('请至少填写一组可测试的 Web Research 配置');
    } catch (error) {
      console.error('test web research failed', error);
      message.error('Web Research 测试失败');
    } finally {
      setTestingWebResearch(false);
    }
  };

  const openCreatePreset = () => {
    setEditingPreset(null);
    presetForm.setFieldsValue({
      ...presetToFormValues(null),
      ...presetToFormValues({
        id: '',
        name: '',
        description: '',
        is_active: false,
        created_at: '',
        config: buildPresetConfig({
          name: '',
          description: '',
          api_provider: providerValue as ProviderValue,
          provider_type: providerValue as ProviderValue,
          api_key: String(settingsForm.getFieldValue('api_key') || '').trim(),
          api_base_url: String(settingsForm.getFieldValue('api_base_url') || '').trim(),
          api_backup_urls_text: String(settingsForm.getFieldValue('api_backup_urls_text') || ''),
          fallback_strategy: settingsForm.getFieldValue('fallback_strategy') || 'auto',
          azure_api_version: String(settingsForm.getFieldValue('azure_api_version') || ''),
          llm_model: String(settingsForm.getFieldValue('llm_model') || '').trim(),
          temperature: Number(settingsForm.getFieldValue('temperature') ?? 0.7),
          max_tokens: Number(settingsForm.getFieldValue('max_tokens') ?? 4096),
          system_prompt: String(settingsForm.getFieldValue('system_prompt') || ''),
        }),
      }),
    });
    setPresetModalOpen(true);
  };

  const openEditPreset = (preset: APIKeyPreset) => {
    setEditingPreset(preset);
    presetForm.setFieldsValue(presetToFormValues(preset));
    setPresetModalOpen(true);
  };

  const handleSubmitPreset = async () => {
    try {
      const values = await presetForm.validateFields();
      const config = buildPresetConfig(values);
      if (!config.api_key || isPlaceholderApiKey(config.api_key)) {
        message.warning('预设必须保存真实 API Key');
        return;
      }

      setSubmittingPreset(true);
      if (editingPreset) {
        const payload: PresetUpdateRequest = {
          name: values.name,
          description: values.description,
          config,
        };
        await settingsApi.updatePreset(editingPreset.id, payload);
        message.success('预设已更新');
      } else {
        const payload: PresetCreateRequest = {
          name: values.name,
          description: values.description,
          config,
        };
        await settingsApi.createPreset(payload);
        message.success('预设已创建');
      }

      setPresetModalOpen(false);
      await loadPresets();
    } catch (error) {
      if (error && typeof error === 'object' && 'errorFields' in error) {
        return;
      }
      console.error('submit preset failed', error);
      message.error('保存预设失败');
    } finally {
      setSubmittingPreset(false);
    }
  };
  const handleDeletePreset = async (presetId: string) => {
    try {
      await settingsApi.deletePreset(presetId);
      message.success('预设已删除');
      await loadPresets();
    } catch (error) {
      console.error('delete preset failed', error);
      message.error('删除预设失败');
    }
  };

  const handleActivatePreset = async (presetId: string) => {
    try {
      await settingsApi.activatePreset(presetId);
      message.success('预设已激活');
      await Promise.all([loadSettings(), loadPresets()]);
    } catch (error) {
      console.error('activate preset failed', error);
      message.error('激活预设失败');
    }
  };

  const handleTestPreset = async (presetId: string) => {
    try {
      const result = await settingsApi.testPreset(presetId);
      setProbeAlert(formatProbeResult(result));
      message[result.success ? 'success' : 'warning'](result.message || '预设测试完成');
    } catch (error) {
      console.error('test preset failed', error);
      message.error('测试预设失败');
    }
  };

  const handleCreateSnapshotPreset = async () => {
    try {
      const values = await snapshotForm.validateFields();
      setCreatingSnapshot(true);
      await settingsApi.createPresetFromCurrent(values.name, values.description);
      message.success('已从当前配置创建预设');
      setSnapshotModalOpen(false);
      await loadPresets();
    } catch (error) {
      if (error && typeof error === 'object' && 'errorFields' in error) {
        return;
      }
      console.error('create preset from current failed', error);
      message.error('从当前配置创建预设失败');
    } finally {
      setCreatingSnapshot(false);
    }
  };

  const renderAlert = (alert: AlertState | null) => {
    if (!alert) {
      return null;
    }

    return (
      <Alert
        showIcon
        type={alert.type}
        message={alert.title}
        description={
          <Space direction="vertical" size={6} style={{ width: '100%' }}>
            <Text>{alert.message}</Text>
            {alert.suggestions?.length ? (
              <div>
                <Text strong>建议：</Text>
                <ul style={{ margin: '6px 0 0 18px', padding: 0 }}>
                  {alert.suggestions.map((item) => (
                    <li key={item}>{item}</li>
                  ))}
                </ul>
              </div>
            ) : null}
            {alert.extra ? (
              <Paragraph copyable={{ text: alert.extra }} style={{ marginBottom: 0 }}>
                {alert.extra}
              </Paragraph>
            ) : null}
          </Space>
        }
      />
    );
  };

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: isMobile ? 16 : 20, paddingBottom: 24, background: pageBackground }}>
      <Card
        variant="borderless"
        style={{
          background: heroBackground,
          borderRadius: isMobile ? 22 : 28,
          border: `1px solid ${alphaColor(editorialInk, 0.08)}`,
          boxShadow: `0 24px 48px ${alphaColor(token.colorText, 0.16)}`,
          overflow: 'hidden',
        }}
        styles={{ body: { padding: isMobile ? 18 : 24 } }}
      >
        <div style={{ position: 'absolute', top: -56, right: -40, width: 180, height: 180, borderRadius: '50%', background: alphaColor(editorialInk, 0.06), pointerEvents: 'none' }} />
        <div style={{ position: 'absolute', bottom: -34, left: '24%', width: 110, height: 110, borderRadius: '50%', background: alphaColor(editorialInk, 0.05), pointerEvents: 'none' }} />
        <Row gutter={[24, 20]} align="middle" style={{ position: 'relative', zIndex: 1 }}>
          <Col xs={24} lg={15}>
            <Space direction="vertical" size={8} style={{ width: '100%' }}>
              <Text style={{ color: alphaColor(editorialInk, 0.7), fontSize: 11, letterSpacing: '0.18em', textTransform: 'uppercase' }}>
                Configuration
              </Text>
              <Title
                level={isMobile ? 3 : 2}
                style={{ margin: 0, color: editorialInk, fontFamily: designDisplayFont, letterSpacing: '-0.03em' }}
              >
                <ApiOutlined style={{ marginRight: 10, fontSize: isMobile ? 24 : 28, color: alphaColor(editorialInk, 0.88) }} />
                API 设置
              </Title>
              <Paragraph style={{ margin: 0, color: alphaColor(editorialInk, 0.8), fontSize: isMobile ? 13 : 15, lineHeight: 1.8 }}>
                这里统一管理主模型、Web Research 和 API 预设。页面结构按文档型工作台整理，方便你一边理解参数含义，一边安全地调整运行配置。
              </Paragraph>
            </Space>
          </Col>
          <Col xs={24} lg={9}>
            <Space direction="vertical" size={12} style={{ width: '100%' }}>
              {[ 
                { label: '当前模型', value: currentModel || '未设置' },
                { label: '已保存密钥', value: hasStoredApiKey ? '已保存' : '未保存' },
                { label: '可用预设', value: `${presets.length} 个` },
              ].map((item) => (
                <div
                  key={item.label}
                  style={{
                    display: 'flex',
                    justifyContent: 'space-between',
                    alignItems: 'center',
                    gap: 12,
                    borderRadius: 18,
                    padding: '12px 14px',
                    background: alphaColor('#ffffff', 0.08),
                    border: `1px solid ${alphaColor(editorialInk, 0.1)}`,
                    backdropFilter: 'blur(10px)',
                  }}
                >
                  <Text style={{ color: alphaColor(editorialInk, 0.72), fontSize: 12 }}>{item.label}</Text>
                  <Text style={{ color: editorialInk, fontWeight: 600 }}>{item.value}</Text>
                </div>
              ))}
            </Space>
          </Col>
        </Row>
      </Card>

      {loadingSettings ? (
        <InlineDeferredPanel
          eyebrow="Settings Workspace"
          title="正在展开设置工作区"
          message="系统正在恢复模型配置、联网研究与预设管理面板，原有设置读取、保存和测试逻辑保持不变。"
          minHeight={isMobile ? 320 : 360}
          tags={[
            { label: '设置中心', color: 'processing' },
            { label: '主链路配置恢复中', color: 'gold' },
            { label: '保存逻辑保持原样', color: 'green' },
          ]}
        />
      ) : (
        <Card
          variant="borderless"
          style={{
            borderRadius: isMobile ? 20 : 24,
            background: panelBackground,
            border: `1px solid ${panelBorder}`,
            boxShadow: `0 20px 40px ${alphaColor(token.colorText, 0.06)}`,
          }}
          styles={{ body: { padding: isMobile ? 14 : 18 } }}
        >
        <Tabs
          tabBarStyle={{ marginBottom: isMobile ? 16 : 20 }}
          items={[
            {
              key: 'current',
              label: '当前配置',
              children: (
                <Space direction="vertical" size={16} style={{ width: '100%' }}>
                  <Alert
                    type="info"
                    showIcon
                    style={{
                      borderRadius: 16,
                      border: `1px solid ${alphaColor(token.colorInfo, 0.2)}`,
                      background: alphaColor(token.colorInfo, 0.08),
                    }}
                    message="建议先完成主模型配置，再测试连接；确认模型探测正常后，再单独配置 Web Research。"
                  />

                  <Card
                    variant="borderless"
                    style={{
                      borderRadius: 20,
                      background: quietPanelBackground,
                      border: `1px solid ${panelBorder}`,
                    }}
                  >
                    <Space direction="vertical" size={4} style={{ width: '100%', marginBottom: 20 }}>
                      <Text style={{ fontSize: 11, letterSpacing: '0.18em', textTransform: 'uppercase', color: token.colorTextTertiary }}>
                        Model Workspace
                      </Text>
                      <Title level={isMobile ? 4 : 3} style={{ margin: 0, fontFamily: designDisplayFont, letterSpacing: '-0.03em' }}>
                        主模型与接口配置
                      </Title>
                      <Text type="secondary">
                        适合放这里的内容只有一件事：把“生成主链路”配置到可用状态。操作说明放右侧，不把表单本身做得过于花哨。
                      </Text>
                    </Space>
                    <Row gutter={[16, 16]}>
                      <Col xs={24} lg={16}>
                        <Form form={settingsForm} layout="vertical" initialValues={defaultMainSettingsValues}>
                          <Row gutter={[16, 0]}>
                            <Col xs={24} md={12}>
                              <Form.Item label="提供商" name="api_provider" rules={[{ required: true, message: '请选择提供商' }]}>
                                <Select options={providerOptions as unknown as { label: string; value: string }[]} />
                              </Form.Item>
                            </Col>
                            <Col xs={24} md={12}>
                              <Form.Item label="回退策略" name="fallback_strategy">
                                <Select
                                  options={[
                                    { value: 'auto', label: '自动回退' },
                                    { value: 'manual', label: '手动切换' },
                                  ]}
                                />
                              </Form.Item>
                            </Col>
                            <Col span={24}>
                              <Form.Item label="API Key" name="api_key">
                                <Input.Password
                                  placeholder={apiKeyPlaceholder}
                                  autoComplete="new-password"
                                  onChange={(event) => {
                                    if (String(event.target.value || '').trim()) {
                                      setClearStoredApiKey(false);
                                    }
                                  }}
                                  addonAfter={hasStoredApiKey ? (
                                    <Space size={4}>
                                      <Button
                                        type="link"
                                        size="small"
                                        loading={loadingStoredApiKey}
                                        onClick={() => void handleToggleStoredApiKey()}
                                      >
                                        {showStoredApiKey ? '隐藏已保存密钥' : '显示已保存密钥'}
                                      </Button>
                                      <Button
                                        type="link"
                                        size="small"
                                        danger
                                        onClick={() => {
                                          settingsForm.setFieldValue('api_key', '');
                                          setShowStoredApiKey(false);
                                          setStoredApiKeyPreview('');
                                          setClearStoredApiKey(true);
                                          message.info('已标记清空已保存 API Key，保存设置后生效');
                                        }}
                                      >
                                        清空已保存密钥
                                      </Button>
                                    </Space>
                                  ) : undefined}
                                />
                              </Form.Item>
                              {hasStoredApiKey && !showStoredApiKey ? (
                                <Alert
                                  type="info"
                                  showIcon
                                  message="已保存 API Key"
                                  description={clearStoredApiKey ? "当前已标记清空已保存密钥；点击保存后生效。若改为输入新值，则会覆盖而不是清空。" : "当前输入框留空表示保持已保存密钥不变；输入新值并保存会覆盖。如需删除，请点击右侧“清空已保存密钥”。"}
                                  style={{ marginTop: -12, marginBottom: 16 }}
                                />
                              ) : null}
                              {showStoredApiKey && storedApiKeyPreview ? (
                                <Alert
                                  type="info"
                                  showIcon
                                  message="已保存 API Key"
                                  description={
                                    <Typography.Text code copyable={{ text: storedApiKeyPreview }}>
                                      {storedApiKeyPreview}
                                    </Typography.Text>
                                  }
                                  style={{ marginTop: -12, marginBottom: 16 }}
                                />
                              ) : null}
                            </Col>
                            <Col span={24}>
                              <Form.Item label="API Base URL" name="api_base_url" rules={[{ required: true, message: '请输入 API Base URL' }]}>
                                <Input placeholder={providerHint.baseUrl} />
                              </Form.Item>
                            </Col>
                            <Col span={24}>
                              <Form.Item label="备用 Base URL（每行一个，可选）" name="api_backup_urls_text">
                                <TextArea rows={3} placeholder="https://example-1.com/v1&#10;https://example-2.com/v1" />
                              </Form.Item>
                            </Col>
                            <Col xs={24} md={12}>
                              <Form.Item label="模型" name="llm_model" rules={[{ required: true, message: '请输入或选择模型' }]}>
                                <Select
                                  showSearch
                                  allowClear
                                  options={modelSelectOptions}
                                  placeholder={providerHint.model}
                                  onDropdownVisibleChange={(open) => {
                                    if (open) {
                                      ensureCurrentModelVisible();
                                    }
                                  }}
                                  dropdownRender={(menu) => (
                                    <>
                                      {menu}
                                      <div style={{ padding: 8, borderTop: `1px solid ${token.colorBorderSecondary}` }}>
                                        <Text type="secondary">若列表为空，可直接输入模型名并保存。</Text>
                                      </div>
                                    </>
                                  )}
                                  mode={undefined}
                                />
                              </Form.Item>
                            </Col>
                            <Col xs={24} md={12}>
                              <Form.Item label="Azure API Version（可选）" name="azure_api_version">
                                <Input placeholder="2024-02-01" />
                              </Form.Item>
                            </Col>
                            <Col xs={24} md={12}>
                              <Form.Item label="Temperature" name="temperature">
                                <InputNumber min={0} max={2} step={0.1} style={{ width: '100%' }} />
                              </Form.Item>
                            </Col>
                            <Col xs={24} md={12}>
                              <Form.Item label="Max Tokens" name="max_tokens">
                                <InputNumber min={1} max={200000} step={256} style={{ width: '100%' }} />
                              </Form.Item>
                            </Col>
                            <Col span={24}>
                              <Form.Item label="System Prompt（可选）" name="system_prompt">
                                <TextArea rows={5} placeholder="给模型的系统提示词" />
                              </Form.Item>
                            </Col>
                          </Row>
                        </Form>
                      </Col>
                      <Col xs={24} lg={8}>
                        <Card
                          size="small"
                          title="当前状态"
                          variant="borderless"
                          style={{
                            background: alphaColor(token.colorPrimary, 0.05),
                            borderRadius: 18,
                            marginBottom: 12,
                            border: `1px solid ${alphaColor(token.colorPrimary, 0.08)}`,
                          }}
                        >
                          <Space direction="vertical" size={8} style={{ width: '100%' }}>
                            <Text>已保存密钥：{hasStoredApiKey ? '是' : '否'}</Text>
                            <Text>当前模型：{settingsForm.getFieldValue('llm_model') || '未设置'}</Text>
                            <Text>已加载预设：{presets.length}</Text>
                            <Text type="secondary">
                              如果 API Key 输入框留空，保存时会保持现有密钥，不会被 `********` 覆盖。
                            </Text>
                          </Space>
                        </Card>
                        <Card
                          size="small"
                          title="配置建议"
                          variant="borderless"
                          style={{
                            background: alphaColor(token.colorWarning, 0.06),
                            borderRadius: 18,
                            border: `1px solid ${alphaColor(token.colorWarning, 0.12)}`,
                          }}
                        >
                          <Space direction="vertical" size={8} style={{ width: '100%' }}>
                            <Text type="secondary">1. 先填 `API Base URL` 和 `API Key`。</Text>
                            <Text type="secondary">2. 再获取模型或直接输入模型名。</Text>
                            <Text type="secondary">3. 成功连通后再微调 `Temperature / Max Tokens`。</Text>
                            <Tag color="processing" style={{ width: 'fit-content', margin: 0 }}>
                              当前推荐占位：{providerHint.model}
                            </Tag>
                          </Space>
                        </Card>
                      </Col>
                    </Row>
                    <Space wrap>
                      <Button type="primary" icon={<SaveOutlined />} loading={savingSettings} onClick={() => void handleSaveSettings()}>
                        保存设置
                      </Button>
                      <Button icon={<ReloadOutlined />} loading={fetchingModels} onClick={() => void handleFetchModels()}>
                        获取模型
                      </Button>
                      <Button icon={<PlayCircleOutlined />} loading={testingConnection} onClick={() => void handleTestConnection()}>
                        测试连接
                      </Button>
                      <Button icon={<ThunderboltOutlined />} loading={checkingFunctionCalling} onClick={() => void handleCheckFunctionCalling()}>
                        检测 Function Calling
                      </Button>
                    </Space>
                  </Card>

                  {renderAlert(probeAlert)}
                  {renderAlert(functionCallingAlert)}

                  <Card
                    title="生成前网络检索（Web Research）"
                    variant="borderless"
                    style={{
                      borderRadius: 20,
                      background: quietPanelBackground,
                      border: `1px solid ${panelBorder}`,
                    }}
                  >
                    <Space direction="vertical" size={4} style={{ width: '100%', marginBottom: 20 }}>
                      <Text style={{ fontSize: 11, letterSpacing: '0.18em', textTransform: 'uppercase', color: token.colorTextTertiary }}>
                        Research Stack
                      </Text>
                      <Title level={isMobile ? 4 : 3} style={{ margin: 0, fontFamily: designDisplayFont, letterSpacing: '-0.03em' }}>
                        生成前网络检索（Web Research）
                      </Title>
                      <Text type="secondary">
                        这一层更像“输入增强器”。目标是把事实资料、外部趋势和检索结论带进生成链路，而不是让联网配置本身变成新的维护负担。
                      </Text>
                    </Space>
                    <Form form={webResearchForm} layout="vertical" initialValues={defaultWebResearchValues}>
                      <Row gutter={[16, 0]}>
                        <Col xs={24} md={8}>
                          <Form.Item label="启用 Web Research" name="web_research_enabled" valuePropName="checked">
                            <Switch />
                          </Form.Item>
                        </Col>
                        <Col xs={24} md={8}>
                          <Form.Item label="启用 Exa" name="web_research_exa_enabled" valuePropName="checked">
                            <Switch disabled={!webResearchEnabled} />
                          </Form.Item>
                        </Col>
                        <Col xs={24} md={8}>
                          <Form.Item label="启用 Grok" name="web_research_grok_enabled" valuePropName="checked">
                            <Switch disabled={!webResearchEnabled} />
                          </Form.Item>
                        </Col>
                        <Col xs={24} md={12}>
                          <Form.Item label="Exa API Key" name="web_research_exa_api_key">
                            <Input.Password placeholder="exa_..." disabled={!webResearchEnabled || !exaEnabled} />
                          </Form.Item>
                        </Col>
                        <Col xs={24} md={12}>
                          <Form.Item label="Exa Base URL（可选）" name="web_research_exa_base_url">
                            <Input placeholder="https://api.exa.ai" disabled={!webResearchEnabled || !exaEnabled} />
                          </Form.Item>
                        </Col>
                        <Col xs={24} md={12}>
                          <Form.Item label="Grok API Key" name="web_research_grok_api_key">
                            <Input.Password placeholder="xai-..." disabled={!webResearchEnabled || !grokEnabled} />
                          </Form.Item>
                        </Col>
                        <Col xs={24} md={12}>
                          <Form.Item label="Grok Base URL" name="web_research_grok_base_url">
                            <Input placeholder="https://api.x.ai/v1" disabled={!webResearchEnabled || !grokEnabled} />
                          </Form.Item>
                        </Col>
                        <Col xs={24} md={12}>
                          <Form.Item label="Grok Model" name="web_research_grok_model">
                            <Input placeholder="grok-4.1-fast" disabled={!webResearchEnabled || !grokEnabled} />
                          </Form.Item>
                        </Col>
                        <Col xs={24} md={12}>
                          <Form.Item label="启用 Grok Search" name="web_research_grok_search_enabled" valuePropName="checked">
                            <Switch disabled={!webResearchEnabled || !grokEnabled} />
                          </Form.Item>
                        </Col>
                      </Row>
                    </Form>
                    <Space wrap>
                      <Button type="primary" icon={<SaveOutlined />} loading={savingSettings} onClick={() => void handleSaveWebResearchSettings()}>
                        保存 Web Research 设置
                      </Button>
                      <Button icon={<ExperimentOutlined />} loading={testingWebResearch} onClick={() => void handleTestWebResearch()}>
                        测试 Web Research
                      </Button>
                    </Space>
                  </Card>

                  {renderAlert(webResearchAlert)}
                </Space>
              ),
            },
            {
              key: 'presets',
              label: '配置预设',
              children: (
                <Space direction="vertical" size={16} style={{ width: '100%' }}>
                  <Card
                    variant="borderless"
                    style={{
                      borderRadius: 20,
                      background: quietPanelBackground,
                      border: `1px solid ${panelBorder}`,
                    }}
                  >
                    <Space direction="vertical" size={4} style={{ width: '100%', marginBottom: 16 }}>
                      <Text style={{ fontSize: 11, letterSpacing: '0.18em', textTransform: 'uppercase', color: token.colorTextTertiary }}>
                        Reusable Profiles
                      </Text>
                      <Title level={isMobile ? 4 : 3} style={{ margin: 0, fontFamily: designDisplayFont, letterSpacing: '-0.03em' }}>
                        API 预设
                      </Title>
                      <Text type="secondary">
                        预设更像“可切换的工作配置”。适合按模型用途、成本档位、写作阶段来拆分，而不是把所有参数塞进一个默认配置里。
                      </Text>
                    </Space>
                    <Space wrap>
                      <Button type="primary" icon={<PlusOutlined />} onClick={openCreatePreset}>
                        新建预设
                      </Button>
                      <Button icon={<CopyOutlined />} onClick={() => { snapshotForm.resetFields(); setSnapshotModalOpen(true); }}>
                        从当前配置创建预设
                      </Button>
                      <Button icon={<ReloadOutlined />} loading={loadingPresets} onClick={() => void loadPresets()}>
                        刷新预设
                      </Button>
                    </Space>
                  </Card>

                  <Card
                    variant="borderless"
                    style={{
                      borderRadius: 20,
                      background: quietPanelBackground,
                      border: `1px solid ${panelBorder}`,
                    }}
                  >
                    {loadingPresets ? (
                      <InlineDeferredPanel
                        eyebrow="Preset Library"
                        title="正在整理 API 预设列表"
                        message="系统正在恢复预设档案、激活入口与测试操作，原有预设读取、切换和删除逻辑保持不变。"
                        minHeight={260}
                        tags={[
                          { label: 'API 预设', color: 'purple' },
                          { label: '预设档案恢复中', color: 'processing' },
                          { label: '切换逻辑保持原样', color: 'green' },
                        ]}
                      />
                    ) : presets.length === 0 ? (
                      <Empty description="暂无 API 预设" />
                    ) : (
                      <List
                        itemLayout="vertical"
                        dataSource={presets}
                        renderItem={(preset) => (
                          <List.Item
                            key={preset.id}
                            style={{
                              borderRadius: 18,
                              padding: '16px 18px',
                              marginBottom: 12,
                              background: token.colorBgContainer,
                              border: `1px solid ${token.colorBorderSecondary}`,
                            }}
                            actions={[
                              <Button key="activate" type={preset.is_active ? 'default' : 'link'} icon={<CheckCircleOutlined />} onClick={() => void handleActivatePreset(preset.id)}>
                                {preset.is_active ? '当前启用' : '激活'}
                              </Button>,
                              <Button key="test" type="link" icon={<PlayCircleOutlined />} onClick={() => void handleTestPreset(preset.id)}>
                                测试
                              </Button>,
                              <Button key="edit" type="link" icon={<EditOutlined />} onClick={() => openEditPreset(preset)}>
                                编辑
                              </Button>,
                              <Popconfirm
                                key="delete"
                                title="确认删除这个预设吗？"
                                description="激活中的预设不能直接删除。"
                                onConfirm={() => void handleDeletePreset(preset.id)}
                              >
                                <Button danger type="link" icon={<DeleteOutlined />}>删除</Button>
                              </Popconfirm>,
                            ]}
                          >
                            <Space direction="vertical" size={4} style={{ width: '100%' }}>
                              <Space wrap>
                                <Text strong>{preset.name}</Text>
                                {preset.is_active ? <Tag color="success">已激活</Tag> : null}
                                <Tag color="blue">{preset.config.api_provider}</Tag>
                                <Tag>{preset.config.llm_model}</Tag>
                              </Space>
                              {preset.description ? <Paragraph style={{ marginBottom: 0 }}>{preset.description}</Paragraph> : null}
                              <Text type="secondary">Base URL：{preset.config.api_base_url || '未设置'} · Temperature：{preset.config.temperature} · Max Tokens：{preset.config.max_tokens}</Text>
                            </Space>
                          </List.Item>
                        )}
                      />
                    )}
                  </Card>
                </Space>
              ),
            },
          ]}
        />
        </Card>
      )}

      <Modal
        title={(
          <Space direction="vertical" size={2}>
            <Text style={{ fontSize: 11, letterSpacing: '0.18em', textTransform: 'uppercase', color: token.colorTextTertiary }}>
              Preset Editor
            </Text>
            <Title level={4} style={{ margin: 0, fontFamily: designDisplayFont, letterSpacing: '-0.03em' }}>
              {editingPreset ? '编辑预设' : '新建预设'}
            </Title>
            <Text type="secondary">
              为一组可重复使用的模型参数建立清晰说明，方便按创作阶段、成本档位和模型用途切换。
            </Text>
          </Space>
        )}
        open={presetModalOpen}
        onCancel={() => setPresetModalOpen(false)}
        onOk={() => void handleSubmitPreset()}
        okText={editingPreset ? '保存修改' : '创建预设'}
        confirmLoading={submittingPreset}
        width={720}
        styles={modalSurfaceStyles}
        destroyOnClose
      >
        <Alert
          type="info"
          showIcon
          style={{ marginBottom: 16, borderRadius: 14 }}
          message="建议把一个预设定义为一种稳定工作模式，而不是临时参数草稿。"
        />
        <Form form={presetForm} layout="vertical">
          <Row gutter={[16, 0]}>
            <Col xs={24} md={12}>
              <Form.Item label="预设名称" name="name" rules={[{ required: true, message: '请输入预设名称' }]}>
                <Input placeholder="例如：DeepSeek 主线写作" />
              </Form.Item>
            </Col>
            <Col xs={24} md={12}>
              <Form.Item label="提供商" name="api_provider" rules={[{ required: true, message: '请选择提供商' }]}>
                <Select options={providerOptions as unknown as { label: string; value: string }[]} />
              </Form.Item>
            </Col>
            <Col span={24}>
              <Form.Item label="描述" name="description">
                <Input placeholder="给这个预设补充说明" />
              </Form.Item>
            </Col>
            <Col span={24}>
              <Form.Item label="API Key" name="api_key" rules={[{ required: true, message: '预设必须包含真实 API Key' }]}>
                <Input.Password placeholder="请输入真实 API Key" autoComplete="new-password" />
              </Form.Item>
            </Col>
            <Col span={24}>
              <Form.Item label="API Base URL" name="api_base_url" rules={[{ required: true, message: '请输入 API Base URL' }]}>
                <Input placeholder="https://api.openai.com/v1" />
              </Form.Item>
            </Col>
            <Col span={24}>
              <Form.Item label="备用 Base URL（每行一个，可选）" name="api_backup_urls_text">
                <TextArea rows={3} />
              </Form.Item>
            </Col>
            <Col xs={24} md={12}>
              <Form.Item label="模型" name="llm_model" rules={[{ required: true, message: '请输入模型名' }]}>
                <Input placeholder="deepseek-chat / claude-3-5-sonnet-latest / gemini-2.5-pro" />
              </Form.Item>
            </Col>
            <Col xs={24} md={12}>
              <Form.Item label="回退策略" name="fallback_strategy">
                <Select options={[{ value: 'auto', label: '自动回退' }, { value: 'manual', label: '手动切换' }]} />
              </Form.Item>
            </Col>
            <Col xs={24} md={12}>
              <Form.Item label="Temperature" name="temperature">
                <InputNumber min={0} max={2} step={0.1} style={{ width: '100%' }} />
              </Form.Item>
            </Col>
            <Col xs={24} md={12}>
              <Form.Item label="Max Tokens" name="max_tokens">
                <InputNumber min={1} max={200000} step={256} style={{ width: '100%' }} />
              </Form.Item>
            </Col>
            <Col span={24}>
              <Form.Item label="System Prompt（可选）" name="system_prompt">
                <TextArea rows={4} />
              </Form.Item>
            </Col>
          </Row>
        </Form>
      </Modal>

      <Modal
        title={(
          <Space direction="vertical" size={2}>
            <Text style={{ fontSize: 11, letterSpacing: '0.18em', textTransform: 'uppercase', color: token.colorTextTertiary }}>
              Snapshot Draft
            </Text>
            <Title level={4} style={{ margin: 0, fontFamily: designDisplayFont, letterSpacing: '-0.03em' }}>
              从当前配置创建预设
            </Title>
            <Text type="secondary">
              把当前已验证的设置保存为可复用快照，适合在“可用状态”稳定后再执行这一步。
            </Text>
          </Space>
        )}
        open={snapshotModalOpen}
        onCancel={() => setSnapshotModalOpen(false)}
        onOk={() => void handleCreateSnapshotPreset()}
        okText="创建预设"
        confirmLoading={creatingSnapshot}
        styles={modalSurfaceStyles}
        destroyOnClose
      >
        <Alert
          type="info"
          showIcon
          style={{ marginBottom: 16 }}
          message="此操作会把当前已保存配置复制为新的预设。"
        />
        <Form form={snapshotForm} layout="vertical">
          <Form.Item label="预设名称" name="name" rules={[{ required: true, message: '请输入预设名称' }]}>
            <Input placeholder="例如：当前生产配置" />
          </Form.Item>
          <Form.Item label="描述" name="description">
            <Input placeholder="可选备注" />
          </Form.Item>
        </Form>
      </Modal>
    </div>
  );
}
