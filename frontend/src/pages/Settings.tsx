import { Suspense, lazy, useEffect, useState } from 'react';
import { Card, Form, message, Space, Typography, Spin, Modal, Alert, Grid, Tabs, Tag, Row, Col } from 'antd';
import { WarningOutlined } from '@ant-design/icons';
import { settingsApi, mcpPluginApi } from '../services/api';
import type { SettingsUpdate, APIKeyPreset, PresetCreateRequest, APIKeyPresetConfig } from '../types';
import { eventBus, EventNames } from '../store/eventBus';
import { hasUsableApiCredentials, isPlaceholderApiKey } from '../utils/apiKey';

const { Title, Text } = Typography;
const { useBreakpoint } = Grid;

type ModelOption = { value: string; label: string; description: string };
type SettingsSectionKey = 'provider' | 'network' | 'model' | 'research';

const buildModelSelectOptions = (
  options: ModelOption[],
  searchText: string
): ModelOption[] => {
  const keyword = searchText.trim();
  if (!keyword) return options;

  const exists = options.some(
    (item) => item.value.toLowerCase() === keyword.toLowerCase()
  );
  if (exists) return options;

  return [
    {
      value: keyword,
      label: `${keyword} (自定义输入)`,
      description: '手动输入的模型名称',
    },
    ...options,
  ];
};

const LazyProviderSelector = lazy(() => import('../components/ProviderSelector'));
const LazyEndpointListEditor = lazy(() => import('../components/EndpointListEditor'));
const LazyAzureConfigGuide = lazy(() => import('../components/AzureConfigGuide'));
const LazySettingsPresetModal = lazy(() => import('../components/SettingsPresetModal'));
const LazySettingsPresetsTab = lazy(() => import('../components/SettingsPresetsTab'));
const LazySettingsCurrentTab = lazy(() => import('../components/SettingsCurrentTab'));

const settingsLazyFallback = (
  <div style={{ padding: '12px 0', textAlign: 'center' }}>
    <Spin size="small" />
  </div>
);

export default function SettingsPage() {
  const screens = useBreakpoint();
  const isMobile = !screens.md; // md断点是768px
  const [form] = Form.useForm();
  const [modal, contextHolder] = Modal.useModal();
  const [loading, setLoading] = useState(false);
  const [initialLoading, setInitialLoading] = useState(true);
  const [hasSettings, setHasSettings] = useState(false);
  const [isDefaultSettings, setIsDefaultSettings] = useState(false);
  const [modelOptions, setModelOptions] = useState<ModelOption[]>([]);
  const [modelSearchText, setModelSearchText] = useState('');
  const [fetchingModels, setFetchingModels] = useState(false);
  const [modelsFetched, setModelsFetched] = useState(false);
  const [testingApi, setTestingApi] = useState(false);
  const [testResult, setTestResult] = useState<{
    success: boolean;
    message: string;
    response_time_ms?: number;
    response_preview?: string;
    error?: string;
    error_type?: string;
    suggestions?: string[];
  } | null>(null);
  const [showTestResult, setShowTestResult] = useState(false);
  const [testingWebResearchProvider, setTestingWebResearchProvider] = useState<'exa' | 'grok' | null>(null);
  const [webResearchTestResult, setWebResearchTestResult] = useState<{
    success: boolean;
    provider: string;
    message: string;
    response_preview?: string;
    result_count?: number;
    source_count?: number;
    error?: string;
    error_type?: string;
    suggestions?: string[];
  } | null>(null);

  // 预设相关状态
  const [activeTab, setActiveTab] = useState('current');
  const [activeSettingsSection, setActiveSettingsSection] = useState<SettingsSectionKey>('provider');
  const [presets, setPresets] = useState<APIKeyPreset[]>([]);
  const [presetsLoading, setPresetsLoading] = useState(false);
  const [activePresetId, setActivePresetId] = useState<string | undefined>();
  const [editingPreset, setEditingPreset] = useState<APIKeyPreset | null>(null);
  const [isPresetModalVisible, setIsPresetModalVisible] = useState(false);
  const [testingPresetId, setTestingPresetId] = useState<string | null>(null);
  const [presetForm] = Form.useForm();
  
  // 预设编辑窗口的模型列表状态（独立于当前配置的模型列表）
  const [presetModelOptions, setPresetModelOptions] = useState<ModelOption[]>([]);
  const [presetModelSearchText, setPresetModelSearchText] = useState('');
  const [fetchingPresetModels, setFetchingPresetModels] = useState(false);
  const [presetModelsFetched, setPresetModelsFetched] = useState(false);

  // API 兼容性相关状态
  const [selectedProvider, setSelectedProvider] = useState('openai');
  const [endpoints, setEndpoints] = useState<Array<{
    url: string;
    type: 'primary' | 'fallback';
    status?: 'success' | 'error' | 'pending' | 'untested';
  }>>([]);
  const [fallbackStrategy, setFallbackStrategy] = useState<'auto' | 'manual'>('auto');

  const watchedProvider = Form.useWatch('api_provider', form) || selectedProvider;
  const watchedModel = Form.useWatch('llm_model', form) || '未设置';
  const watchedBaseUrl = Form.useWatch('api_base_url', form) || '未设置';
  const watchedTemperature = Form.useWatch('temperature', form) ?? 0.7;
  const watchedMaxTokens = Form.useWatch('max_tokens', form) ?? '未设置';
  const watchedWebResearchEnabled = Boolean(Form.useWatch('web_research_enabled', form));
  const watchedExaEnabled = Form.useWatch('web_research_exa_enabled', form) !== false;
  const watchedGrokEnabled = Form.useWatch('web_research_grok_enabled', form) !== false;

  const clipDisplayText = (value: string, limit = isMobile ? 20 : 32) => {
    const normalized = value.trim();
    if (!normalized) return '未设置';
    return normalized.length <= limit ? normalized : `${normalized.slice(0, limit - 1)}…`;
  };

  const sectionCardStyle = {
    marginBottom: isMobile ? 16 : 20,
    borderRadius: isMobile ? 14 : 18,
    border: '1px solid #edf2f7',
    boxShadow: '0 10px 30px rgba(15, 23, 42, 0.05)',
    background: 'linear-gradient(180deg, #ffffff 0%, #fbfdff 100%)',
    overflow: 'hidden' as const,
  };

  const sectionCardStyles = {
    header: {
      padding: isMobile ? '14px 16px' : '16px 20px',
      borderBottom: '1px solid #f1f5f9',
      background: 'rgba(248, 250, 252, 0.9)',
    },
    body: {
      padding: isMobile ? 16 : 20,
    },
  };

  const fieldPanelStyle = {
    padding: isMobile ? 14 : 16,
    borderRadius: 14,
    border: '1px solid #eef2f7',
    background: 'linear-gradient(180deg, #ffffff 0%, #f8fbff 100%)',
    height: '100%',
  };

  const fieldHintTextStyle = {
    display: 'block',
    marginBottom: 12,
    color: 'var(--color-text-secondary)',
    fontSize: isMobile ? 12 : 13,
    lineHeight: 1.65,
  };

  const renderSectionTitle = (title: string, description: string, tagLabel: string, tagColor: string) => (
    <Space direction="vertical" size={2} style={{ width: '100%' }}>
      <Space wrap size={8}>
        <Text strong style={{ fontSize: isMobile ? 15 : 16 }}>{title}</Text>
        <Tag color={tagColor} style={{ marginInlineEnd: 0 }}>{tagLabel}</Tag>
      </Space>
      <Text style={{ fontSize: isMobile ? 12 : 13, color: 'var(--color-text-secondary)' }}>
        {description}
      </Text>
    </Space>
  );

  const settingsSectionItems: Array<{
    key: SettingsSectionKey;
    label: string;
    description: string;
    summary: string;
  }> = [
    {
      key: 'provider',
      label: '基础接入',
      description: '选择 API 提供商，并填写 Key 与基础地址。',
      summary: String(watchedProvider).toUpperCase(),
    },
    {
      key: 'network',
      label: '网络容灾',
      description: '维护主备端点与切换策略，提升稳定性。',
      summary: `${Math.max(endpoints.length, watchedBaseUrl === '未设置' ? 0 : 1)} 个端点 / ${fallbackStrategy === 'auto' ? '自动降级' : '手动切换'}`,
    },
    {
      key: 'model',
      label: '生成参数',
      description: '配置模型、温度、Token 与系统提示词。',
      summary: `${clipDisplayText(String(watchedModel), isMobile ? 12 : 18)} / Token ${String(watchedMaxTokens)}`,
    },
    {
      key: 'research',
      label: '联网检索',
      description: '分别配置 Exa 与 Grok 的检索增强能力。',
      summary: watchedWebResearchEnabled ? `Exa ${watchedExaEnabled ? '开' : '关'} / Grok ${watchedGrokEnabled ? '开' : '关'}` : '总开关关闭',
    },
  ];

  const activeSettingsSectionMeta =
    settingsSectionItems.find((item) => item.key === activeSettingsSection) || settingsSectionItems[0];

  useEffect(() => {
    loadSettings();
    if (activeTab === 'presets') {
      loadPresets();
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    if (activeTab === 'presets') {
      loadPresets();
    } else if (activeTab === 'current') {
      // 切换到当前配置Tab时，刷新设置以获取最新数据
      loadSettings();
      // 清除旧的测试结果，因为可能是其他配置的测试结果
      setTestResult(null);
      setShowTestResult(false);
      setWebResearchTestResult(null);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [activeTab]);

  const loadSettings = async () => {
    setInitialLoading(true);
    try {
      const settings = await settingsApi.getSettings();
      form.setFieldsValue(settings);

      // 初始化 API 兼容性相关状态
      setSelectedProvider(settings.provider_type || settings.api_provider || 'openai');
      setFallbackStrategy(settings.fallback_strategy || 'auto');
      // 构建端点列表：主端点 + 备端点
      const endpointList: Array<{ url: string; type: 'primary' | 'fallback'; status?: 'success' | 'error' | 'pending' | 'untested' }> = [];
      if (settings.api_base_url) {
        endpointList.push({ url: settings.api_base_url, type: 'primary', status: 'untested' });
      }
      if (settings.api_backup_urls && settings.api_backup_urls.length > 0) {
        settings.api_backup_urls.forEach(url => {
          endpointList.push({ url, type: 'fallback', status: 'untested' });
        });
      }
      setEndpoints(endpointList);

      // 判断是否为默认设置（id='0'表示来自.env的默认配置）
      if (settings.id === '0' || !settings.id) {
        setIsDefaultSettings(true);
        setHasSettings(false);
      } else {
        setIsDefaultSettings(false);
        setHasSettings(true);
      }
    } catch (error) {
      // 如果404表示还没有设置，使用默认值
      if ((error as { response?: { status?: number } } | undefined)?.response?.status === 404) {
        setHasSettings(false);
        setIsDefaultSettings(true);
        form.setFieldsValue({
          api_provider: 'openai',
          api_base_url: 'https://api.openai.com/v1',
          llm_model: 'gpt-4',
          temperature: 0.7,
          max_tokens: 2000,
          web_research_enabled: false,
          web_research_exa_enabled: true,
          web_research_grok_enabled: true,
          web_research_exa_base_url: '',
          web_research_grok_model: 'grok-4.1-fast',
        });
      } else {
        message.error('加载设置失败');
      }
    } finally {
      setInitialLoading(false);
    }
  };

  const handleSave = async (values: SettingsUpdate) => {
    setLoading(true);
    try {
      // 注入 API 兼容性字段
      const saveData: SettingsUpdate = {
        ...values,
        provider_type: selectedProvider,
        fallback_strategy: fallbackStrategy,
        api_backup_urls: endpoints.filter(e => e.type === 'fallback').map(e => e.url).filter(Boolean),
      };
      // 如果主端点列表有值，同步 api_base_url
      const primaryEndpoint = endpoints.find(e => e.type === 'primary');
      if (primaryEndpoint?.url) {
        saveData.api_base_url = primaryEndpoint.url;
      }

      // 检查是否与 MCP 缓存的配置不一致
      const verifiedConfigStr = localStorage.getItem('mcp_verified_config');
      let configChanged = false;
      
      if (verifiedConfigStr) {
        try {
          const verifiedConfig = JSON.parse(verifiedConfigStr);
          configChanged =
            verifiedConfig.provider !== saveData.api_provider ||
            verifiedConfig.baseUrl !== saveData.api_base_url ||
            verifiedConfig.model !== saveData.llm_model;
        } catch (e) {
          console.error('Failed to parse verified config:', e);
        }
      }
      
      await settingsApi.saveSettings(saveData);
      message.success('设置已保存');
      setHasSettings(true);
      setIsDefaultSettings(false);
      
      // 保存后清除测试结果，因为配置可能已变更
      setTestResult(null);
      setShowTestResult(false);
      setWebResearchTestResult(null);
      
      // 手动保存配置后，需要同步更新预设激活状态
      // 因为用户手动修改的配置可能与之前激活的预设不一致了
      // 重新加载预设列表以确保状态正确（后端在save时会自动取消激活状态）
      if (activePresetId) {
        // 检查当前保存的配置是否与激活预设一致
        const activePreset = presets.find(p => p.id === activePresetId);
        if (activePreset) {
          const presetConfig = activePreset.config;
          const configMismatch =
            presetConfig.api_provider !== saveData.api_provider ||
            presetConfig.api_key !== saveData.api_key ||
            presetConfig.api_base_url !== saveData.api_base_url ||
            presetConfig.llm_model !== saveData.llm_model ||
            presetConfig.temperature !== saveData.temperature ||
            presetConfig.max_tokens !== saveData.max_tokens;
          
          if (configMismatch) {
            // 配置已变更，清除前端的激活状态标记
            setActivePresetId(undefined);
            message.info('配置已更改，预设激活状态已取消');
            // 刷新预设列表以同步后端取消激活的状态
            loadPresets();
          }
        }
      }
      
      // 如果配置发生变化，需要处理 MCP 插件
      if (configChanged) {
        // 清除 MCP 验证缓存
        localStorage.removeItem('mcp_verified_config');
        
        // 检查并禁用所有 MCP 插件
        try {
          const plugins = await mcpPluginApi.getPlugins();
          const activePlugins = plugins.filter(p => p.enabled);
          
          if (activePlugins.length > 0) {
            // 禁用所有插件
            message.loading({ content: '正在禁用 MCP 插件...', key: 'disable_mcp' });
            await Promise.all(activePlugins.map(p => mcpPluginApi.togglePlugin(p.id, false)));
            message.success({ content: '已禁用所有 MCP 插件', key: 'disable_mcp' });
            
            // 显示提示弹窗
            modal.warning({
              title: (
                <Space>
                  <WarningOutlined style={{ color: '#faad14' }} />
                  <span>API 配置已更改</span>
                </Space>
              ),
              centered: true,
              content: (
                <div style={{ padding: '8px 0' }}>
                  <Alert
                    message="检测到您修改了 API 配置（提供商、地址或模型），为确保 MCP 插件正常工作，系统已自动禁用所有插件。"
                    type="warning"
                    showIcon
                    style={{ marginBottom: 16 }}
                  />
                  <div style={{
                    padding: 12,
                    background: 'var(--color-info-bg)',
                    border: '1px solid var(--color-info-border)',
                    borderRadius: 8
                  }}>
                    <Text strong style={{ display: 'block', marginBottom: 8 }}>请完成以下步骤：</Text>
                    <ol style={{ margin: 0, paddingLeft: 20, fontSize: 13 }}>
                      <li>前往 MCP 插件管理页面</li>
                      <li>重新进行"模型能力检查"</li>
                      <li>确认新模型支持 Function Calling 后再启用插件</li>
                    </ol>
                  </div>
                </div>
              ),
              okText: '前往 MCP 页面',
              cancelText: '稍后处理',
              onOk: () => {
                eventBus.emit(EventNames.SWITCH_TO_MCP_VIEW);
              },
            });
          }
        } catch (err) {
          console.error('Failed to disable MCP plugins:', err);
        }
      }
    } catch {
      message.error('保存设置失败');
    } finally {
      setLoading(false);
    }
  };

  const handleReset = () => {
    modal.confirm({
      title: '重置设置',
      content: '确定要重置为默认值吗？',
      centered: true,
      okText: '确定',
      cancelText: '取消',
      onOk: () => {
        form.setFieldsValue({
          api_provider: 'openai',
          api_key: '',
          api_base_url: 'https://api.openai.com/v1',
          llm_model: 'gpt-4',
          temperature: 0.7,
          max_tokens: 2000,
          web_research_enabled: false,
          web_research_exa_enabled: true,
          web_research_grok_enabled: true,
          web_research_exa_api_key: '',
          web_research_exa_base_url: '',
          web_research_grok_api_key: '',
          web_research_grok_base_url: '',
          web_research_grok_model: 'grok-4.1-fast',
        });
        setSelectedProvider('openai');
        setEndpoints([{ url: 'https://api.openai.com/v1', type: 'primary', status: 'untested' }]);
        setFallbackStrategy('auto');
        setWebResearchTestResult(null);
        message.info('已重置为默认值，请点击保存');
      },
    });
  };

  const handleDelete = () => {
    modal.confirm({
      title: '删除设置',
      content: '确定要删除所有设置吗？此操作不可恢复。',
      centered: true,
      okText: '确定',
      cancelText: '取消',
      okType: 'danger',
      onOk: async () => {
        setLoading(true);
        try {
          await settingsApi.deleteSettings();
          message.success('设置已删除');
          setHasSettings(false);
          form.resetFields();
        } catch {
          message.error('删除设置失败');
        } finally {
          setLoading(false);
        }
      },
    });
  };

  const handleProviderChange = (value: string) => {
    setSelectedProvider(value);
    // 对于 openai 兼容族，统一走 openai 的默认 URL
    const providerDefaultUrls: Record<string, string> = {
      openai: 'https://api.openai.com/v1',
      openai_responses: 'https://api.openai.com/v1',
      anthropic: 'https://api.anthropic.com',
      newapi: '',
      azure: '',
      custom: '',
      sub2api: 'https://ai.qaq.al',
      gemini: 'https://generativelanguage.googleapis.com/v1beta',
    };
    const defaultUrl = providerDefaultUrls[value] || '';
    if (defaultUrl) {
      form.setFieldValue('api_base_url', defaultUrl);
      // 同步更新端点列表的主端点
      setEndpoints(prev => {
        if (prev.length === 0) return [{ url: defaultUrl, type: 'primary', status: 'untested' as const }];
        const updated = [...prev];
        updated[0] = { ...updated[0], url: defaultUrl, status: 'untested' as const };
        return updated;
      });
    }
    form.setFieldValue('provider_type', value);
    // 清空模型列表，需要重新获取
    setModelOptions([]);
    setModelSearchText('');
    setModelsFetched(false);
  };

  const handleFetchModels = async (silent: boolean = false) => {
    const apiKey = form.getFieldValue('api_key');
    const apiBaseUrl = form.getFieldValue('api_base_url');
    const provider = form.getFieldValue('api_provider');

    if (!hasUsableApiCredentials(apiKey, apiBaseUrl)) {
      if (!silent) {
        message.warning(
          isPlaceholderApiKey(apiKey)
            ? '当前 API 密钥仍为示例占位值，请先填写真实密钥'
            : '请先填写 API 密钥和 API 地址'
        );
      }
      return;
    }

    setFetchingModels(true);
    try {
      const response = await settingsApi.getAvailableModels({
        api_key: apiKey,
        api_base_url: apiBaseUrl,
        provider: provider || 'openai'
      });

      setModelOptions(response.models);
      setModelsFetched(true);
      if (!silent) {
        message.success(`成功获取 ${response.count || response.models.length} 个可用模型`);
      }
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    } catch (error: any) {
      const errorMsg = error?.response?.data?.detail || '获取模型列表失败';
      if (!silent) {
        message.error(errorMsg);
      }
      setModelOptions([]);
      setModelsFetched(true); // 即使失败也标记为已尝试，避免重复请求
    } finally {
      setFetchingModels(false);
    }
  };

  const handleModelSelectFocus = () => {
    // 如果还没有获取过模型列表，自动获取
    if (!modelsFetched && !fetchingModels) {
      handleFetchModels(true); // silent模式，不显示成功消息
    }
  };

  const handleTestConnection = async () => {
    const apiKey = form.getFieldValue('api_key');
    const apiBaseUrl = form.getFieldValue('api_base_url');
    const provider = form.getFieldValue('api_provider');
    const modelName = form.getFieldValue('llm_model');
    const temperature = form.getFieldValue('temperature');
    const maxTokens = form.getFieldValue('max_tokens');

    if (!apiKey || !apiBaseUrl || !provider || !modelName) {
      message.warning('请先填写完整的配置信息');
      return;
    }

    setTestingApi(true);
    setTestResult(null);

    try {
      const result = await settingsApi.testApiConnection({
        api_key: apiKey,
        api_base_url: apiBaseUrl,
        provider: provider,
        llm_model: modelName,
        temperature: temperature,
        max_tokens: maxTokens
      });

      setTestResult(result);
      setShowTestResult(true);

      if (result.success) {
        message.success(`测试成功！响应时间: ${result.response_time_ms}ms`);
      } else {
        message.error('API 测试失败，请查看详细信息');
      }
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    } catch (error: any) {
      const errorMsg = error?.response?.data?.detail || '测试请求失败';
      message.error(errorMsg);
      setTestResult({
        success: false,
        message: '测试请求失败',
        error: errorMsg,
        error_type: 'RequestError',
        suggestions: ['请检查网络连接', '请确认后端服务是否正常运行']
      });
      setShowTestResult(true);
    } finally {
      setTestingApi(false);
    }
  };

  const handleTestWebResearch = async (provider: 'exa' | 'grok') => {
    const exaApiKey = form.getFieldValue('web_research_exa_api_key');
    const exaBaseUrl = form.getFieldValue('web_research_exa_base_url');
    const grokApiKey = form.getFieldValue('web_research_grok_api_key');
    const grokBaseUrl = form.getFieldValue('web_research_grok_base_url');
    const grokModel = form.getFieldValue('web_research_grok_model');

    if (provider === 'exa' && !exaApiKey) {
      message.warning('请先填写 Exa API Key');
      return;
    }
    if (provider === 'grok' && (!grokApiKey || !grokBaseUrl)) {
      message.warning('请先填写 Grok API Key 和 Base URL');
      return;
    }

    setTestingWebResearchProvider(provider);
    setWebResearchTestResult(null);
    try {
      const result = await settingsApi.testWebResearchConnection({
        provider,
        exa_api_key: exaApiKey,
        exa_base_url: exaBaseUrl,
        grok_api_key: grokApiKey,
        grok_base_url: grokBaseUrl,
        grok_model: grokModel,
      });
      setWebResearchTestResult(result);
      if (result.success) {
        message.success(`${provider.toUpperCase()} 检索测试成功`);
      } else {
        message.error(`${provider.toUpperCase()} 检索测试失败`);
      }
    } catch (error) {
      const errorMsg = (
        error as { response?: { data?: { detail?: string } } } | undefined
      )?.response?.data?.detail || '????????';
      setWebResearchTestResult({
        success: false,
        provider,
        message: `${provider.toUpperCase()} 检索测试失败`,
        error: errorMsg,
        error_type: 'RequestError',
        suggestions: ['请检查后端服务是否正常', '请检查网络和技能目录配置'],
      });
      message.error(errorMsg);
    } finally {
      setTestingWebResearchProvider(null);
    }
  };

  // ========== 预设管理函数 ==========

  const loadPresets = async () => {
    setPresetsLoading(true);
    try {
      const response = await settingsApi.getPresets();
      setPresets(response.presets);
      setActivePresetId(response.active_preset_id);
    } catch (error) {
      message.error('加载预设失败');
      console.error(error);
    } finally {
      setPresetsLoading(false);
    }
  };

  const showPresetModal = (preset?: APIKeyPreset) => {
    // 重置预设模型列表状态
    setPresetModelOptions([]);
    setPresetModelSearchText('');
    setPresetModelsFetched(false);
    
    if (preset) {
      setEditingPreset(preset);
      presetForm.setFieldsValue({
        name: preset.name,
        description: preset.description,
        ...preset.config,
      });
    } else {
      setEditingPreset(null);
      presetForm.resetFields();
      presetForm.setFieldsValue({
        api_provider: 'openai',
        api_base_url: 'https://api.openai.com/v1',
        temperature: 0.7,
        max_tokens: 2000,
      });
    }
    setIsPresetModalVisible(true);
  };

  const handlePresetCancel = () => {
    setIsPresetModalVisible(false);
    setEditingPreset(null);
    presetForm.resetFields();
    // 清除预设模型列表状态
    setPresetModelOptions([]);
    setPresetModelSearchText('');
    setPresetModelsFetched(false);
  };

  // 预设编辑窗口：获取模型列表
  const handleFetchPresetModels = async (silent: boolean = false) => {
    const apiKey = presetForm.getFieldValue('api_key');
    const apiBaseUrl = presetForm.getFieldValue('api_base_url');
    const provider = presetForm.getFieldValue('api_provider');

    if (!hasUsableApiCredentials(apiKey, apiBaseUrl)) {
      if (!silent) {
        message.warning(
          isPlaceholderApiKey(apiKey)
            ? '当前 API 密钥仍为示例占位值，请先填写真实密钥'
            : '请先填写 API 密钥和 API 地址'
        );
      }
      return;
    }

    setFetchingPresetModels(true);
    try {
      const response = await settingsApi.getAvailableModels({
        api_key: apiKey,
        api_base_url: apiBaseUrl,
        provider: provider || 'openai'
      });

      setPresetModelOptions(response.models);
      setPresetModelsFetched(true);
      if (!silent) {
        message.success(`成功获取 ${response.count || response.models.length} 个可用模型`);
      }
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    } catch (error: any) {
      const errorMsg = error?.response?.data?.detail || '获取模型列表失败';
      if (!silent) {
        message.error(errorMsg);
      }
      setPresetModelOptions([]);
      setPresetModelsFetched(true);
    } finally {
      setFetchingPresetModels(false);
    }
  };

  // 预设编辑窗口：模型选择框获得焦点时自动获取
  const handlePresetModelSelectFocus = () => {
    if (!presetModelsFetched && !fetchingPresetModels) {
      handleFetchPresetModels(true);
    }
  };

  const handlePresetModelSearchChange = (value: string) => {
    setPresetModelSearchText(value);
  };

  const handlePresetModelChange = () => {
    setPresetModelSearchText('');
  };

  const handlePresetModelCommit = () => {
    const customModel = presetModelSearchText.trim();
    if (customModel) {
      presetForm.setFieldValue('llm_model', customModel);
    }
  };

  const handlePresetModelReload = () => {
    if (!fetchingPresetModels) {
      setPresetModelsFetched(false);
      handleFetchPresetModels(false);
    }
  };

  // 预设编辑窗口：提供商变更时更新默认URL并清空模型列表
  const handlePresetProviderChange = (value: string) => {
    const providerDefaultUrls: Record<string, string> = {
      openai: 'https://api.openai.com/v1',
      openai_responses: 'https://api.openai.com/v1',
      anthropic: 'https://api.anthropic.com',
      sub2api: 'https://ai.qaq.al',
      gemini: 'https://generativelanguage.googleapis.com/v1beta',
    };
    const defaultUrl = providerDefaultUrls[value];
    if (defaultUrl) {
      presetForm.setFieldValue('api_base_url', defaultUrl);
    }
    presetForm.setFieldValue('provider_type', value);
    // 清空模型列表，需要重新获取
    setPresetModelOptions([]);
    setPresetModelSearchText('');
    setPresetModelsFetched(false);
  };

  const handlePresetSave = async () => {
    try {
      const values = await presetForm.validateFields();
      const config: APIKeyPresetConfig = {
        api_provider: values.api_provider,
        api_key: values.api_key,
        api_base_url: values.api_base_url,
        llm_model: values.llm_model,
        temperature: values.temperature,
        max_tokens: values.max_tokens,
        provider_type: values.api_provider,
        api_backup_urls: values.api_backup_urls || [],
        fallback_strategy: values.fallback_strategy || 'auto',
        azure_api_version: values.azure_api_version,
      };

      if (editingPreset) {
        await settingsApi.updatePreset(editingPreset.id, {
          name: values.name,
          description: values.description,
          config,
        });
        message.success('预设已更新');
      } else {
        const request: PresetCreateRequest = {
          name: values.name,
          description: values.description,
          config,
        };
        await settingsApi.createPreset(request);
        message.success('预设已创建');
      }

      handlePresetCancel();
      loadPresets();
    } catch (error) {
      console.error('保存失败:', error);
    }
  };

  const handlePresetDelete = async (presetId: string) => {
    try {
      await settingsApi.deletePreset(presetId);
      message.success('预设已删除');
      loadPresets();
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    } catch (error: any) {
      message.error(error.response?.data?.detail || '删除失败');
      console.error(error);
    }
  };

  const handlePresetActivate = async (presetId: string, presetName: string) => {
    try {
      // 获取预设配置用于比较
      const preset = presets.find(p => p.id === presetId);
      
      await settingsApi.activatePreset(presetId);
      message.success(`已激活预设: ${presetName}`);
      
      // 激活预设后清除当前配置Tab的测试结果
      setTestResult(null);
      setShowTestResult(false);
      
      // 清除模型列表缓存，因为API配置可能已变更
      setModelOptions([]);
      setModelsFetched(false);
      
      loadPresets();
      loadSettings(); // 重新加载当前配置
      
      // 检查是否与 MCP 缓存的配置不一致
      if (preset) {
        const verifiedConfigStr = localStorage.getItem('mcp_verified_config');
        let configChanged = false;
        
        if (verifiedConfigStr) {
          try {
            const verifiedConfig = JSON.parse(verifiedConfigStr);
            configChanged =
              verifiedConfig.provider !== preset.config.api_provider ||
              verifiedConfig.baseUrl !== preset.config.api_base_url ||
              verifiedConfig.model !== preset.config.llm_model;
          } catch (e) {
            console.error('Failed to parse verified config:', e);
            configChanged = true; // 解析失败也视为配置变化
          }
        } else {
          // 没有缓存的配置，如果有启用的插件也需要处理
          configChanged = true;
        }
        
        if (configChanged) {
          // 清除 MCP 验证缓存
          localStorage.removeItem('mcp_verified_config');
          
          // 检查并禁用所有 MCP 插件
          try {
            const plugins = await mcpPluginApi.getPlugins();
            const activePlugins = plugins.filter(p => p.enabled);
            
            if (activePlugins.length > 0) {
              // 禁用所有插件
              message.loading({ content: '正在禁用 MCP 插件...', key: 'disable_mcp' });
              await Promise.all(activePlugins.map(p => mcpPluginApi.togglePlugin(p.id, false)));
              message.success({ content: '已禁用所有 MCP 插件', key: 'disable_mcp' });
              
              // 显示提示弹窗
              modal.warning({
                title: (
                  <Space>
                    <WarningOutlined style={{ color: '#faad14' }} />
                    <span>API 配置已更改</span>
                  </Space>
                ),
                centered: true,
                content: (
                  <div style={{ padding: '8px 0' }}>
                    <Alert
                      message={`切换到预设「${presetName}」后，API 配置发生了变化。为确保 MCP 插件正常工作，系统已自动禁用所有插件。`}
                      type="warning"
                      showIcon
                      style={{ marginBottom: 16 }}
                    />
                    <div style={{
                      padding: 12,
                      background: 'var(--color-info-bg)',
                      border: '1px solid var(--color-info-border)',
                      borderRadius: 8
                    }}>
                      <Text strong style={{ display: 'block', marginBottom: 8 }}>请完成以下步骤：</Text>
                      <ol style={{ margin: 0, paddingLeft: 20, fontSize: 13 }}>
                        <li>前往 MCP 插件管理页面</li>
                        <li>重新进行"模型能力检查"</li>
                        <li>确认新模型支持 Function Calling 后再启用插件</li>
                      </ol>
                    </div>
                  </div>
                ),
                okText: '前往 MCP 页面',
                cancelText: '稍后处理',
                onOk: () => {
                  eventBus.emit(EventNames.SWITCH_TO_MCP_VIEW);
                },
              });
            }
          } catch (err) {
            console.error('Failed to disable MCP plugins:', err);
          }
        }
      }
    } catch (error) {
      message.error('激活失败');
      console.error(error);
    }
  };

  const handlePresetTest = async (presetId: string) => {
    setTestingPresetId(presetId);
    try {
      const result = await settingsApi.testPreset(presetId);
      if (result.success) {
        modal.success({
          title: '测试成功',
          centered: true,
          width: isMobile ? '90%' : 600,
          content: (
            <div style={{ padding: '8px 0' }}>
              <div style={{ marginBottom: 24, padding: 16, background: 'var(--color-success-bg)', border: '1px solid var(--color-success-border)', borderRadius: 8 }}>
                <Typography.Text strong style={{ color: 'var(--color-success)' }}>
                  ✓ API 连接正常
                </Typography.Text>
              </div>

              <div style={{
                padding: 16,
                background: 'var(--color-bg-layout)',
                borderRadius: 8,
                marginBottom: 16
              }}>
                <div style={{ marginBottom: 8, fontSize: 14 }}>
                  <Text type="secondary">提供商：</Text>
                  <Text strong>{result.provider?.toUpperCase() || 'N/A'}</Text>
                </div>
                <div style={{ marginBottom: 8, fontSize: 14 }}>
                  <Text type="secondary">模型：</Text>
                  <Text strong>{result.model || 'N/A'}</Text>
                </div>
                {result.response_time_ms !== undefined && (
                  <div style={{ fontSize: 14 }}>
                    <Text type="secondary">响应时间：</Text>
                    <Text strong>{result.response_time_ms}ms</Text>
                  </div>
                )}
              </div>

              <Alert
                message="预设配置测试通过，可以正常使用"
                type="success"
                showIcon
              />
            </div>
          ),
        });
      } else {
        modal.error({
          title: '测试失败',
          centered: true,
          width: isMobile ? '90%' : 600,
          content: (
            <div style={{ padding: '8px 0' }}>
              <div style={{ marginBottom: 16 }}>
                <Alert
                  message={result.message || 'API 测试失败'}
                  type="error"
                  showIcon
                />
              </div>

              {result.error && (
                <div style={{
                  padding: 16,
                  background: 'var(--color-error-bg)',
                  border: '1px solid var(--color-error-border)',
                  borderRadius: 8,
                  marginBottom: 16
                }}>
                  <Text strong style={{ fontSize: 14, display: 'block', marginBottom: 8 }}>错误信息:</Text>
                  <Text style={{ fontSize: 13, color: 'var(--color-error)', fontFamily: 'monospace', whiteSpace: 'pre-wrap', wordBreak: 'break-word' }}>
                    {result.error}
                  </Text>
                </div>
              )}

              {result.suggestions && result.suggestions.length > 0 && (
                <div style={{
                  padding: 16,
                  background: 'var(--color-warning-bg)',
                  border: '1px solid var(--color-warning-border)',
                  borderRadius: 8,
                  marginBottom: 16
                }}>
                  <Text strong style={{ fontSize: 14, display: 'block', marginBottom: 8 }}>💡 建议:</Text>
                  <ul style={{ margin: 0, paddingLeft: 20, fontSize: 13 }}>
                    {result.suggestions.map((s, i) => (
                      <li key={i} style={{ marginBottom: 4 }}>{s}</li>
                    ))}
                  </ul>
                </div>
              )}

              <Alert
                message="预设配置存在问题，请检查后重试"
                type="warning"
                showIcon
              />
            </div>
          ),
        });
      }
    } catch (error) {
      message.error('测试失败');
      console.error(error);
    } finally {
      setTestingPresetId(null);
    }
  };

  const handleCreateFromCurrent = () => {
    const currentConfig = form.getFieldsValue();
    presetForm.setFieldsValue({
      name: '',
      description: '',
      ...currentConfig,
    });
    setEditingPreset(null);
    setIsPresetModalVisible(true);
  };

  const mergedModelOptions = buildModelSelectOptions(modelOptions, modelSearchText);
  const mergedPresetModelOptions = buildModelSelectOptions(
    presetModelOptions,
    presetModelSearchText
  );

  return (
    <>
      {contextHolder}
      <div style={{
        minHeight: '90vh',
        background: 'linear-gradient(180deg, var(--color-bg-base) 0%, #EEF2F3 100%)',
        padding: isMobile ? '20px 16px 70px' : '24px 24px 70px',
        display: 'flex',
        flexDirection: 'column',
      }}>
        <div style={{
          maxWidth: 1400,
          margin: '0 auto',
          width: '100%',
          flex: 1,
          display: 'flex',
          flexDirection: 'column',
        }}>
          {/* 顶部导航卡片 */}
          <Card
            variant="borderless"
            style={{
              background: 'linear-gradient(135deg, var(--color-primary) 0%, #5A9BA5 50%, var(--color-primary-hover) 100%)',
              borderRadius: isMobile ? 16 : 24,
              boxShadow: '0 12px 40px rgba(77, 128, 136, 0.25), 0 4px 12px rgba(0, 0, 0, 0.06)',
              marginBottom: isMobile ? 20 : 24,
              border: 'none',
              position: 'relative',
              overflow: 'hidden'
            }}
          >
            {/* 装饰性背景元素 */}
            <div style={{ position: 'absolute', top: -60, right: -60, width: 200, height: 200, borderRadius: '50%', background: 'rgba(255, 255, 255, 0.08)', pointerEvents: 'none' }} />
            <div style={{ position: 'absolute', bottom: -40, left: '30%', width: 120, height: 120, borderRadius: '50%', background: 'rgba(255, 255, 255, 0.05)', pointerEvents: 'none' }} />
            <div style={{ position: 'absolute', top: '50%', right: '15%', width: 80, height: 80, borderRadius: '50%', background: 'rgba(255, 255, 255, 0.06)', pointerEvents: 'none' }} />

            <Row align="middle" justify="space-between" gutter={[16, 16]} style={{ position: 'relative', zIndex: 1 }}>
              <Col xs={24} sm={12}>
                <Space direction="vertical" size={4}>
                  <Title level={isMobile ? 3 : 2} style={{ margin: 0, color: '#fff', textShadow: '0 2px 4px rgba(0,0,0,0.1)' }}>
                    AI API 设置
                  </Title>
                  <Text style={{ fontSize: isMobile ? 12 : 14, color: 'rgba(255,255,255,0.85)', marginLeft: isMobile ? 40 : 48 }}>
                    配置AI接口参数，管理多个API配置预设
                  </Text>
                </Space>
              </Col>
              <Col xs={24} sm={12}>
                {/* 按钮区域预留 */}
              </Col>
            </Row>
          </Card>

          {/* 主内容卡片 */}
          <Card
            variant="borderless"
            style={{
              background: 'rgba(255, 255, 255, 0.95)',
              borderRadius: isMobile ? 12 : 16,
              boxShadow: '0 8px 32px rgba(0, 0, 0, 0.1)',
              flex: 1,
            }}
            styles={{
              body: {
                padding: isMobile ? '16px' : '24px'
              }
            }}
          >
            <Tabs
              activeKey={activeTab}
              onChange={setActiveTab}
              items={[
                {
                  key: 'current',
                  label: '????',
                  children: activeTab === 'current' ? (
                    <Suspense fallback={settingsLazyFallback}>
                      <LazySettingsCurrentTab
                        LazyAzureConfigGuide={LazyAzureConfigGuide}
                        LazyEndpointListEditor={LazyEndpointListEditor}
                        LazyProviderSelector={LazyProviderSelector}
                        activeSettingsSection={activeSettingsSection}
                        activeSettingsSectionMeta={activeSettingsSectionMeta}
                        clipDisplayText={clipDisplayText}
                        endpoints={endpoints}
                        fallbackStrategy={fallbackStrategy}
                        fetchingModels={fetchingModels}
                        fieldHintTextStyle={fieldHintTextStyle}
                        fieldPanelStyle={fieldPanelStyle}
                        form={form}
                        handleDelete={handleDelete}
                        handleFetchModels={handleFetchModels}
                        handleModelSelectFocus={handleModelSelectFocus}
                        handleProviderChange={handleProviderChange}
                        handleReset={handleReset}
                        handleSave={handleSave}
                        handleTestConnection={handleTestConnection}
                        handleTestWebResearch={handleTestWebResearch}
                        hasSettings={hasSettings}
                        initialLoading={initialLoading}
                        isDefaultSettings={isDefaultSettings}
                        isMobile={isMobile}
                        loading={loading}
                        mergedModelOptions={mergedModelOptions}
                        modelOptions={modelOptions}
                        modelSearchText={modelSearchText}
                        modelsFetched={modelsFetched}
                        renderSectionTitle={renderSectionTitle}
                        sectionCardStyle={sectionCardStyle}
                        sectionCardStyles={sectionCardStyles}
                        selectedProvider={selectedProvider}
                        setActiveSettingsSection={setActiveSettingsSection}
                        setEndpoints={setEndpoints}
                        setFallbackStrategy={setFallbackStrategy}
                        setModelsFetched={setModelsFetched}
                        setModelSearchText={setModelSearchText}
                        setShowTestResult={setShowTestResult}
                        settingsLazyFallback={settingsLazyFallback}
                        settingsSectionItems={settingsSectionItems}
                        setWebResearchTestResult={setWebResearchTestResult}
                        showTestResult={showTestResult}
                        testResult={testResult}
                        testingApi={testingApi}
                        testingWebResearchProvider={testingWebResearchProvider}
                        watchedBaseUrl={watchedBaseUrl}
                        watchedExaEnabled={watchedExaEnabled}
                        watchedGrokEnabled={watchedGrokEnabled}
                        watchedMaxTokens={watchedMaxTokens}
                        watchedModel={watchedModel}
                        watchedProvider={watchedProvider}
                        watchedTemperature={watchedTemperature}
                        watchedWebResearchEnabled={watchedWebResearchEnabled}
                        webResearchTestResult={webResearchTestResult}
                      />
                    </Suspense>
                  ) : null,
                },
                {
                  key: 'presets',
                  label: '配置预设',
                  children: activeTab === 'presets' ? (
                    <Suspense fallback={settingsLazyFallback}>
                      <LazySettingsPresetsTab
                        presetsLoading={presetsLoading}
                        presets={presets}
                        activePresetId={activePresetId}
                        testingPresetId={testingPresetId}
                        onCreateFromCurrent={handleCreateFromCurrent}
                        onCreatePreset={() => showPresetModal()}
                        onActivatePreset={handlePresetActivate}
                        onTestPreset={handlePresetTest}
                        onEditPreset={showPresetModal}
                        onDeletePreset={handlePresetDelete}
                      />
                    </Suspense>
                  ) : null,
                },
              ]}
            />
          </Card>
        </div>

        {isPresetModalVisible ? (
          <Suspense fallback={null}>
            <LazySettingsPresetModal
              open={isPresetModalVisible}
              isMobile={isMobile}
              editingPreset={editingPreset}
              form={presetForm}
              fetchingPresetModels={fetchingPresetModels}
              presetModelsFetched={presetModelsFetched}
              mergedPresetModelOptions={mergedPresetModelOptions}
              onOk={handlePresetSave}
              onCancel={handlePresetCancel}
              onProviderChange={handlePresetProviderChange}
              onModelSelectFocus={handlePresetModelSelectFocus}
              onModelSearchChange={handlePresetModelSearchChange}
              onModelChange={handlePresetModelChange}
              onModelCommit={handlePresetModelCommit}
              onModelReload={handlePresetModelReload}
            />
          </Suspense>
        ) : null}
      </div>
    </>
  );
}
